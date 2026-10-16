//! Velas das criptomoedas em tempo real pela API pública da Coinbase
//! (api.exchange.coinbase.com), sem cadastro. Só para o gráfico: o valor da
//! carteira continua usando o fechamento diário do Yahoo.

use serde_json::Value;

use crate::yahoo::{Candle, Candles};
use crate::{http, ProviderError, Result};

/// Segundos por vela aceitos: 1, 5, 15 minutos, 1 hora, 6 horas e 1 dia.
pub const GRANULARITIES: &[i64] = &[60, 300, 900, 3600, 21600, 86400];

pub fn candles_url(ticker: &str, granularity: i64) -> String {
    format!(
        "https://api.exchange.coinbase.com/products/{ticker}-USD/candles?granularity={granularity}"
    )
}

/// Resposta: [[tempo, mínima, máxima, abertura, fechamento, volume], ...],
/// da mais nova para a mais antiga (até 300 velas).
pub fn parse_candles(json: &[u8]) -> Result<Candles> {
    let v: Value =
        serde_json::from_slice(json).map_err(|e| ProviderError::Format(e.to_string()))?;
    let rows = v.as_array().ok_or_else(|| {
        let msg = v
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("resposta inesperada");
        ProviderError::NotFound(msg.to_string())
    })?;
    let mut candles: Vec<Candle> = rows
        .iter()
        .filter_map(|r| {
            let r = r.as_array()?;
            let n = |i: usize| r.get(i).and_then(Value::as_f64);
            Some(Candle {
                time: r.first()?.as_i64()?,
                low: n(1)?,
                high: n(2)?,
                open: n(3)?,
                close: n(4)?,
                volume: n(5).unwrap_or(0.0),
            })
        })
        .collect();
    candles.sort_by_key(|c| c.time);
    let last = candles.last();
    Ok(Candles {
        price: last.map(|c| c.close),
        price_time: last.map(|c| c.time),
        previous_close: None,
        gmtoffset: 0,
        candles,
    })
}

pub fn fetch_candles(ticker: &str, granularity: i64) -> Result<Candles> {
    parse_candles(&http::get_bytes(&candles_url(ticker, granularity))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpreta_velas_em_ordem() {
        let json =
            br#"[[1772457000,99.0,101.0,100.0,100.5,2.5],[1772456940,98.0,100.0,99.0,100.0,1.0]]"#;
        let c = parse_candles(json).unwrap();
        assert_eq!(c.candles.len(), 2);
        assert_eq!(c.candles[0].time, 1772456940);
        assert_eq!(c.price, Some(100.5));
        assert!(parse_candles(br#"{"message":"NotFound"}"#).is_err());
        assert!(candles_url("BTC", 60).ends_with("/products/BTC-USD/candles?granularity=60"));
    }
}
