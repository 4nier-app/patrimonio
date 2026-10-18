//! Chave do banco e `cofre.json`.
//!
//! Formato 2 (desde a versão 0.4.0): o banco é criptografado com uma chave
//! aleatória de 32 bytes. Essa chave é guardada duas vezes em `cofre.json`,
//! cada uma cifrada (ChaCha20-Poly1305) com uma chave derivada por Argon2id:
//! uma vez a partir da senha e, opcionalmente, outra a partir da chave de
//! recuperação. Assim a senha pode ser redefinida com a chave de recuperação
//! sem recriptografar o banco. Nenhuma das duas é gravada.
//!
//! Formato 1 (até a 0.3.0): a chave do banco era o próprio Argon2id da senha.
//! Ao abrir um cofre nesse formato, ele é convertido para o formato 2 mantendo
//! a mesma chave do banco.

use std::fs;
use std::path::Path;

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{Result, VaultError};

pub type DataKey = Zeroizing<[u8; 32]>;

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
        KdfParams {
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        }
    }
}

impl KdfParams {
    /// Parâmetros leves, só para testes automatizados.
    pub fn fast_for_tests() -> Self {
        KdfParams {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        }
    }
}

/// A chave do banco cifrada com uma chave derivada de um segredo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrappedKey {
    pub sal: String,
    pub nonce: String,
    pub chave: String,
}

/// Conteúdo de `cofre.json`. Não contém a senha, a chave de recuperação nem a
/// chave do banco em aberto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultHeader {
    pub formato: u32,
    pub kdf: String,
    #[serde(flatten)]
    pub params: KdfParams,
    /// Formato 1: sal da senha.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sal: Option<String>,
    /// Formato 2: chave do banco cifrada pela senha.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub senha: Option<WrappedKey>,
    /// Formato 2: chave do banco cifrada pela chave de recuperação.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recuperacao: Option<WrappedKey>,
}

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::getrandom(&mut bytes).map_err(|e| VaultError::Kdf(e.to_string()))?;
    Ok(bytes)
}

/// Gera uma chave aleatória para o banco.
pub fn new_data_key() -> Result<DataKey> {
    Ok(Zeroizing::new(random::<32>()?))
}

pub(crate) fn derive(params: KdfParams, secret: &str, salt: &[u8]) -> Result<DataKey> {
    let params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(32),
    )
    .map_err(|e| VaultError::Kdf(e.to_string()))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(secret.as_bytes(), salt, key.as_mut())
        .map_err(|e| VaultError::Kdf(e.to_string()))?;
    Ok(key)
}

fn invalid(e: impl ToString) -> VaultError {
    VaultError::InvalidVaultFile(e.to_string())
}

/// Rótulo autenticado junto com a chave, para que a cópia da senha não possa
/// ser trocada pela da recuperação. Não mude estes textos com o nome do
/// projeto: cofres já criados deixariam de abrir.
const AAD_SENHA: &[u8] = b"carteira-auditavel/senha";
const AAD_RECUPERACAO: &[u8] = b"carteira-auditavel/recuperacao";

fn wrap(params: KdfParams, secret: &str, aad: &[u8], data_key: &DataKey) -> Result<WrappedKey> {
    let salt = random::<16>()?;
    let nonce = random::<12>()?;
    let kek = derive(params, secret, &salt)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(kek.as_slice()));
    let sealed = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: data_key.as_slice(),
                aad,
            },
        )
        .map_err(|e| VaultError::Kdf(e.to_string()))?;
    Ok(WrappedKey {
        sal: hex::encode(salt),
        nonce: hex::encode(nonce),
        chave: hex::encode(sealed),
    })
}

/// `None` quando o segredo não abre a chave (senha ou chave de recuperação erradas).
fn unwrap(
    params: KdfParams,
    secret: &str,
    aad: &[u8],
    wrapped: &WrappedKey,
) -> Result<Option<DataKey>> {
    let salt = hex::decode(&wrapped.sal).map_err(invalid)?;
    let nonce = hex::decode(&wrapped.nonce).map_err(invalid)?;
    let sealed = hex::decode(&wrapped.chave).map_err(invalid)?;
    if nonce.len() != 12 {
        return Err(invalid("nonce com tamanho errado"));
    }
    let kek = derive(params, secret, &salt)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(kek.as_slice()));
    let Ok(plain) = cipher.decrypt(Nonce::from_slice(&nonce), Payload { msg: &sealed, aad }) else {
        return Ok(None);
    };
    let plain = Zeroizing::new(plain);
    let bytes: [u8; 32] = plain
        .as_slice()
        .try_into()
        .map_err(|_| invalid("chave do banco com tamanho errado"))?;
    Ok(Some(Zeroizing::new(bytes)))
}

