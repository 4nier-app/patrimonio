//! Cadastro das empresas listadas na B3 (o mesmo do site da B3): a atividade
//! principal e a classificação setorial, em português.
//!
//! O serviço recebe os parâmetros em JSON codificado em Base64 no próprio
//! endereço. Primeiro acha a empresa pelo código de negociação (as 4 letras de
//! PETR4) e pega o código CVM; depois busca o detalhe.
//!
//! A B3 troca as vírgulas dos textos por pontos ("Pesquisa. lavra. refinação");
//! `virgulas` desfaz isso.

use serde::Deserialize;

use crate::{http, ProviderError, Result};

pub const SOURCE: &str = "B3 (cadastro das empresas listadas)";

const BASE: &str = "https://sistemaswebb3-listados.b3.com.br/listedCompaniesProxy/CompanyCall";

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Company {
    /// Nome de pregão (PETROBRAS).
    pub trading_name: String,
    /// Razão social.
    pub company_name: String,
    /// Setor / subsetor / segmento.
    pub classification: Option<String>,
    pub activity: Option<String>,
    pub website: Option<String>,
}

fn base64(input: &str) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let b = input.as_bytes();
    let mut out = String::new();
    for chunk in b.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Código da empresa (as letras antes do número): PETR4 → PETR.
pub fn issuer_of(ticker: &str) -> Option<String> {
    let t = ticker.trim().to_uppercase();
    let letters: String = t.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    (letters.len() == 4 && t.len() > 4 && t[4..].chars().all(|c| c.is_ascii_digit()))
        .then_some(letters)
}

pub fn search_url(issuer: &str) -> String {
    let q = format!(r#"{{"language":"pt-br","pageNumber":1,"pageSize":60,"company":"{issuer}"}}"#);
    format!("{BASE}/GetInitialCompanies/{}", base64(&q))
}

pub fn detail_url(code_cvm: &str) -> String {
    let q = format!(r#"{{"codeCVM":"{code_cvm}","language":"pt-br"}}"#);
    format!("{BASE}/GetDetail/{}", base64(&q))
}

#[derive(Deserialize)]
struct Search {
    #[serde(default)]
    results: Vec<SearchRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchRow {
    #[serde(default, rename = "codeCVM")]
    code_cvm: String,
    #[serde(default)]
    issuing_company: String,
}

/// Código CVM da empresa com esse código de negociação.
pub fn parse_search(json: &[u8], issuer: &str) -> Result<Option<String>> {
    let s: Search =
        serde_json::from_slice(json).map_err(|e| ProviderError::Format(e.to_string()))?;
    Ok(s.results
        .into_iter()
        .find(|r| r.issuing_company.eq_ignore_ascii_case(issuer) && !r.code_cvm.is_empty())
        .map(|r| r.code_cvm))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Detail {
    #[serde(default)]
    trading_name: String,
    #[serde(default)]
    company_name: String,
    industry_classification: Option<String>,
    activity: Option<String>,
    website: Option<String>,
}

/// Desfaz a troca de vírgulas por pontos: ". " seguido de minúscula volta a
/// ser ", ". Na classificação (sem frases), todo ". " é vírgula.
pub fn virgulas(text: &str, always: bool) -> String {
    let chars: Vec<char> = text.trim().chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '.' && chars.get(i + 1) == Some(&' ') {
            let next = chars.get(i + 2).copied().unwrap_or(' ');
            if always || next.is_lowercase() {
                out.push(',');
                i += 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

pub fn parse_detail(json: &[u8]) -> Result<Company> {
    let d: Detail =
        serde_json::from_slice(json).map_err(|e| ProviderError::Format(e.to_string()))?;
    if d.trading_name.trim().is_empty() && d.company_name.trim().is_empty() {
        return Err(ProviderError::NotFound("empresa".into()));
    }
    Ok(Company {
        trading_name: d.trading_name.trim().to_string(),
        company_name: d.company_name.trim().to_string(),
        classification: clean(d.industry_classification).map(|c| virgulas(&c, true)),
        activity: clean(d.activity).map(|a| virgulas(&a, false)),
        website: clean(d.website),
    })
}

/// Atividade e setor da empresa do código de negociação (PETR4), ou `None`
/// quando não é uma empresa listada (FII, ETF, BDR...).
pub fn fetch(ticker: &str) -> Result<Option<Company>> {
    let Some(issuer) = issuer_of(ticker) else {
        return Ok(None);
    };
    let Some(cvm) = parse_search(&http::get_bytes(&search_url(&issuer))?, &issuer)? else {
        return Ok(None);
    };
    parse_detail(&http::get_bytes(&detail_url(&cvm))?).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enderecos_e_codigo() {
        assert_eq!(
            base64(r#"{"codeCVM":"9512","language":"pt-br"}"#),
            "eyJjb2RlQ1ZNIjoiOTUxMiIsImxhbmd1YWdlIjoicHQtYnIifQ=="
        );
        assert_eq!(base64("ab"), "YWI=");
        assert_eq!(issuer_of("ABCD4").as_deref(), Some("ABCD"));
        assert_eq!(issuer_of("ABCD11").as_deref(), Some("ABCD"));
        assert_eq!(issuer_of("VOO"), None);
        assert_eq!(issuer_of("CSPX.L"), None);
    }

    #[test]
    fn interpreta_busca_e_detalhe() {
        // Respostas sintéticas no formato do serviço.
        let search = br#"{"page":{},"results":[{"codeCVM":"1","issuingCompany":"ABCE"},{"codeCVM":"42","issuingCompany":"ABCD"}]}"#;
        assert_eq!(parse_search(search, "ABCD").unwrap().as_deref(), Some("42"));
        assert_eq!(parse_search(search, "ZZZZ").unwrap(), None);
        let detail = r#"{"issuingCompany":"ABCD","companyName":"EMPRESA EXEMPLO S.A.","tradingName":"EXEMPLO",
            "industryClassification":"Bens Industriais. Serviços / Transporte / Rodoviário",
            "activity":"Fabricação. venda e manutenção de peças. Atua também no exterior.","website":"www.exemplo.com.br"}"#;
        let c = parse_detail(detail.as_bytes()).unwrap();
        assert_eq!(c.trading_name, "EXEMPLO");
        assert_eq!(
            c.classification.as_deref(),
            Some("Bens Industriais, Serviços / Transporte / Rodoviário")
        );
        assert_eq!(
            c.activity.as_deref(),
            Some("Fabricação, venda e manutenção de peças. Atua também no exterior.")
        );
    }
}
