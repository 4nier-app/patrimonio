//! Comandos expostos à interface. Toda a lógica fica nos crates `carteira-*`;
//! aqui só existe a ponte com o Tauri.

mod asset;
mod changelog;
mod evolution;
mod market;
mod recovery;
mod views;

use std::path::PathBuf;
use std::sync::Mutex;

use carteira_core::corporate::CorporateEvent;
use carteira_core::income::Income;
use carteira_core::operations::{Operation, PricedOperation};
use carteira_core::savings::{Savings, SavingsMove};
use carteira_ledger::Event;
use carteira_storage::{
    backup_summary, looks_like_email, Account, Asset, BackupInfo, BackupSummary, IntegrityReport,
    NoteImport, NoteStatus, QuoteUpdate, RecoveryStatus, Vault, VaultError, VersionEntry,
    BACKUP_EXTENSION,
};
use serde::Serialize;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTO_BACKUP_DAYS: i64 = 7;
const AUTO_BACKUP_KEEP: usize = 5;

struct AppState {
    data_dir: PathBuf,
    vault: Mutex<Option<Vault>>,
}

#[derive(Serialize)]
struct VaultStatus {
    exists: bool,
    /// O cofre desta pasta tem chave de recuperação ("Esqueci minha senha").
    has_recovery_key: bool,
    unlocked: bool,
    app_version: &'static str,
    data_dir: String,
}

fn err(e: VaultError) -> String {
    e.to_string()
}

fn with_vault<T>(
    state: &State<AppState>,
    f: impl FnOnce(&mut Vault) -> Result<T, VaultError>,
) -> Result<T, String> {
    let mut guard = state
        .vault
        .lock()
        .map_err(|_| "estado interno indisponível".to_string())?;
    let vault = guard
        .as_mut()
        .ok_or_else(|| "o cofre está bloqueado".to_string())?;
    f(vault).map_err(err)
}

#[tauri::command]
fn vault_status(state: State<AppState>) -> VaultStatus {
    VaultStatus {
        exists: Vault::exists(&state.data_dir),
        has_recovery_key: Vault::has_recovery_key(&state.data_dir),
        unlocked: state.vault.lock().map(|v| v.is_some()).unwrap_or(false),
        app_version: APP_VERSION,
        data_dir: state.data_dir.display().to_string(),
    }
}

fn after_unlock(mut vault: Vault, state: &State<AppState>) -> Result<(), String> {
    vault
        .record_app_version(&changelog::section(APP_VERSION))
        .map_err(err)?;
    // Cópia de segurança semanal na pasta do cofre; uma falha aqui não
    // impede de abrir a carteira (a tela Segurança mostra a data do último).
    let _ = vault.auto_backup(AUTO_BACKUP_DAYS, AUTO_BACKUP_KEEP);
    *state
        .vault
        .lock()
        .map_err(|_| "estado interno indisponível".to_string())? = Some(vault);
    Ok(())
}

/// Cria o cofre e já gera a chave de recuperação, que volta para a tela
/// mostrar ao usuário (ela não é gravada em lugar nenhum).
#[tauri::command]
fn create_vault(
    password: String,
    confirmation: String,
    email: Option<String>,
    state: State<AppState>,
) -> Result<String, String> {
    if password != confirmation {
        return Err("as senhas não conferem".into());
    }
    let email = email.filter(|e| !e.trim().is_empty());
    if let Some(e) = &email {
        if !looks_like_email(e.trim()) {
            return Err(format!("e-mail inválido: {}", e.trim()));
        }
    }
    let mut vault = Vault::create(&state.data_dir, &password, APP_VERSION).map_err(err)?;
    let key = vault.setup_recovery(email.as_deref()).map_err(err)?;
    after_unlock(vault, &state)?;
    Ok(key)
}

/// "Esqueci minha senha": a chave de recuperação abre o cofre e define a nova senha.
#[tauri::command]
fn reset_password(
    recovery_key: String,
    new: String,
    confirmation: String,
    state: State<AppState>,
) -> Result<(), String> {
    if new != confirmation {
        return Err("as senhas novas não conferem".into());
    }
    let vault =
        Vault::reset_password(&state.data_dir, &recovery_key, &new, APP_VERSION).map_err(err)?;
    after_unlock(vault, &state)
}

#[tauri::command]
fn recovery_status(state: State<AppState>) -> Result<RecoveryStatus, String> {
    with_vault(&state, |v| v.recovery_status())
}