impl VaultHeader {
    /// Cabeçalho novo (formato 2) com a chave do banco protegida pela senha.
    pub fn new(params: KdfParams, data_key: &DataKey, password: &str) -> Result<Self> {
        Ok(VaultHeader {
            formato: 2,
            kdf: "argon2id".into(),
            params,
            sal: None,
            senha: Some(wrap(params, password, AAD_SENHA, data_key)?),
            recuperacao: None,
        })
    }

    pub fn is_legacy(&self) -> bool {
        self.formato == 1
    }

    pub fn has_recovery(&self) -> bool {
        self.recuperacao.is_some()
    }

    /// Chave do banco a partir da senha. No formato 2, `None` significa senha
    /// errada. No formato 1 a chave sempre sai; quem confere é o SQLCipher.
    pub fn key_from_password(&self, password: &str) -> Result<Option<DataKey>> {
        if self.kdf != "argon2id" {
            return Err(invalid(format!("derivação {} não suportada", self.kdf)));
        }
        match self.formato {
            1 => {
                let salt = hex::decode(self.sal.as_deref().ok_or_else(|| invalid("sem sal"))?)
                    .map_err(invalid)?;
                Ok(Some(derive(self.params, password, &salt)?))
            }
            2 => {
                let wrapped = self.senha.as_ref().ok_or_else(|| invalid("sem senha"))?;
                unwrap(self.params, password, AAD_SENHA, wrapped)
            }
            n => Err(invalid(format!("formato {n} não suportado"))),
        }
    }

    /// Chave do banco a partir da chave de recuperação. `None` quando a chave
    /// não confere ou quando o cofre não tem chave de recuperação.
    pub fn key_from_recovery(&self, recovery_key: &str) -> Result<Option<DataKey>> {
        match (&self.formato, &self.recuperacao) {
            (2, Some(wrapped)) => unwrap(
                self.params,
                &normalize_recovery_key(recovery_key),
                AAD_RECUPERACAO,
                wrapped,
            ),
            _ => Ok(None),
        }
    }

    /// Protege a chave do banco com uma nova senha (novo sal). Converte o
    /// formato 1 para o 2.
    pub fn set_password(&mut self, data_key: &DataKey, password: &str) -> Result<()> {
        self.senha = Some(wrap(self.params, password, AAD_SENHA, data_key)?);
        self.formato = 2;
        self.sal = None;
        Ok(())
    }

    /// Protege a chave do banco com uma nova chave de recuperação; a anterior
    /// deixa de funcionar.
    pub fn set_recovery(&mut self, data_key: &DataKey, recovery_key: &str) -> Result<()> {
        self.recuperacao = Some(wrap(
            self.params,
            &normalize_recovery_key(recovery_key),
            AAD_RECUPERACAO,
            data_key,
        )?);
        Ok(())
    }

    pub fn read(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(invalid)
    }

    /// Grava num arquivo temporário e renomeia, para nunca deixar um
    /// `cofre.json` pela metade.
    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(invalid)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// Letras e números sem os que se confundem (0/O, 1/I/L).
const RECOVERY_ALPHABET: &[u8; 32] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789X";

/// Gera uma chave de recuperação: 32 caracteres (160 bits) em 8 grupos de 4.
pub fn new_recovery_key() -> Result<String> {
    let bytes = random::<32>()?;
    let chars: Vec<char> = bytes
        .iter()
        .map(|b| RECOVERY_ALPHABET[(*b & 31) as usize] as char)
        .collect();
    Ok(chars
        .chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-"))
}

/// Ignora espaços, hífens e maiúsculas/minúsculas ao digitar a chave.
pub fn normalize_recovery_key(key: &str) -> String {
    key.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn senha_e_recuperacao_abrem_a_mesma_chave() {
        let p = KdfParams::fast_for_tests();
        let dk = new_data_key().unwrap();
        let mut h = VaultHeader::new(p, &dk, "minha senha").unwrap();
        let rk = new_recovery_key().unwrap();
        h.set_recovery(&dk, &rk).unwrap();
        let h = VaultHeader::parse(&serde_json::to_string(&h).unwrap()).unwrap();
        assert_eq!(*h.key_from_password("minha senha").unwrap().unwrap(), *dk);
        assert!(h.key_from_password("outra senha").unwrap().is_none());
        let digitada = rk.to_lowercase().replace('-', " ");
        assert_eq!(*h.key_from_recovery(&digitada).unwrap().unwrap(), *dk);
        assert!(h.key_from_recovery("AAAA-BBBB").unwrap().is_none());
        // A cópia da senha não serve como cópia da recuperação.
        let mut troca = h.clone();
        troca.recuperacao = troca.senha.clone();
        assert!(troca.key_from_recovery(&rk).unwrap().is_none());
    }

    #[test]
    fn chave_de_recuperacao_tem_8_grupos_de_4() {
        let rk = new_recovery_key().unwrap();
        assert_eq!(rk.len(), 39);
        assert_eq!(rk.split('-').count(), 8);
        assert_eq!(normalize_recovery_key(&rk).len(), 32);
        assert_ne!(rk, new_recovery_key().unwrap());
    }
}
