//! Fechamentos diários pelo gráfico público do Yahoo Finance.
//!
//! Usado para o mês corrente (a B3 só publica o arquivo mensal depois que o
//! mês acaba) e para o último preço. Ativos da B3 usam o sufixo ".SA".

use std::str::FromStr;

use chrono::{DateTime, NaiveDate};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{http, ProviderError, Result};

pub const SOURCE: &str = "YAHOO";

pub fn b3_symbol(ticker: &str) -> String {
    format!("{ticker}.SA")
}

/// Ativos dos EUA usam o próprio código, com "-" no lugar de "." (BRK.B → BRK-B).
pub fn us_symbol(ticker: &str) -> String {
    ticker.replace('.', "-")
}

pub fn chart_url(symbol: &str, from: NaiveDate, to: NaiveDate) -> String {
    let p1 = from
        .and_hms_opt(0, 0, 0)
        .expect("meia-noite")
        .and_utc()
        .timestamp();
    let p2 = to
        .and_hms_opt(23, 59, 59)
        .expect("fim do dia")
        .and_utc()
        .timestamp();
    format!("https://{{host}}/v8/finance/chart/{symbol}?period1={p1}&period2={p2}&interval=1d")
}

const HOSTS: &[&str] = &["query1.finance.yahoo.com", "query2.finance.yahoo.com"];

#[derive(Deserialize)]
struct Envelope {
    chart: Chart,
}

#[derive(Deserialize)]
struct Chart {
    result: Option<Vec<ChartResult>>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    #[serde(default)]
    gmtoffset: i64,
    regular_market_price: Option<f64>,
    regular_market_time: Option<i64>,
}

#[derive(Deserialize)]
struct ChartResult {
    meta: Meta,
    #[serde(default)]
    timestamp: Vec<i64>,
    indicators: Indicators,
}

#[derive(Deserialize)]
struct Indicators {
    quote: Vec<QuoteSeries>,
}

#[derive(Deserialize)]
struct QuoteSeries {
    #[serde(default)]
    close: Vec<Option<f64>>,
}

/// O Yahoo devolve preços em ponto flutuante; a B3 negocia em centavos.
fn to_cents(v: f64) -> Result<Decimal> {
    Decimal::from_str(&format!("{v:.6}"))
        .map(|d| d.round_dp(2))
        .map_err(|e| ProviderError::Format(format!("preço inválido {v}: {e}")))
}

fn local_date(ts: i64, offset: i64) -> Result<NaiveDate> {
    DateTime::from_timestamp(ts + offset, 0)
        .map(|d| d.date_naive())
        .ok_or_else(|| ProviderError::Format(format!("data inválida {ts}")))
}

/// Interpreta a resposta: fechamentos por dia, incluindo o último preço.
pub fn parse_chart(json: &[u8]) -> Result<Vec<(NaiveDate, Decimal)>> {
    let env: Envelope =
        serde_json::from_slice(json).map_err(|e| ProviderError::Format(e.to_string()))?;
    if let Some(err) = env.chart.error.filter(|e| !e.is_null()) {
        return Err(ProviderError::NotFound(err.to_string()));
    }
    let result = env
        .chart
        .result
        .and_then(|r| r.into_iter().next())
        .ok_or_else(|| ProviderError::Format("resposta sem resultado".into()))?;
    let offset = result.meta.gmtoffset;
    let closes = result
        .indicators
        .quote
        .into_iter()
        .next()
        .map(|q| q.close)
        .unwrap_or_default();
    let mut out: Vec<(NaiveDate, Decimal)> = Vec::new();
    for (ts, close) in result.timestamp.iter().zip(closes) {
        if let Some(c) = close {
            out.push((local_date(*ts, offset)?, to_cents(c)?));
        }
    }
    if let (Some(p), Some(t)) = (
        result.meta.regular_market_price,
        result.meta.regular_market_time,
    ) {
        let d = local_date(t, offset)?;
        let p = to_cents(p)?;
        match out.last_mut() {
            Some(last) if last.0 == d => last.1 = p,
            Some(last) if last.0 > d => {}
            _ => out.push((d, p)),
        }
    }
    out.dedup_by_key(|(d, _)| *d);
    Ok(out)
}

pub fn fetch_closes(
    symbol: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<(NaiveDate, Decimal)>> {
    let template = chart_url(symbol, from, to);
    let mut last = ProviderError::Network("Yahoo indisponível".into());
    for host in HOSTS {
        match http::get_bytes(&template.replace("{host}", host)) {
            Ok(body) => return parse_chart(&body),
            Err(e) => last = e,
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn interpreta_grafico() {
        // Resposta sintética no formato do serviço; 1772456400 = 02/03/2026 10:00 em Brasília.
        let json = br#"{"chart":{"result":[{"meta":{"currency":"BRL","symbol":"TEST3.SA","gmtoffset":-10800,
            "regularMarketPrice":12.5,"regularMarketTime":1772654400},
            "timestamp":[1772456400,1772542800,1772629200],
            "indicators":{"quote":[{"close":[12.34000015258789,null,12.399999618530273]}]}}],"error":null}}"#;
        let closes = parse_chart(json).unwrap();
        let d = |day| NaiveDate::from_ymd_opt(2026, 3, day).unwrap();
        assert_eq!(closes, vec![(d(2), dec!(12.34)), (d(4), dec!(12.50))]);
    }

    #[test]
    fn ativo_inexistente() {
        let json = br#"{"chart":{"result":null,"error":{"code":"Not Found","description":"No data found"}}}"#;
        assert!(matches!(parse_chart(json), Err(ProviderError::NotFound(_))));
    }
}
