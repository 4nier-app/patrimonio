//! Backup do cofre num arquivo único.
//!
//! O arquivo é um JSON com um manifesto (versão do app e do esquema, data,
//! número de eventos, último hash da cadeia e SHA-256 do banco), o
//! `cofre.json` e o banco em base64. O banco continua criptografado: o backup
//! abre com a mesma senha (ou chave de recuperação) do momento em que foi feito.

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use carteira_ledger::Action;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::kdf::VaultHeader;
use crate::{
    apply_key, last_event, migrations, now_iso, Result, Vault, VaultError, DB_FILE, VAULT_FILE,
};

pub const BACKUP_EXTENSION: &str = "cabackup";
const APP_NAME: &str = "4Nier Patrimônio Auditável";
pub(crate) const AUTO_DIR: &str = "backups";
pub(crate) const PREVIOUS_DIR: &str = "anteriores";

#[derive(Serialize, Deserialize)]
struct BackupFile {
    formato: u32,
    aplicativo: String,
    versao_app: String,
    versao_esquema: i64,
    criado_em: String,
    eventos: i64,
    ultimo_hash: String,
    sha256_banco: String,
    cofre: VaultHeader,
    banco: String,
}

/// Resultado de um backup gravado.
#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    pub path: String,
    pub created_at: String,
    pub events: i64,
    pub bytes: u64,
}

/// O que dá para saber de um arquivo de backup sem a senha.
#[derive(Debug, Clone, Serialize)]
pub struct BackupSummary {
    pub path: String,
    pub created_at: String,
    pub app_version: String,
    pub schema_version: i64,
    pub events: i64,
    pub has_recovery_key: bool,
}

fn read_file(path: &Path) -> Result<BackupFile> {
    let text = fs::read_to_string(path)?;
    let file: BackupFile = serde_json::from_str(&text)
        .map_err(|_| VaultError::Invalid("este arquivo não é um backup do 4Nier".into()))?;
    if file.formato != 1 {
        return Err(VaultError::Invalid(format!(
            "formato de backup {} não suportado; atualize o aplicativo",
            file.formato
        )));
    }
    Ok(file)
}

