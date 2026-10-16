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
    // Índices começam com "^" (^BVSP), que precisa ir codificado no endereço.
    let symbol = symbol.replace('^', "%5E");
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
    #[serde(default)]
    chart_previous_close: Option<f64>,
    #[serde(default)]
    previous_close: Option<f64>,
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
    #[serde(default)]
    open: Vec<Option<f64>>,
    #[serde(default)]
    high: Vec<Option<f64>>,
    #[serde(default)]
    low: Vec<Option<f64>>,
    #[serde(default)]
    volume: Vec<Option<f64>>,
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

/// Vela do gráfico (preços em ponto flutuante: só para desenhar, nunca para
/// cálculo). `time` em segundos desde 1970, UTC.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Candle {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Gráfico de um ativo: velas e o último preço.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Candles {
    pub candles: Vec<Candle>,
    /// Último preço e quando foi negociado.
    pub price: Option<f64>,
    pub price_time: Option<i64>,
    /// Fechamento do dia anterior, para a variação do dia.
    pub previous_close: Option<f64>,
    /// Fuso da bolsa em segundos (Brasília = −10800).
    pub gmtoffset: i64,
}

/// Intervalo e período aceitos pelo gráfico do Yahoo (ex.: "5m" e "1d").
pub fn candles_url(symbol: &str, interval: &str, range: &str) -> String {
    let symbol = symbol.replace('^', "%5E");
    format!("https://{{host}}/v8/finance/chart/{symbol}?interval={interval}&range={range}&includePrePost=false")
}

pub fn parse_candles(json: &[u8]) -> Result<Candles> {
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
    let q = result.indicators.quote.into_iter().next();
    let mut candles = Vec::new();
    if let Some(q) = q {
        for (i, ts) in result.timestamp.iter().enumerate() {
            let get = |v: &Vec<Option<f64>>| v.get(i).copied().flatten();
            let Some(close) = get(&q.close) else {
                continue;
            };
            let open = get(&q.open).unwrap_or(close);
            candles.push(Candle {
                time: *ts,
                open,
                high: get(&q.high).unwrap_or(open.max(close)),
                low: get(&q.low).unwrap_or(open.min(close)),
                close,
                volume: get(&q.volume).unwrap_or(0.0),
            });
        }
    }
    Ok(Candles {
        candles,
        price: result.meta.regular_market_price,
        price_time: result.meta.regular_market_time,
        previous_close: result
            .meta
            .previous_close
            .or(result.meta.chart_previous_close),
        gmtoffset: result.meta.gmtoffset,
    })
}

pub fn fetch_candles(symbol: &str, interval: &str, range: &str) -> Result<Candles> {
    let template = candles_url(symbol, interval, range);
    let mut last = ProviderError::Network("Yahoo indisponível".into());
    for host in HOSTS {
        match http::get_bytes(&template.replace("{host}", host)) {
            Ok(body) => return parse_candles(&body),
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
    fn interpreta_velas() {
        let json = br#"{"chart":{"result":[{"meta":{"gmtoffset":-10800,"regularMarketPrice":12.5,
            "regularMarketTime":1772460000,"chartPreviousClose":12.0,"previousClose":12.1},
            "timestamp":[1772456400,1772456700,1772457000],
            "indicators":{"quote":[{"open":[12.0,null,12.3],"high":[12.4,null,12.6],"low":[11.9,null,12.2],
            "close":[12.2,null,12.5],"volume":[100,null,50]}]}}],"error":null}}"#;
        let c = parse_candles(json).unwrap();
        assert_eq!(c.candles.len(), 2, "vela sem preço fica de fora");
        assert_eq!(c.candles[1].high, 12.6);
        assert_eq!(c.previous_close, Some(12.1));
        assert_eq!(c.price, Some(12.5));
        assert!(candles_url("^BVSP", "5m", "1d").contains("%5EBVSP?interval=5m&range=1d"));
    }

    #[test]
    fn ativo_inexistente() {
        let json = br#"{"chart":{"result":null,"error":{"code":"Not Found","description":"No data found"}}}"#;
        assert!(matches!(parse_chart(json), Err(ProviderError::NotFound(_))));
    }
}
