//! Cofre local da Carteira Auditável.
//!
//! O banco é um arquivo SQLite criptografado com SQLCipher. A chave é derivada
//! da senha do usuário com Argon2id e um sal aleatório; a senha nunca é
//! gravada. O sal e os parâmetros da derivação ficam em `cofre.json`, ao lado
//! do banco. Sem a senha, o arquivo do banco é ilegível.

mod backup;
mod incomes;
mod kdf;
mod migrations;
mod notes;
mod records;

use std::fs;
use std::path::{Path, PathBuf};

use carteira_ledger::{self as ledger, Action, ChainError, Event, NewEvent};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use zeroize::Zeroizing;

pub use backup::{
    summary as backup_summary, BackupInfo, BackupSummary, PreparedRestore, BACKUP_EXTENSION,
};
use kdf::DataKey;
pub use kdf::KdfParams;
pub use notes::{NoteImport, NoteStatus};
pub use records::{
    normalize_ticker, Account, Asset, DarfPayment, FxTable, NewFxRate, NewQuote, PriceTable,
    QuoteUpdate, USD_BRL,
};

pub const DB_FILE: &str = "carteira.db";
pub const VAULT_FILE: &str = "cofre.json";

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("já existe um cofre nesta pasta")]
    AlreadyExists,
    #[error("nenhum cofre encontrado nesta pasta")]
    NotFound,
    #[error("senha incorreta")]
    WrongPassword,
    #[error("chave de recuperação incorreta")]
    WrongRecoveryKey,
    #[error("este cofre não tem chave de recuperação")]
    NoRecoveryKey,
    #[error("a senha precisa ter pelo menos {0} caracteres")]
    WeakPassword(usize),
    #[error("arquivo do cofre inválido: {0}")]
    InvalidVaultFile(String),
    #[error("erro de banco de dados: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("erro de arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("erro na derivação da chave: {0}")]
    Kdf(String),
}

pub type Result<T> = std::result::Result<T, VaultError>;

/// Conferência simples: algo@algo.algo, sem espaços.
pub fn looks_like_email(e: &str) -> bool {
    let Some((user, domain)) = e.split_once('@') else {
        return false;
    };
    !user.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !e.chars().any(char::is_whitespace)
        && !domain.contains('@')
}

pub const MIN_PASSWORD_LEN: usize = 8;

/// Resultado da verificação de integridade do histórico.
#[derive(Debug, Clone, Serialize)]
pub struct IntegrityReport {
    pub ok: bool,
    pub events_checked: usize,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionEntry {
    pub version: String,
    pub installed_at: String,
    pub changelog: String,
}

pub struct Vault {
    conn: Connection,
    dir: PathBuf,
    app_version: String,
    /// Chave do banco em memória enquanto o cofre está aberto; permite trocar
    /// a senha e gerar nova chave de recuperação sem recriptografar o banco.
    data_key: DataKey,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn apply_key(conn: &Connection, key: &[u8]) -> Result<()> {
    // Senha errada é um caso esperado; não precisa de log do SQLCipher no terminal.
    let _ = conn.execute_batch("PRAGMA cipher_log_level = NONE;");
    let hex_key = Zeroizing::new(hex::encode(key));
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex_key.as_str()))?;
    Ok(())
}

/// Abre o banco com a chave; `None` quando a chave não abre o arquivo.
fn open_db(path: &Path, key: &DataKey) -> Result<Option<Connection>> {
    let conn = Connection::open(path)?;
    apply_key(&conn, key.as_slice())?;
    // Com a chave errada o SQLCipher só falha na primeira leitura.
    if conn
        .query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })
        .is_err()
    {
        return Ok(None);
    }
    Ok(Some(conn))
}

/// Situação da chave de recuperação, para a tela de Segurança.
#[derive(Debug, Clone, Serialize)]
pub struct RecoveryStatus {
    pub configured: bool,
    pub email: Option<String>,
    pub created_at: Option<String>,
}

const SETTING_EMAIL: &str = "recuperacao.email";
const SETTING_RECOVERY_AT: &str = "recuperacao.gerada_em";

impl Vault {
    /// Indica se já existe um cofre na pasta.
    pub fn exists(dir: &Path) -> bool {
        dir.join(VAULT_FILE).exists()
    }

