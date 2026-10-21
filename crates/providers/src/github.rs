//! Versão mais recente publicada no GitHub, para avisar quando há atualização.
//! A consulta só pergunta qual é a última versão; nenhum dado do cofre sai.

use serde::Deserialize;

use crate::{http, ProviderError, Result};

/// Página onde a pessoa baixa a versão nova.
pub const RELEASES_PAGE: &str =
    "https://github.com/marcelo-fournier/4nier-patrimonio/releases/latest";
const LATEST_API: &str =
    "https://api.github.com/repos/marcelo-fournier/4nier-patrimonio/releases/latest";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

/// Última versão publicada (sem rascunhos nem pré-lançamentos), como "0.15.3".
pub fn latest_version() -> Result<String> {
    parse(&http::get_bytes(LATEST_API)?)
}

fn parse(bytes: &[u8]) -> Result<String> {
    let release: Release =
        serde_json::from_slice(bytes).map_err(|e| ProviderError::Format(e.to_string()))?;
    Ok(release.tag_name.trim().trim_start_matches('v').to_string())
}

fn numbers(version: &str) -> Vec<u64> {
    version
        .trim_start_matches('v')
        .split('.')
        .map(|p| {
            p.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

/// `latest` é mais nova que `current`? Compara número a número (0.15.10 > 0.15.9).
pub fn is_newer(latest: &str, current: &str) -> bool {
    let (mut a, mut b) = (numbers(latest), numbers(current));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_a_versao_da_resposta_do_github() {
        let json = br#"{"tag_name":"v0.15.3","name":"0.15.3","draft":false}"#;
        assert_eq!(parse(json).unwrap(), "0.15.3");
    }

    #[test]
    fn compara_versoes_numero_a_numero() {
        assert!(is_newer("0.15.3", "0.15.2"));
        assert!(is_newer("0.15.10", "0.15.9"));
        assert!(is_newer("1.0.0", "0.15.3"));
        assert!(!is_newer("0.15.3", "0.15.3"));
        assert!(!is_newer("0.15.2", "0.15.3"));
        assert!(!is_newer("v0.15.3", "0.15.3"));
    }
}