/// Lê o manifesto de um backup, sem abrir o banco.
pub fn summary(path: &Path) -> Result<BackupSummary> {
    let f = read_file(path)?;
    Ok(BackupSummary {
        path: path.display().to_string(),
        created_at: f.criado_em,
        app_version: f.versao_app,
        schema_version: f.versao_esquema,
        events: f.eventos,
        has_recovery_key: f.cofre.has_recovery(),
    })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

impl Vault {
    /// Grava um backup completo do cofre em `path`.
    pub fn export_backup(&mut self, path: &Path) -> Result<BackupInfo> {
        // Cópia consistente do banco, criptografada com a mesma chave.
        let tmp = self.dir.join("backup-em-andamento.db");
        if tmp.exists() {
            fs::remove_file(&tmp)?;
        }
        {
            let mut dst = Connection::open(&tmp)?;
            apply_key(&dst, self.data_key.as_slice())?;
            let backup = rusqlite::backup::Backup::new(&self.conn, &mut dst)?;
            backup.run_to_completion(256, std::time::Duration::ZERO, None)?;
        }
        let db = fs::read(&tmp);
        let _ = fs::remove_file(&tmp);
        let db = db?;

        let last = last_event(&self.conn)?;
        let created_at = now_iso();
        let file = BackupFile {
            formato: 1,
            aplicativo: APP_NAME.into(),
            versao_app: self.app_version.clone(),
            versao_esquema: migrations::current(&self.conn)?,
            criado_em: created_at.clone(),
            eventos: last.as_ref().map(|e| e.seq).unwrap_or(0),
            ultimo_hash: last.map(|e| e.hash).unwrap_or_default(),
            sha256_banco: hex::encode(Sha256::digest(&db)),
            cofre: VaultHeader::read(&self.dir.join(VAULT_FILE))?,
            banco: base64::engine::general_purpose::STANDARD.encode(&db),
        };
        let json =
            serde_json::to_vec_pretty(&file).map_err(|e| VaultError::Invalid(e.to_string()))?;
        write_atomic(path, &json)?;

        let info = BackupInfo {
            path: path.display().to_string(),
            created_at,
            events: file.eventos,
            bytes: json.len() as u64,
        };
        let after = serde_json::json!({
            "evento": "backup exportado",
            "arquivo": info.path,
            "eventos": info.events,
        })
        .to_string();
        self.append_event(
            "SISTEMA",
            "backup",
            Action::Criada,
            None,
            Some(&after),
            None,
        )?;
        self.set_setting("backup.ultimo", &info.created_at)?;
        Ok(info)
    }

    /// Faz um backup automático na pasta `backups` do cofre quando o último
    /// tem mais de `max_age_days` dias, e guarda só os `keep` mais recentes.
    pub fn auto_backup(&mut self, max_age_days: i64, keep: usize) -> Result<Option<BackupInfo>> {
        let dir = self.dir.join(AUTO_DIR);
        let mut existing = list_auto(&dir)?;
        let today = chrono::Local::now().date_naive();
        let newest = existing.last().and_then(|p| auto_date(p));
        if newest.is_some_and(|d| (today - d).num_days() < max_age_days) {
            return Ok(None);
        }
        let path = dir.join(format!("automatico-{today}.{BACKUP_EXTENSION}"));
        let info = self.export_backup(&path)?;
        existing.push(path);
        existing.sort();
        existing.dedup();
        while existing.len() > keep {
            let _ = fs::remove_file(existing.remove(0));
        }
        Ok(Some(info))
    }

    /// Pasta dos backups automáticos.
    pub fn auto_backup_dir(&self) -> PathBuf {
        self.dir.join(AUTO_DIR)
    }

    /// Data do backup automático mais recente.
    pub fn last_auto_backup(&self) -> Result<Option<chrono::NaiveDate>> {
        Ok(list_auto(&self.auto_backup_dir())?
            .last()
            .and_then(|p| auto_date(p)))
    }

    /// Restaura um backup na pasta `dir` (prepara e aplica de uma vez). O
    /// cofre atual precisa estar fechado.
    pub fn restore_backup(
        dir: &Path,
        backup: &Path,
        password: &str,
        app_version: &str,
    ) -> Result<(Vault, Option<PathBuf>)> {
        Self::prepare_restore(dir, backup, password, app_version)?.apply()
    }

    /// Primeira parte da restauração: abre e confere o backup (senha, versão
    /// do esquema e cadeia de hash) numa pasta temporária, sem tocar no cofre
    /// atual, que pode continuar aberto.
    pub fn prepare_restore(
        dir: &Path,
        backup: &Path,
        password: &str,
        app_version: &str,
    ) -> Result<PreparedRestore> {
        let file = read_file(backup)?;
        if file.versao_esquema > migrations::latest() {
            return Err(VaultError::Invalid(format!(
                "este backup foi feito na versão {} do aplicativo, mais nova que esta; atualize o aplicativo antes de restaurar",
                file.versao_app
            )));
        }
        let db = base64::engine::general_purpose::STANDARD
            .decode(file.banco.as_bytes())
            .map_err(|_| VaultError::Invalid("backup corrompido (banco ilegível)".into()))?;
        if hex::encode(Sha256::digest(&db)) != file.sha256_banco {
            return Err(VaultError::Invalid(
                "backup corrompido: o conteúdo não confere com o hash do manifesto".into(),
            ));
        }

        fs::create_dir_all(dir)?;
        let staging = dir.join("restauracao-em-andamento");
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir_all(&staging)?;
        let checked = (|| -> Result<()> {
            fs::write(staging.join(DB_FILE), &db)?;
            file.cofre.write(&staging.join(VAULT_FILE))?;
            let v = Vault::open(&staging, password, app_version)?;
            let report = v.verify_integrity()?;
            if !report.ok {
                return Err(VaultError::Invalid(format!(
                    "o histórico do backup não passou na conferência: {}",
                    report.problem.unwrap_or_default()
                )));
            }
            Ok(())
        })();
        if let Err(e) = checked {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
        Ok(PreparedRestore {
            dir: dir.to_path_buf(),
            staging,
            backup: backup.to_path_buf(),
            created_at: file.criado_em,
            password: zeroize::Zeroizing::new(password.to_string()),
            app_version: app_version.to_string(),
        })
    }
}

/// Backup conferido, pronto para substituir o cofre atual.
pub struct PreparedRestore {
    dir: PathBuf,
    staging: PathBuf,
    backup: PathBuf,
    created_at: String,
    password: zeroize::Zeroizing<String>,
    app_version: String,
}

impl PreparedRestore {
    /// Segunda parte: guarda o cofre atual em `anteriores/<data-hora>` e põe o
    /// backup no lugar. O cofre atual precisa estar fechado. Retorna o cofre
    /// restaurado, já aberto, e a pasta onde o anterior ficou guardado.
    pub fn apply(self) -> Result<(Vault, Option<PathBuf>)> {
        let dir = &self.dir;
        let mut previous = None;
        if dir.join(DB_FILE).exists() || dir.join(VAULT_FILE).exists() {
            let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string();
            let keep = dir.join(PREVIOUS_DIR).join(stamp);
            fs::create_dir_all(&keep)?;
            for name in [DB_FILE, VAULT_FILE] {
                if dir.join(name).exists() {
                    fs::rename(dir.join(name), keep.join(name))?;
                }
            }
            previous = Some(keep);
        }
        for name in [DB_FILE, VAULT_FILE] {
            fs::rename(self.staging.join(name), dir.join(name))?;
        }
        let _ = fs::remove_dir_all(&self.staging);

        let mut vault = Vault::open(dir, &self.password, &self.app_version)?;
        let after = serde_json::json!({
            "evento": "backup restaurado",
            "arquivo": self.backup.display().to_string(),
            "backup_criado_em": self.created_at,
            "cofre_anterior_guardado_em": previous.as_ref().map(|p| p.display().to_string()),
        })
        .to_string();
        vault.append_event(
            "SISTEMA",
            "backup",
            Action::Alterada,
            None,
            Some(&after),
            None,
        )?;
        Ok((vault, previous))
    }
}

fn list_auto(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| auto_date(p).is_some())
        .collect();
    files.sort();
    Ok(files)
}

fn auto_date(path: &Path) -> Option<chrono::NaiveDate> {
    let name = path.file_name()?.to_str()?;
    let date = name
        .strip_prefix("automatico-")?
        .strip_suffix(&format!(".{BACKUP_EXTENSION}"))?;
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KdfParams;

    const SENHA: &str = "senha-de-teste-123";

    fn vault(dir: &Path) -> Vault {
        Vault::create_with_params(dir, SENHA, "0.4.0", KdfParams::fast_for_tests()).unwrap()
    }

    fn add_events(v: &mut Vault, n: usize) {
        for i in 0..n {
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
    }

    #[test]
    fn exporta_e_restaura_em_outro_computador() {
        let a = tempfile::tempdir().unwrap();
        let mut v = vault(a.path());
        add_events(&mut v, 3);
        let file = a.path().join(format!("meu.{BACKUP_EXTENSION}"));
        let info = v.export_backup(&file).unwrap();
        assert_eq!(info.events, 4);
        let text = fs::read_to_string(&file).unwrap();
        assert!(
            !text.contains("op-1"),
            "o banco segue criptografado no backup"
        );

        let s = summary(&file).unwrap();
        assert_eq!(s.events, 4);
        assert_eq!(s.app_version, "0.4.0");

        // Computador novo: pasta vazia.
        let b = tempfile::tempdir().unwrap();
        let novo = b.path().join("dados");
        assert!(matches!(
            Vault::restore_backup(&novo, &file, "senha-errada-1", "0.4.0"),
            Err(VaultError::WrongPassword)
        ));
        assert!(
            !Vault::exists(&novo),
            "senha errada não deixa nada para trás"
        );
        let (r, previous) = Vault::restore_backup(&novo, &file, SENHA, "0.4.0").unwrap();
        assert!(previous.is_none());
        assert!(r.verify_integrity().unwrap().ok);
        // Os 4 da cópia + "backup restaurado"; o "backup exportado" foi
        // registrado depois da cópia, só no cofre original.
        assert_eq!(r.list_events(100, 0).unwrap().len(), 5);
    }

    #[test]
    fn restaurar_guarda_o_cofre_atual() {
        let a = tempfile::tempdir().unwrap();
        let mut v = vault(a.path());
        let file = a.path().join(format!("b.{BACKUP_EXTENSION}"));
        v.export_backup(&file).unwrap();
        add_events(&mut v, 5);
        drop(v);
        let (r, previous) = Vault::restore_backup(a.path(), &file, SENHA, "0.4.0").unwrap();
        let previous = previous.unwrap();
        assert!(previous.join(DB_FILE).exists() && previous.join(VAULT_FILE).exists());
        assert_eq!(r.list_events(100, 0).unwrap().len(), 2);
        drop(r);
        // O anterior continua abrindo com a senha.
        let old = Vault::open(&previous, SENHA, "0.4.0").unwrap();
        assert_eq!(old.list_events(100, 0).unwrap().len(), 7);
    }

    #[test]
    fn recusa_backup_alterado_ou_de_versao_mais_nova() {
        let a = tempfile::tempdir().unwrap();
        let mut v = vault(a.path());
        let file = a.path().join(format!("b.{BACKUP_EXTENSION}"));
        v.export_backup(&file).unwrap();
        drop(v);
        let mut f: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        let original = f.clone();

        f["sha256_banco"] = "00".into();
        fs::write(&file, f.to_string()).unwrap();
        let e = Vault::restore_backup(a.path(), &file, SENHA, "0.4.0")
            .err()
            .unwrap();
        assert!(e.to_string().contains("corrompido"), "{e}");

        let mut f = original.clone();
        f["versao_esquema"] = 999.into();
        fs::write(&file, f.to_string()).unwrap();
        let e = Vault::restore_backup(a.path(), &file, SENHA, "0.4.0")
            .err()
            .unwrap();
        assert!(e.to_string().contains("mais nova"), "{e}");

        fs::write(&file, "{\"qualquer\": 1}").unwrap();
        assert!(summary(&file).is_err());
        // Nada disso mexeu no cofre atual.
        assert!(Vault::open(a.path(), SENHA, "0.4.0").is_ok());
        assert!(!a.path().join(PREVIOUS_DIR).exists());
    }

    #[test]
    fn backup_automatico_semanal_guarda_os_mais_recentes() {
        let a = tempfile::tempdir().unwrap();
        let mut v = vault(a.path());
        let dir = v.auto_backup_dir();
        fs::create_dir_all(&dir).unwrap();
        for d in ["2026-01-01", "2026-01-08", "2026-01-15"] {
            fs::write(dir.join(format!("automatico-{d}.{BACKUP_EXTENSION}")), "x").unwrap();
        }
        assert!(v.auto_backup(7, 3).unwrap().is_some());
        let left = list_auto(&dir).unwrap();
        assert_eq!(left.len(), 3);
        assert!(!dir
            .join(format!("automatico-2026-01-01.{BACKUP_EXTENSION}"))
            .exists());
        // Já tem um de hoje: não faz outro.
        assert!(v.auto_backup(7, 3).unwrap().is_none());
        assert_eq!(
            v.last_auto_backup().unwrap(),
            Some(chrono::Local::now().date_naive())
        );
    }
}