    /// Cria um cofre novo, protegido pela senha.
    pub fn create(dir: &Path, password: &str, app_version: &str) -> Result<Self> {
        Self::create_with_params(dir, password, app_version, KdfParams::default())
    }

    pub fn create_with_params(
        dir: &Path,
        password: &str,
        app_version: &str,
        params: KdfParams,
    ) -> Result<Self> {
        if password.chars().count() < MIN_PASSWORD_LEN {
            return Err(VaultError::WeakPassword(MIN_PASSWORD_LEN));
        }
        if Self::exists(dir) || dir.join(DB_FILE).exists() {
            return Err(VaultError::AlreadyExists);
        }
        fs::create_dir_all(dir)?;
        let data_key = kdf::new_data_key()?;
        let header = kdf::VaultHeader::new(params, &data_key, password)?;

        let conn = Connection::open(dir.join(DB_FILE))?;
        apply_key(&conn, data_key.as_slice())?;
        migrations::run(&conn)?;
        header.write(&dir.join(VAULT_FILE))?;

        let mut vault = Vault {
            conn,
            dir: dir.to_path_buf(),
            app_version: app_version.to_string(),
            data_key,
        };
        vault.append_event(
            "SISTEMA",
            "cofre",
            Action::Criada,
            None,
            Some(r#"{"evento":"cofre criado"}"#),
            None,
        )?;
        Ok(vault)
    }

    /// Abre um cofre existente. Senha errada resulta em [`VaultError::WrongPassword`].
    /// Um cofre do formato antigo (até a 0.3.0) é convertido na primeira abertura.
    pub fn open(dir: &Path, password: &str, app_version: &str) -> Result<Self> {
        let vault_file = dir.join(VAULT_FILE);
        if !vault_file.exists() {
            return Err(VaultError::NotFound);
        }
        let mut header = kdf::VaultHeader::read(&vault_file)?;
        let data_key = header
            .key_from_password(password)?
            .ok_or(VaultError::WrongPassword)?;
        let conn = open_db(&dir.join(DB_FILE), &data_key)?.ok_or(VaultError::WrongPassword)?;
        migrations::run(&conn)?;
        let mut vault = Vault {
            conn,
            dir: dir.to_path_buf(),
            app_version: app_version.to_string(),
            data_key,
        };
        if header.is_legacy() {
            header.set_password(&vault.data_key, password)?;
            header.write(&vault_file)?;
            vault.append_event(
                "SISTEMA",
                "cofre",
                Action::Alterada,
                None,
                Some(r#"{"evento":"cofre convertido para o formato 2 (permite chave de recuperação)"}"#),
                None,
            )?;
        }
        Ok(vault)
    }

    /// "Esqueci minha senha": abre o cofre com a chave de recuperação e grava
    /// a nova senha. A chave de recuperação continua valendo.
    pub fn reset_password(
        dir: &Path,
        recovery_key: &str,
        new_password: &str,
        app_version: &str,
    ) -> Result<Self> {
        if new_password.chars().count() < MIN_PASSWORD_LEN {
            return Err(VaultError::WeakPassword(MIN_PASSWORD_LEN));
        }
        let vault_file = dir.join(VAULT_FILE);
        if !vault_file.exists() {
            return Err(VaultError::NotFound);
        }
        let mut header = kdf::VaultHeader::read(&vault_file)?;
        if !header.has_recovery() {
            return Err(VaultError::NoRecoveryKey);
        }
        let data_key = header
            .key_from_recovery(recovery_key)?
            .ok_or(VaultError::WrongRecoveryKey)?;
        let conn = open_db(&dir.join(DB_FILE), &data_key)?.ok_or(VaultError::WrongRecoveryKey)?;
        migrations::run(&conn)?;
        header.set_password(&data_key, new_password)?;
        header.write(&vault_file)?;
        let mut vault = Vault {
            conn,
            dir: dir.to_path_buf(),
            app_version: app_version.to_string(),
            data_key,
        };
        vault.append_event(
            "SISTEMA",
            "cofre",
            Action::Alterada,
            None,
            Some(r#"{"evento":"senha redefinida com a chave de recuperação"}"#),
            None,
        )?;
        Ok(vault)
    }

    /// Indica se o cofre da pasta tem chave de recuperação, sem abri-lo.
    pub fn has_recovery_key(dir: &Path) -> bool {
        kdf::VaultHeader::read(&dir.join(VAULT_FILE))
            .map(|h| h.has_recovery())
            .unwrap_or(false)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Troca a senha. A chave do banco não muda: ela passa a ser protegida
    /// pela nova senha (com novo sal), e a chave de recuperação continua valendo.
    pub fn change_password(&mut self, current: &str, new: &str) -> Result<()> {
        if new.chars().count() < MIN_PASSWORD_LEN {
            return Err(VaultError::WeakPassword(MIN_PASSWORD_LEN));
        }
        let vault_file = self.dir.join(VAULT_FILE);
        let mut header = kdf::VaultHeader::read(&vault_file)?;
        match header.key_from_password(current)? {
            Some(k) if *k == *self.data_key => {}
            _ => return Err(VaultError::WrongPassword),
        }
        header.set_password(&self.data_key, new)?;
        header.write(&vault_file)?;
        self.append_event(
            "SISTEMA",
            "cofre",
            Action::Alterada,
            None,
            Some(r#"{"evento":"senha alterada"}"#),
            None,
        )?;
        Ok(())
    }

    /// Gera uma chave de recuperação nova (a anterior deixa de valer) e guarda
    /// o e-mail para onde ela será enviada. Retorna a chave, que não é gravada.
    pub fn setup_recovery(&mut self, email: Option<&str>) -> Result<String> {
        let email = email.map(str::trim).filter(|e| !e.is_empty());
        if let Some(e) = email {
            if !looks_like_email(e) {
                return Err(VaultError::Invalid(format!("e-mail inválido: {e}")));
            }
        }
        let key = kdf::new_recovery_key()?;
        let vault_file = self.dir.join(VAULT_FILE);
        let mut header = kdf::VaultHeader::read(&vault_file)?;
        let replaced = header.has_recovery();
        header.set_recovery(&self.data_key, &key)?;
        header.write(&vault_file)?;
        let now = now_iso();
        match email {
            Some(e) => self.set_setting(SETTING_EMAIL, e)?,
            None => self.delete_setting(SETTING_EMAIL)?,
        }
        self.set_setting(SETTING_RECOVERY_AT, &now)?;
        let after = serde_json::json!({
            "evento": if replaced { "nova chave de recuperação gerada (a anterior deixou de valer)" } else { "chave de recuperação gerada" },
            "email": email,
        })
        .to_string();
        self.append_event(
            "SISTEMA",
            "cofre",
            Action::Alterada,
            None,
            Some(&after),
            None,
        )?;
        Ok(key)
    }

    pub fn recovery_status(&self) -> Result<RecoveryStatus> {
        let header = kdf::VaultHeader::read(&self.dir.join(VAULT_FILE))?;
        Ok(RecoveryStatus {
            configured: header.has_recovery(),
            email: self.setting(SETTING_EMAIL)?,
            created_at: self.setting(SETTING_RECOVERY_AT)?,
        })
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now_iso()],
        )?;
        Ok(())
    }

    fn delete_setting(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Acrescenta um evento ao fim da cadeia, numa transação.
    pub fn append_event(
        &mut self,
        entity: &str,
        entity_id: &str,
        action: Action,
        before: Option<&str>,
        after: Option<&str>,
        reason: Option<&str>,
    ) -> Result<Event> {
        let tx = self.conn.transaction()?;
        let last = last_event(&tx)?;
        let event = ledger::chain(
            last.as_ref(),
            NewEvent {
                occurred_at: now_iso(),
                entity: entity.to_string(),
                entity_id: entity_id.to_string(),
                action,
                before: before.map(str::to_string),
                after: after.map(str::to_string),
                reason: reason.map(str::to_string),
                app_version: self.app_version.clone(),
            },
        );
        tx.execute(
            "INSERT INTO event_log (seq, occurred_at, entity, entity_id, action, before_json, after_json, reason, app_version, prev_hash, hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                event.seq,
                event.data.occurred_at,
                event.data.entity,
                event.data.entity_id,
                event.data.action.as_str(),
                event.data.before,
                event.data.after,
                event.data.reason,
                event.data.app_version,
                event.prev_hash,
                event.hash,
            ],
        )?;
        tx.commit()?;
        Ok(event)
    }

    /// Eventos do mais recente para o mais antigo.
    pub fn list_events(&self, limit: i64, offset: i64) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT_EVENT} ORDER BY seq DESC LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![limit, offset], row_to_event)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Recalcula a cadeia de hash inteira.
    pub fn verify_integrity(&self) -> Result<IntegrityReport> {
        let mut stmt = self
            .conn
            .prepare(&format!("{SELECT_EVENT} ORDER BY seq ASC"))?;
        let events: Vec<Event> = stmt
            .query_map([], row_to_event)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(match ledger::verify_chain(&events) {
            Ok(n) => IntegrityReport {
                ok: true,
                events_checked: n,
                problem: None,
            },
            Err(
                e @ (ChainError::SequenceGap { .. }
                | ChainError::BrokenLink { .. }
                | ChainError::Tampered { .. }),
            ) => IntegrityReport {
                ok: false,
                events_checked: events.len(),
                problem: Some(e.to_string()),
            },
        })
    }

    /// Registra a versão do app na primeira vez em que ela abre o cofre.
    /// Retorna `true` quando a versão é nova.
    pub fn record_app_version(&mut self, changelog: &str) -> Result<bool> {
        let version = self.app_version.clone();
        let known: Option<String> = self
            .conn
            .query_row(
                "SELECT version FROM app_version_history WHERE version = ?1",
                [&version],
                |r| r.get(0),
            )
            .optional()?;
        if known.is_some() {
            return Ok(false);
        }
        self.conn.execute(
            "INSERT INTO app_version_history (version, installed_at, changelog) VALUES (?1, ?2, ?3)",
            params![version, now_iso(), changelog],
        )?;
        let after =
            serde_json::json!({ "evento": "versão instalada", "versao": version }).to_string();
        self.append_event(
            "SISTEMA",
            "versao",
            Action::Criada,
            None,
            Some(&after),
            None,
        )?;
        Ok(true)
    }

    pub fn version_history(&self) -> Result<Vec<VersionEntry>> {
        let mut stmt = self
            .conn
            .prepare("SELECT version, installed_at, changelog FROM app_version_history ORDER BY installed_at DESC, version DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(VersionEntry {
                version: r.get(0)?,
                installed_at: r.get(1)?,
                changelog: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    #[cfg(test)]
    fn raw(&self) -> &Connection {
        &self.conn
    }
}

const SELECT_EVENT: &str = "SELECT seq, occurred_at, entity, entity_id, action, before_json, after_json, reason, app_version, prev_hash, hash FROM event_log";

fn row_to_event(r: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    let action: String = r.get(4)?;
    Ok(Event {
        seq: r.get(0)?,
        data: NewEvent {
            occurred_at: r.get(1)?,
            entity: r.get(2)?,
            entity_id: r.get(3)?,
            action: Action::parse(&action).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    format!("ação desconhecida: {action}").into(),
                )
            })?,
            before: r.get(5)?,
            after: r.get(6)?,
            reason: r.get(7)?,
            app_version: r.get(8)?,
        },
        prev_hash: r.get(9)?,
        hash: r.get(10)?,
    })
}

