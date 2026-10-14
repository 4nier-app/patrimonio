//! Derivação da chave do banco a partir da senha (Argon2id).

use std::fs;
use std::path::Path;

use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{Result, VaultError};

/// Parâmetros do Argon2id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memória em KiB.
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for KdfParams {
    /// 64 MiB, 3 passagens: leva por volta de meio segundo num computador comum.
    fn default() -> Self {
        KdfParams { memory_kib: 64 * 1024, iterations: 3, parallelism: 1 }
    }
}

impl KdfParams {
    /// Parâmetros leves, só para testes automatizados.
    pub fn fast_for_tests() -> Self {
        KdfParams { memory_kib: 8, iterations: 1, parallelism: 1 }
    }
}

/// Conteúdo de `cofre.json`. Não contém a senha nem a chave.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultHeader {
    pub formato: u32,
    pub kdf: String,
    #[serde(flatten)]
    pub params: KdfParams,
    /// Sal aleatório em hexadecimal.
    pub sal: String,
}

impl VaultHeader {
    pub fn new(params: KdfParams) -> Result<Self> {
        let mut salt = [0u8; 16];
        getrandom::getrandom(&mut salt).map_err(|e| VaultError::Kdf(e.to_string()))?;
        Ok(VaultHeader { formato: 1, kdf: "argon2id".into(), params, sal: hex::encode(salt) })
    }

    pub fn params(&self) -> KdfParams {
        self.params
    }

    pub fn derive_key(&self, password: &str) -> Result<Zeroizing<[u8; 32]>> {
        if self.formato != 1 || self.kdf != "argon2id" {
            return Err(VaultError::InvalidVaultFile(format!("formato {} / {} não suportado", self.formato, self.kdf)));
        }
        let salt = hex::decode(&self.sal).map_err(|e| VaultError::InvalidVaultFile(e.to_string()))?;
        let params = Params::new(self.params.memory_kib, self.params.iterations, self.params.parallelism, Some(32))
            .map_err(|e| VaultError::Kdf(e.to_string()))?;
        let mut key = Zeroizing::new([0u8; 32]);
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(password.as_bytes(), &salt, key.as_mut())
            .map_err(|e| VaultError::Kdf(e.to_string()))?;
        Ok(key)
    }

    pub fn read(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(|e| VaultError::InvalidVaultFile(e.to_string()))
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(|e| VaultError::InvalidVaultFile(e.to_string()))?;
        fs::write(path, text)?;
        Ok(())
    }
}
