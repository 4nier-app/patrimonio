//! Download simples com tempo limite.

use std::io::Read;
use std::time::Duration;

use crate::{ProviderError, Result};

// Alguns serviços recusam clientes sem um identificador de navegador.
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) CarteiraAuditavel/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/4nier-app/patrimonio)"
);
const MAX_BYTES: u64 = 200 * 1024 * 1024;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

pub fn get_bytes(url: &str) -> Result<Vec<u8>> {
    let mut resp = agent()
        .get(url)
        .call()
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    match resp.status().as_u16() {
        200 => {}
        404 => return Err(ProviderError::NotFound(url.to_string())),
        s => return Err(ProviderError::Network(format!("HTTP {s} em {url}"))),
    }
    let mut buf = Vec::new();
    resp.body_mut()
        .as_reader()
        .take(MAX_BYTES)
        .read_to_end(&mut buf)
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    Ok(buf)
}