/// Gera uma chave de recuperação nova; a anterior deixa de valer.
#[tauri::command]
fn generate_recovery_key(email: Option<String>, state: State<AppState>) -> Result<String, String> {
    with_vault(&state, |v| v.setup_recovery(email.as_deref()))
}

/// Abre o programa de e-mail do usuário com a mensagem pronta para ele mesmo.
#[tauri::command]
fn send_recovery_email(email: String, key: String, app: tauri::AppHandle) -> Result<(), String> {
    let email = email.trim();
    if !looks_like_email(email) {
        return Err(format!("e-mail inválido: {email}"));
    }
    let now = chrono::Local::now().format("%d/%m/%Y às %H:%M").to_string();
    app.opener()
        .open_url(recovery::mailto(email, &key, &now), None::<&str>)
        .map_err(|e| format!("não foi possível abrir o programa de e-mail: {e}"))
}

// ---------- etapa 3: backup ----------

#[derive(Serialize)]
struct BackupStatus {
    auto_dir: String,
    last_auto: Option<String>,
    last_backup: Option<String>,
}

#[tauri::command]
fn backup_status(state: State<AppState>) -> Result<BackupStatus, String> {
    with_vault(&state, |v| {
        Ok(BackupStatus {
            auto_dir: v.auto_backup_dir().display().to_string(),
            last_auto: v.last_auto_backup()?.map(|d| d.to_string()),
            last_backup: v.setting("backup.ultimo")?,
        })
    })
}

/// Pergunta onde salvar e grava o backup. `None` quando o usuário cancela.
#[tauri::command]
async fn export_backup(app: tauri::AppHandle) -> Result<Option<BackupInfo>, String> {
    let name = format!(
        "carteira-auditavel-{}.{BACKUP_EXTENSION}",
        chrono::Local::now().format("%Y-%m-%d")
    );
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Salvar backup da carteira")
        .set_file_name(&name)
        .add_filter("Backup do 4Nier", &[BACKUP_EXTENSION]);
    if let Ok(docs) = app.path().document_dir() {
        dialog = dialog.set_directory(docs);
    }
    let Some(path) = dialog.blocking_save_file() else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    with_vault(&state, |v| v.export_backup(&path)).map(Some)
}

/// Pergunta qual backup restaurar e mostra o que há nele (sem a senha).
#[tauri::command]
async fn pick_backup(app: tauri::AppHandle) -> Result<Option<BackupSummary>, String> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Escolha o backup da carteira")
        .add_filter("Backup do 4Nier", &[BACKUP_EXTENSION]);
    if let Ok(docs) = app.path().document_dir() {
        dialog = dialog.set_directory(docs);
    }
    let Some(path) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    backup_summary(&path).map(Some).map_err(err)
}

#[derive(Serialize)]
struct RestoreResult {
    /// Pasta onde o cofre que estava em uso ficou guardado.
    previous: Option<String>,
}

/// Restaura o backup com a senha que valia quando ele foi feito. O backup é
/// conferido antes; só então o cofre atual é fechado e guardado.
#[tauri::command]
async fn restore_backup(
    path: String,
    password: String,
    app: tauri::AppHandle,
) -> Result<RestoreResult, String> {
    let state = app.state::<AppState>();
    let prepared = Vault::prepare_restore(
        &state.data_dir,
        std::path::Path::new(&path),
        &password,
        APP_VERSION,
    )
    .map_err(err)?;
    let mut guard = state
        .vault
        .lock()
        .map_err(|_| "estado interno indisponível".to_string())?;
    // Fecha o cofre atual antes de mover os arquivos.
    *guard = None;
    let (vault, previous) = prepared.apply().map_err(err)?;
    drop(guard);
    after_unlock(vault, &state)?;
    Ok(RestoreResult {
        previous: previous.map(|p| p.display().to_string()),
    })
}

