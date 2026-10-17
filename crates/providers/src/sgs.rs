//! Séries do Sistema Gerenciador de Séries Temporais (SGS) do Banco Central:
//! CDI (taxa diária, série 12) e IPCA (variação mensal, série 433).
//!
//! Os valores vêm em percentual (0,040168 = 0,040168% no dia).

use std::str::FromStr;

use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{http, ProviderError, Result};

pub const SOURCE: &str = "BCB-SGS";
pub const CDI: u32 = 12;
pub const IPCA: u32 = 433;

/// O SGS recusa consultas de séries diárias acima de 10 anos.
const MAX_SPAN_DAYS: i64 = 3650;

pub fn url(code: u32, from: NaiveDate, to: NaiveDate) -> String {
    format!(
        "https://api.bcb.gov.br/dados/serie/bcdata.sgs.{code}/dados?formato=json&dataInicial={}&dataFinal={}",
        from.format("%d/%m/%Y"),
        to.format("%d/%m/%Y")
    )
}

#[derive(Deserialize)]
struct Row {
    data: String,
    valor: String,
}

/// Interpreta a resposta: (data, valor em %) em ordem de data.
pub fn parse(json: &[u8]) -> Result<Vec<(NaiveDate, Decimal)>> {
    let body = json.trim_ascii();
    // Sem dados no intervalo, o SGS responde com um objeto de erro ou com o
    // corpo vazio (comum no IPCA, que só tem um valor por mês).
    if body.is_empty() || body.starts_with(b"{") {
        return Ok(Vec::new());
    }
    // Quando está instável, o Banco Central devolve uma página HTML.
    if body.starts_with(b"<") {
        return Err(ProviderError::Network(
            "o Banco Central está instável no momento; tente de novo mais tarde".into(),
        ));
    }
    let rows: Vec<Row> =
        serde_json::from_slice(json).map_err(|e| ProviderError::Format(e.to_string()))?;
    let mut out = rows
        .into_iter()
        .map(|r| {
            let date = NaiveDate::parse_from_str(&r.data, "%d/%m/%Y")
                .map_err(|e| ProviderError::Format(format!("data {}: {e}", r.data)))?;
            let value = Decimal::from_str(r.valor.trim())
                .map_err(|e| ProviderError::Format(format!("valor {}: {e}", r.valor)))?;
            Ok((date, value))
        })
        .collect::<Result<Vec<_>>>()?;
    out.sort_by_key(|(d, _)| *d);
    out.dedup_by_key(|(d, _)| *d);
    Ok(out)
}

pub fn fetch(code: u32, from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, Decimal)>> {
    let mut out = Vec::new();
    let mut start = from;
    while start <= to {
        let end = (start + Duration::days(MAX_SPAN_DAYS)).min(to);
        let link = url(code, start, end);
        // Uma segunda tentativa resolve a maioria das instabilidades do SGS.
        let values = match http::get_bytes(&link).and_then(|b| parse(&b)) {
            Ok(v) => v,
            Err(ProviderError::NotFound(u)) => return Err(ProviderError::NotFound(u)),
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_secs(2));
                parse(&http::get_bytes(&link)?)?
            }
        };
        out.extend(values);
        start = end + Duration::days(1);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn corpo_vazio_e_pagina_de_erro() {
        assert!(parse(b"").unwrap().is_empty());
        assert!(parse(b"  \r\n").unwrap().is_empty());
        assert!(parse(br#"{"erro":"sem dados"}"#).unwrap().is_empty());
        assert!(matches!(
            parse(b"<html><body>erro</body></html>"),
            Err(ProviderError::Network(_))
        ));
    }

    #[test]
    fn interpreta_serie() {
        let json = br#"[{"data":"02/03/2026","valor":"0.055131"},{"data":"03/03/2026","valor":"0.055131"}]"#;
        let s = parse(json).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(
            s[0],
            (NaiveDate::from_ymd_opt(2026, 3, 2).unwrap(), dec!(0.055131))
        );
    }

    #[test]
    fn sem_dados_no_periodo() {
        let json = br#"{"error":"Value(s) not found","message":"..."}"#;
        assert!(parse(json).unwrap().is_empty());
    }

    #[test]
    fn endereco() {
        let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
        assert!(url(CDI, d(1, 2), d(3, 4)).ends_with(
            "bcdata.sgs.12/dados?formato=json&dataInicial=02/01/2026&dataFinal=04/03/2026"
        ));
    }
}
