//! Comandos expostos à interface. Toda a lógica fica nos crates `carteira-*`;
//! aqui só existe a ponte com o Tauri.

mod changelog;

use std::path::PathBuf;
use std::sync::Mutex;

use carteira_ledger::Event;
use carteira_storage::{IntegrityReport, Vault, VaultError, VersionEntry};
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

fn with_vault<T>(state: &State<AppState>, f: impl FnOnce(&mut Vault) -> Result<T, VaultError>) -> Result<T, String> {
    let mut guard = state.vault.lock().map_err(|_| "estado interno indisponível".to_string())?;
    let vault = guard.as_mut().ok_or_else(|| "o cofre está bloqueado".to_string())?;
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
    vault.record_app_version(&changelog::section(APP_VERSION)).map_err(err)?;
    *state.vault.lock().map_err(|_| "estado interno indisponível".to_string())? = Some(vault);
    Ok(())
}

#[tauri::command]
fn create_vault(password: String, confirmation: String, state: State<AppState>) -> Result<(), String> {
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
fn change_password(current: String, new: String, confirmation: String, state: State<AppState>) -> Result<(), String> {
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

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(AppState { data_dir, vault: Mutex::new(None) });
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
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