/// Abre no Explorer a pasta dos dados (onde ficam os backups automáticos).
#[tauri::command]
fn open_data_folder(state: State<AppState>, app: tauri::AppHandle) -> Result<(), String> {
    app.opener()
        .open_path(state.data_dir.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn unlock(password: String, state: State<AppState>) -> Result<(), String> {
    let vault = Vault::open(&state.data_dir, &password, APP_VERSION).map_err(err)?;
    after_unlock(vault, &state)
}

#[tauri::command]
fn lock(state: State<AppState>) {
    if let Ok(mut v) = state.vault.lock() {
        *v = None;
    }
}

#[tauri::command]
fn change_password(
    current: String,
    new: String,
    confirmation: String,
    state: State<AppState>,
) -> Result<(), String> {
    if new != confirmation {
        return Err("as senhas novas não conferem".into());
    }
    with_vault(&state, |v| v.change_password(&current, &new))
}

#[tauri::command]
fn list_events(limit: i64, offset: i64, state: State<AppState>) -> Result<Vec<Event>, String> {
    with_vault(&state, |v| v.list_events(limit, offset))
}

#[tauri::command]
fn verify_integrity(state: State<AppState>) -> Result<IntegrityReport, String> {
    with_vault(&state, |v| v.verify_integrity())
}

#[tauri::command]
fn version_history(state: State<AppState>) -> Result<Vec<VersionEntry>, String> {
    with_vault(&state, |v| v.version_history())
}

// ---------- etapa 1: ativos, lançamentos, carteira e rentabilidade ----------

#[tauri::command]
fn search_assets(query: String, state: State<AppState>) -> Result<Vec<Asset>, String> {
    with_vault(&state, |v| v.search_assets(&query, 20))
}

/// Fundos do catálogo da CVM pelo nome (como no extrato) ou pelo CNPJ.
#[tauri::command]
fn search_funds(query: String, state: State<AppState>) -> Result<Vec<Asset>, String> {
    with_vault(&state, |v| v.search_funds(&query, 30))
}

/// Nome de um fundo lançado pelo CNPJ que não está no catálogo da CVM.
#[tauri::command]
fn name_fund(ticker: String, name: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.name_fund(&ticker, &name))
}

/// Baixa a lista de fundos da CVM, se ainda não houver ou tiver mais de 30 dias.
#[tauri::command]
async fn update_fund_catalog(app: tauri::AppHandle) -> Result<market::UpdateLine, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        market::update_fund_catalog(&state.vault, &market::Internet, today())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Página do ativo: posição, lançamentos e preço médio.
#[tauri::command]
fn asset_detail(ticker: String, state: State<AppState>) -> Result<asset::AssetDetail, String> {
    with_vault_str(&state, |v| asset::detail(v, &ticker, today()))
}

/// Variação de hoje de cada ativo da carteira (Home broker), buscada na hora.
#[tauri::command]
async fn day_moves(app: tauri::AppHandle) -> Result<asset::DayView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let inputs = {
            let guard = state
                .vault
                .lock()
                .map_err(|_| "estado interno indisponível".to_string())?;
            let v = guard
                .as_ref()
                .ok_or_else(|| "o cofre está bloqueado".to_string())?;
            asset::day_inputs(v, today())?
        };
        Ok(asset::day_moves(inputs, &asset::Internet))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Gráfico do ativo, buscado na hora (sem segurar o cofre durante a rede).
#[tauri::command]
async fn asset_chart(
    ticker: String,
    period: String,
    interval: String,
    app: tauri::AppHandle,
) -> Result<asset::ChartData, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let detail = {
            let guard = state
                .vault
                .lock()
                .map_err(|_| "estado interno indisponível".to_string())?;
            let v = guard
                .as_ref()
                .ok_or_else(|| "o cofre está bloqueado".to_string())?;
            asset::detail(v, &ticker, today())?
        };
        Ok(asset::chart(
            &detail,
            &asset::Internet,
            &period,
            &interval,
            today(),
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn list_accounts(state: State<AppState>) -> Result<Vec<Account>, String> {
    with_vault(&state, |v| v.list_accounts())
}

#[tauri::command]
fn list_operations(state: State<AppState>) -> Result<Vec<PricedOperation>, String> {
    with_vault_str(&state, |v| views::list_operations(v))
}

#[tauri::command]
fn preview_operation(
    operation: Operation,
    state: State<AppState>,
) -> Result<views::Preview, String> {
    with_vault_str(&state, |v| views::preview(v, operation))
}

#[tauri::command]
fn create_operation(operation: Operation, state: State<AppState>) -> Result<Operation, String> {
    with_vault(&state, |v| v.create_operation(operation))
}

#[tauri::command]
fn update_operation(
    operation: Operation,
    reason: Option<String>,
    state: State<AppState>,
) -> Result<Operation, String> {
    let reason = reason.filter(|r| !r.trim().is_empty());
    with_vault(&state, |v| v.update_operation(operation, reason.as_deref()))
}

#[tauri::command]
fn remove_operation(
    id: String,
    reason: Option<String>,
    state: State<AppState>,
) -> Result<(), String> {
    with_vault(&state, |v| v.remove_operation(&id, reason.as_deref()))
}

#[tauri::command]
fn remove_operations(ids: Vec<String>, state: State<AppState>) -> Result<usize, String> {
    with_vault(&state, |v| v.remove_operations(&ids))
}

#[tauri::command]
fn restore_operations(ids: Vec<String>, state: State<AppState>) -> Result<usize, String> {
    with_vault(&state, |v| v.restore_operations(&ids))
}

#[tauri::command]
fn restore_income(id: String, state: State<AppState>) -> Result<Income, String> {
    with_vault(&state, |v| v.restore_income(&id))
}

#[tauri::command]
fn restore_event(id: String, state: State<AppState>) -> Result<CorporateEvent, String> {
    with_vault(&state, |v| v.restore_event(&id))
}

#[tauri::command]
fn savings(state: State<AppState>) -> Result<views::SavingsView, String> {
    with_vault_str(&state, |v| views::savings_view(v))
}

#[tauri::command]
fn create_savings(item: Savings, state: State<AppState>) -> Result<Savings, String> {
    with_vault(&state, |v| v.create_savings(item))
}

#[tauri::command]
fn update_savings(item: Savings, state: State<AppState>) -> Result<Savings, String> {
    with_vault(&state, |v| v.update_savings(item))
}

#[tauri::command]
fn remove_savings(id: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.remove_savings(&id))
}

#[tauri::command]
fn restore_savings(id: String, state: State<AppState>) -> Result<Savings, String> {
    with_vault(&state, |v| v.restore_savings(&id))
}

#[tauri::command]
fn create_savings_move(item: SavingsMove, state: State<AppState>) -> Result<SavingsMove, String> {
    with_vault(&state, |v| v.create_savings_move(item))
}

#[tauri::command]
fn update_savings_move(item: SavingsMove, state: State<AppState>) -> Result<SavingsMove, String> {
    with_vault(&state, |v| v.update_savings_move(item))
}

#[tauri::command]
fn remove_savings_move(id: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.remove_savings_move(&id))
}

#[tauri::command]
fn restore_savings_move(id: String, state: State<AppState>) -> Result<SavingsMove, String> {
    with_vault(&state, |v| v.restore_savings_move(&id))
}

#[tauri::command]
fn corporate_events(state: State<AppState>) -> Result<views::EventsView, String> {
    with_vault_str(&state, |v| views::corporate_events(v))
}

#[tauri::command]
fn preview_event(event: CorporateEvent, state: State<AppState>) -> Result<views::EventRow, String> {
    with_vault_str(&state, |v| views::preview_event(v, event))
}

#[tauri::command]
fn create_event(event: CorporateEvent, state: State<AppState>) -> Result<CorporateEvent, String> {
    with_vault(&state, |v| v.create_event(event))
}

#[tauri::command]
fn update_event(
    event: CorporateEvent,
    reason: Option<String>,
    state: State<AppState>,
) -> Result<CorporateEvent, String> {
    with_vault(&state, |v| v.update_event(event, reason.as_deref()))
}

#[tauri::command]
fn remove_event(id: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.remove_event(&id))
}

