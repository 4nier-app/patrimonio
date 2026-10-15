//! Comandos expostos à interface. Toda a lógica fica nos crates `carteira-*`;
//! aqui só existe a ponte com o Tauri.

mod changelog;
mod market;
mod views;

use std::path::PathBuf;
use std::sync::Mutex;

use carteira_core::operations::{Operation, PricedOperation};
use carteira_ledger::Event;
use carteira_storage::{
    Account, Asset, IntegrityReport, QuoteUpdate, Vault, VaultError, VersionEntry,
};
use serde::Serialize;
use tauri::{Manager, State};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

struct AppState {
    data_dir: PathBuf,
    vault: Mutex<Option<Vault>>,
}

#[derive(Serialize)]
struct VaultStatus {
    exists: bool,
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
        unlocked: state.vault.lock().map(|v| v.is_some()).unwrap_or(false),
        app_version: APP_VERSION,
        data_dir: state.data_dir.display().to_string(),
    }
}

fn after_unlock(mut vault: Vault, state: &State<AppState>) -> Result<(), String> {
    vault
        .record_app_version(&changelog::section(APP_VERSION))
        .map_err(err)?;
    *state
        .vault
        .lock()
        .map_err(|_| "estado interno indisponível".to_string())? = Some(vault);
    Ok(())
}

#[tauri::command]
fn create_vault(
    password: String,
    confirmation: String,
    state: State<AppState>,
) -> Result<(), String> {
    if password != confirmation {
        return Err("as senhas não conferem".into());
    }
    let vault = Vault::create(&state.data_dir, &password, APP_VERSION).map_err(err)?;
    after_unlock(vault, &state)
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
fn remove_operation(id: String, reason: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.remove_operation(&id, &reason))
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
        .setup(|app| {
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
            list_accounts,
            list_operations,
            preview_operation,
            create_operation,
            update_operation,
            remove_operation,
            portfolio,
            returns_report,
            list_quote_updates,
            update_quotes,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