fn last_event(conn: &Connection) -> Result<Option<Event>> {
    Ok(conn
        .query_row(
            &format!("{SELECT_EVENT} ORDER BY seq DESC LIMIT 1"),
            [],
            row_to_event,
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENHA: &str = "senha-de-teste-123";

    fn create(dir: &Path) -> Vault {
        Vault::create_with_params(dir, SENHA, "0.1.0", KdfParams::fast_for_tests()).unwrap()
    }

    #[test]
    fn cria_reabre_e_recusa_senha_errada() {
        let tmp = tempfile::tempdir().unwrap();
        drop(create(tmp.path()));
        assert!(matches!(
            Vault::open(tmp.path(), "outra-senha-qualquer", "0.1.0"),
            Err(VaultError::WrongPassword)
        ));
        let v = Vault::open(tmp.path(), SENHA, "0.1.0").unwrap();
        assert_eq!(v.list_events(10, 0).unwrap().len(), 1);
    }

    #[test]
    fn arquivo_do_banco_nao_e_sqlite_legivel() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = create(tmp.path());
        v.append_event(
            "OPERACAO",
            "op-1",
            Action::Criada,
            None,
            Some(r#"{"ativo":"TESTE3"}"#),
            None,
        )
        .unwrap();
        drop(v);
        let bytes = fs::read(tmp.path().join(DB_FILE)).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3"));
        assert!(!bytes.windows(6).any(|w| w == b"TESTE3"));
    }

    #[test]
    fn nao_cria_sobre_cofre_existente_nem_com_senha_curta() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(matches!(
            Vault::create_with_params(tmp.path(), "curta", "0.1.0", KdfParams::fast_for_tests()),
            Err(VaultError::WeakPassword(_))
        ));
        drop(create(tmp.path()));
        assert!(matches!(
            Vault::create_with_params(tmp.path(), SENHA, "0.1.0", KdfParams::fast_for_tests()),
            Err(VaultError::AlreadyExists)
        ));
    }

    #[test]
    fn log_e_somente_inclusao_e_integro() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = create(tmp.path());
        for i in 0..5 {
            v.append_event(
                "OPERACAO",
                &format!("op-{i}"),
                Action::Criada,
                None,
                Some("{}"),
                None,
            )
            .unwrap();
        }
        assert!(v
            .raw()
            .execute("UPDATE event_log SET reason = 'x' WHERE seq = 2", [])
            .is_err());
        assert!(v
            .raw()
            .execute("DELETE FROM event_log WHERE seq = 2", [])
            .is_err());
        let report = v.verify_integrity().unwrap();
        assert!(report.ok);
        assert_eq!(report.events_checked, 6);
    }

    #[test]
    fn adulteracao_por_fora_e_detectada() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = create(tmp.path());
        v.append_event(
            "OPERACAO",
            "op-1",
            Action::Criada,
            None,
            Some(r#"{"quantidade":"10"}"#),
            None,
        )
        .unwrap();
        v.append_event(
            "OPERACAO",
            "op-2",
            Action::Criada,
            None,
            Some(r#"{"quantidade":"20"}"#),
            None,
        )
        .unwrap();
        // Simula alguém que tem a senha e edita o banco com outra ferramenta.
        v.raw()
            .execute_batch("DROP TRIGGER event_log_no_update")
            .unwrap();
        v.raw()
            .execute(
                "UPDATE event_log SET after_json = '{\"quantidade\":\"99\"}' WHERE seq = 2",
                [],
            )
            .unwrap();
        let report = v.verify_integrity().unwrap();
        assert!(!report.ok);
        assert!(report.problem.unwrap().contains("evento 2"));
    }

    #[test]
    fn registra_cada_versao_uma_vez() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = create(tmp.path());
        assert!(v.record_app_version("primeira versão").unwrap());
        assert!(!v.record_app_version("primeira versão").unwrap());
        drop(v);
        let mut v = Vault::open(tmp.path(), SENHA, "0.2.0").unwrap();
        assert!(v.record_app_version("segunda versão").unwrap());
        let hist = v.version_history().unwrap();
        assert_eq!(hist.len(), 2);
        assert!(v.verify_integrity().unwrap().ok);
    }

    #[test]
    fn senha_com_caracteres_especiais_e_acentos() {
        let tmp = tempfile::tempdir().unwrap();
        let senha = "Ab$1çÃo €\"'%^&*()";
        drop(
            Vault::create_with_params(tmp.path(), senha, "0.1.0", KdfParams::fast_for_tests())
                .unwrap(),
        );
        assert!(Vault::open(tmp.path(), senha, "0.1.0").is_ok());
        assert!(matches!(
            Vault::open(tmp.path(), "Ab$1çÃo €", "0.1.0"),
            Err(VaultError::WrongPassword)
        ));
    }

    #[test]
    fn troca_de_senha() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = create(tmp.path());
        assert!(matches!(
            v.change_password("errada-errada", "nova-senha-456"),
            Err(VaultError::WrongPassword)
        ));
        v.change_password(SENHA, "nova-senha-456").unwrap();
        drop(v);
        assert!(matches!(
            Vault::open(tmp.path(), SENHA, "0.1.0"),
            Err(VaultError::WrongPassword)
        ));
        let v = Vault::open(tmp.path(), "nova-senha-456", "0.1.0").unwrap();
        assert!(v.verify_integrity().unwrap().ok);
    }

    #[test]
    fn chave_de_recuperacao_redefine_a_senha() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = create(tmp.path());
        assert!(!v.recovery_status().unwrap().configured);
        assert!(v.setup_recovery(Some("nao e email")).is_err());
        let rk = v.setup_recovery(Some("eu@exemplo.com")).unwrap();
        let st = v.recovery_status().unwrap();
        assert!(st.configured);
        assert_eq!(st.email.as_deref(), Some("eu@exemplo.com"));
        // Trocar a senha não invalida a chave de recuperação.
        v.change_password(SENHA, "segunda-senha-2").unwrap();
        drop(v);
        assert!(Vault::has_recovery_key(tmp.path()));
        assert!(matches!(
            Vault::reset_password(tmp.path(), "AAAA-AAAA", "terceira-senha-3", "0.1.0"),
            Err(VaultError::WrongRecoveryKey)
        ));
        let v = Vault::reset_password(tmp.path(), &rk.to_lowercase(), "terceira-senha-3", "0.1.0")
            .unwrap();
        assert!(v.verify_integrity().unwrap().ok);
        drop(v);
        assert!(matches!(
            Vault::open(tmp.path(), "segunda-senha-2", "0.1.0"),
            Err(VaultError::WrongPassword)
        ));
        let mut v = Vault::open(tmp.path(), "terceira-senha-3", "0.1.0").unwrap();
        // Uma chave nova substitui a anterior.
        let rk2 = v.setup_recovery(None).unwrap();
        assert_eq!(v.recovery_status().unwrap().email, None);
        drop(v);
        assert!(Vault::reset_password(tmp.path(), &rk, "quarta-senha-44", "0.1.0").is_err());
        assert!(Vault::reset_password(tmp.path(), &rk2, "quarta-senha-44", "0.1.0").is_ok());
    }

    #[test]
    fn cofre_sem_chave_de_recuperacao_avisa() {
        let tmp = tempfile::tempdir().unwrap();
        drop(create(tmp.path()));
        assert!(!Vault::has_recovery_key(tmp.path()));
        assert!(matches!(
            Vault::reset_password(tmp.path(), "AAAA", "nova-senha-123", "0.1.0"),
            Err(VaultError::NoRecoveryKey)
        ));
    }

    /// Cofre criado pela versão 0.3.0 (chave do banco = Argon2id da senha).
    #[test]
    fn converte_cofre_do_formato_antigo_sem_perder_dados() {
        let tmp = tempfile::tempdir().unwrap();
        let params = KdfParams::fast_for_tests();
        let salt = [7u8; 16];
        let key = kdf::derive(params, SENHA, &salt).unwrap();
        let conn = Connection::open(tmp.path().join(DB_FILE)).unwrap();
        apply_key(&conn, key.as_slice()).unwrap();
        migrations::run(&conn).unwrap();
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES ('x', 'antigo', '')",
            [],
        )
        .unwrap();
        drop(conn);
        let header = serde_json::json!({
            "formato": 1, "kdf": "argon2id",
            "memory_kib": params.memory_kib, "iterations": params.iterations,
            "parallelism": params.parallelism, "sal": hex::encode(salt),
        });
        fs::write(tmp.path().join(VAULT_FILE), header.to_string()).unwrap();

        assert!(matches!(
            Vault::open(tmp.path(), "senha-errada-9", "0.4.0"),
            Err(VaultError::WrongPassword)
        ));
        let mut v = Vault::open(tmp.path(), SENHA, "0.4.0").unwrap();
        assert_eq!(v.setting("x").unwrap().as_deref(), Some("antigo"));
        let text = fs::read_to_string(tmp.path().join(VAULT_FILE)).unwrap();
        assert!(text.contains("\"formato\": 2"), "{text}");
        let rk = v.setup_recovery(None).unwrap();
        drop(v);
        assert!(Vault::open(tmp.path(), SENHA, "0.4.0").is_ok());
        let v = Vault::reset_password(tmp.path(), &rk, "nova-senha-123", "0.4.0").unwrap();
        assert_eq!(v.setting("x").unwrap().as_deref(), Some("antigo"));
    }

    #[test]
    fn confere_formato_de_email() {
        assert!(looks_like_email("fulano.tal@gmail.com"));
        for e in [
            "",
            "fulano",
            "fulano@",
            "@gmail.com",
            "a b@c.com",
            "a@b",
            "a@b.",
            "a@@b.com",
        ] {
            assert!(!looks_like_email(e), "{e}");
        }
    }
}