#[tauri::command]
fn incomes(state: State<AppState>) -> Result<views::IncomesView, String> {
    with_vault_str(&state, |v| views::incomes(v))
}

#[tauri::command]
fn create_income(income: Income, state: State<AppState>) -> Result<Income, String> {
    with_vault(&state, |v| v.create_income(income))
}

#[tauri::command]
fn update_income(
    income: Income,
    reason: Option<String>,
    state: State<AppState>,
) -> Result<Income, String> {
    let reason = reason.filter(|r| !r.trim().is_empty());
    with_vault(&state, |v| v.update_income(income, reason.as_deref()))
}

#[tauri::command]
fn remove_income(id: String, reason: Option<String>, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.remove_income(&id, reason.as_deref()))
}

#[tauri::command]
fn note_status(hashes: Vec<String>, state: State<AppState>) -> Result<Vec<NoteStatus>, String> {
    with_vault(&state, |v| {
        hashes.iter().map(|h| v.note_status(h)).collect()
    })
}

/// Para cada (nome de pregão, classe), os códigos da B3 que batem.
#[tauri::command]
fn resolve_b3_names(
    items: Vec<(String, String)>,
    state: State<AppState>,
) -> Result<Vec<Vec<String>>, String> {
    with_vault(&state, |v| {
        items
            .iter()
            .map(|(name, class)| v.find_b3_by_name(name, class))
            .collect()
    })
}

