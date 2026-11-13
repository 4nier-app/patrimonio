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

/// Resposta com o código HTTP e os cookies que o servidor mandou guardar.
pub struct Reply {
    pub status: u16,
    pub cookies: Vec<String>,
    pub body: Vec<u8>,
}

/// Download que envia um cookie e devolve os que o servidor criou (para
/// serviços que pedem um cookie de sessão anônima). Não trata o código HTTP
/// como erro: quem chama decide.
pub fn get_reply(url: &str, cookie: Option<&str>) -> Result<Reply> {
    let mut req = agent().get(url);
    if let Some(c) = cookie {
        req = req.header("Cookie", c);
    }
    let mut resp = req
        .call()
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    let status = resp.status().as_u16();
    let cookies = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|v| v.split(';').next())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    let mut body = Vec::new();
    resp.body_mut()
        .as_reader()
        .take(MAX_BYTES)
        .read_to_end(&mut body)
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    Ok(Reply {
        status,
        cookies,
        body,
    })
}