#[tauri::command]
fn import_note(note: NoteImport, state: State<AppState>) -> Result<usize, String> {
    with_vault(&state, |v| v.import_note(note).map(|ops| ops.len()))
}

#[tauri::command]
fn portfolio(state: State<AppState>) -> Result<views::PortfolioView, String> {
    with_vault_str(&state, |v| views::portfolio(v, today()))
}

#[tauri::command]
fn returns_report(state: State<AppState>) -> Result<views::ReturnsView, String> {
    with_vault_str(&state, |v| views::returns(v, today()))
}

#[tauri::command]
fn taxes(state: State<AppState>) -> Result<views::TaxView, String> {
    with_vault_str(&state, |v| views::taxes(v, today()))
}

/// Marca o DARF de uma competência como pago.
#[tauri::command]
fn mark_darf_paid(
    payment: carteira_storage::DarfPayment,
    state: State<AppState>,
) -> Result<(), String> {
    with_vault(&state, |v| v.mark_darf_paid(&payment))
}

#[tauri::command]
fn unmark_darf_paid(month: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.unmark_darf_paid(&month))
}

#[tauri::command]
fn dashboard(state: State<AppState>) -> Result<views::DashboardView, String> {
    with_vault_str(&state, |v| views::dashboard(v, today()))
}

/// Evolução do patrimônio de um ano (ou de todos, com `year` vazio).
#[tauri::command]
fn evolution(
    year: Option<i32>,
    state: State<AppState>,
) -> Result<evolution::EvolutionView, String> {
    with_vault_str(&state, |v| evolution::evolution(v, year, today()))
}

#[tauri::command]
fn dashboard_days(
    year: i32,
    month: u32,
    state: State<AppState>,
) -> Result<views::DaysView, String> {
    with_vault_str(&state, |v| views::dashboard_days(v, year, month, today()))
}

#[tauri::command]
fn list_quote_updates(state: State<AppState>) -> Result<Vec<QuoteUpdate>, String> {
    with_vault(&state, |v| v.list_quote_updates(30))
}

#[tauri::command]
async fn update_quotes(app: tauri::AppHandle) -> Result<Vec<market::UpdateLine>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        market::update_all(&state.vault, &market::Internet, today())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

fn with_vault_str<T>(
    state: &State<AppState>,
    f: impl FnOnce(&mut Vault) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = state
        .vault
        .lock()
        .map_err(|_| "estado interno indisponível".to_string())?;
    let vault = guard
        .as_mut()
        .ok_or_else(|| "o cofre está bloqueado".to_string())?;
    f(vault)
}

pub fn run() {
    tauri::Builder::default()
        // Uma janela só: duas cópias abertas mexeriam no mesmo cofre ao mesmo
        // tempo. Abrir de novo só traz a janela que já está aberta para a frente.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // A janela abre do jeito que foi fechada (tamanho, posição, maximizada).
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            // Na primeira vez, sem tamanho guardado, abre maximizada: o app tem
            // muitas tabelas e gráficos.
            let saved = app
                .path()
                .app_config_dir()
                .map(|d| d.join(tauri_plugin_window_state::DEFAULT_FILENAME).exists())
                .unwrap_or(false);
            if !saved {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.maximize();
                }
            }
            let data_dir = app.path().app_data_dir()?;
            app.manage(AppState {
                data_dir,
                vault: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            vault_status,
            create_vault,
            unlock,
            lock,
            change_password,
            list_events,
            verify_integrity,
            version_history,
            search_assets,
            search_funds,
            asset_detail,
            asset_chart,
            day_moves,
            name_fund,
            update_fund_catalog,
            list_accounts,
            list_operations,
            preview_operation,
            create_operation,
            update_operation,
            remove_operation,
            remove_operations,
            restore_operations,
            restore_income,
            restore_event,
            corporate_events,
            savings,
            create_savings,
            update_savings,
            remove_savings,
            restore_savings,
            create_savings_move,
            update_savings_move,
            remove_savings_move,
            restore_savings_move,
            preview_event,
            create_event,
            update_event,
            remove_event,
            incomes,
            create_income,
            update_income,
            remove_income,
            portfolio,
            returns_report,
            dashboard,
            note_status,
            resolve_b3_names,
            import_note,
            dashboard_days,
            evolution,
            taxes,
            mark_darf_paid,
            unmark_darf_paid,
            list_quote_updates,
            update_quotes,
            reset_password,
            recovery_status,
            generate_recovery_key,
            send_recovery_email,
            backup_status,
            export_backup,
            pick_backup,
            restore_backup,
            open_data_folder,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
