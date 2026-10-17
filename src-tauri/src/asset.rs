//! Página de um ativo: posição, lançamentos, preço médio ao longo do tempo e
//! o gráfico de preços.
//!
//! O gráfico vem da rede na hora (Yahoo para B3 e EUA; Coinbase para o
//! intradiário das criptomoedas) e não é gravado: serve só para olhar. Sem
//! internet, ou para fundos, usa os fechamentos guardados no cofre.

use carteira_core::model::USD;
use carteira_core::operations::{trades_of, PricedOperation};
use carteira_core::portfolio::{positions, positions_with_fx, Position};
use carteira_core::{crypto, fund};
use carteira_providers::yahoo::{self, Candle, Candles};
use carteira_providers::{coinbase, ProviderError};
use carteira_storage::{normalize_ticker, Vault};
use chrono::{Duration, NaiveDate};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Serialize;

/// Onde o ativo é negociado, para escolher a fonte e explicar o atraso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Venue {
    /// B3: cotação gratuita com cerca de 15 minutos de atraso.
    B3,
    /// Bolsas dos EUA: quase em tempo real no Yahoo.
    Us,
    /// Criptomoedas: tempo real pela Coinbase.
    Cripto,
    /// Fundos: uma cota por dia, pela CVM.
    Fundo,
}

fn venue_of(ticker: &str, currency: &str) -> Venue {
    if fund::is_fund(ticker) {
        Venue::Fundo
    } else if crypto::is_crypto(ticker) {
        Venue::Cripto
    } else if currency == USD {
        Venue::Us
    } else {
        Venue::B3
    }
}

/// Preço médio depois das operações de um dia.
#[derive(Debug, Serialize)]
pub struct AverageStep {
    pub date: NaiveDate,
    pub quantity: Decimal,
    pub average_price: Decimal,
}

#[derive(Debug, Serialize)]
pub struct DailyClose {
    pub date: NaiveDate,
    pub close: Decimal,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct AssetDetail {
    pub ticker: String,
    pub name: String,
    pub kind: String,
    pub currency: String,
    pub venue: Venue,
    /// Posição de hoje (também quando já foi toda vendida).
    pub position: Option<Position>,
    /// Lançamentos do ativo, do mais recente para o mais antigo.
    pub operations: Vec<PricedOperation>,
    pub average_steps: Vec<AverageStep>,
    /// Fechamentos guardados no cofre.
    pub history: Vec<DailyClose>,
}

pub fn detail(v: &Vault, ticker: &str, as_of: NaiveDate) -> Result<AssetDetail, String> {
    let s = |e: carteira_storage::VaultError| e.to_string();
    let ticker = normalize_ticker(ticker);
    let asset = v.get_asset(&ticker).map_err(s)?;
    let all = crate::views::engine_operations(v)?;
    let (priced, _) = crate::views::price_all(v, &all)?;
    let mine: Vec<PricedOperation> = priced
        .into_iter()
        .filter(|p| p.operation.asset == ticker)
        .collect();
    let currency = asset
        .as_ref()
        .map(|a| a.currency.clone())
        .or_else(|| mine.first().map(|p| p.operation.currency.clone()))
        .unwrap_or_else(|| "BRL".into());
    let table = v.price_table(std::slice::from_ref(&ticker)).map_err(s)?;
    let fx = v.fx_table().map_err(s)?;
    let trades = trades_of(&mine);
    let position = positions_with_fx(&trades, &table, Some(&fx), as_of)
        .into_iter()
        .find(|p| p.asset == ticker);
    let mut dates: Vec<NaiveDate> = mine.iter().map(|p| p.operation.date).collect();
    dates.dedup();
    let average_steps = dates
        .into_iter()
        .filter_map(|d| {
            positions(&trades, &table, d)
                .into_iter()
                .find(|p| p.asset == ticker)
                .map(|p| AverageStep {
                    date: d,
                    quantity: p.quantity,
                    average_price: p.average_price,
                })
        })
        .collect();
    let history = table
        .series(&ticker)
        .into_iter()
        .map(|q| DailyClose {
            date: q.date,
            close: q.close,
            source: q.source.clone(),
        })
        .collect();
    let mut operations = mine;
    operations.reverse();
    Ok(AssetDetail {
        venue: venue_of(&ticker, &currency),
        name: asset.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
        kind: asset.map(|a| a.kind).unwrap_or_else(|| "OUTRO".into()),
        ticker,
        currency,
        position,
        operations,
        average_steps,
        history,
    })
}

/// Gráfico pedido: período ("1d", "5d", "1mo", "6mo", "1y", "5y", "max") e,
/// no dia, o tamanho da vela ("1m", "5m", "15m").
#[derive(Debug, Serialize)]
pub struct ChartData {
    pub candles: Vec<Candle>,
    /// Velas de um dia ou mais (eixo por data) ou do dia (eixo por hora).
    pub daily: bool,
    pub price: Option<f64>,
    pub price_time: Option<i64>,
    pub previous_close: Option<f64>,
    pub gmtoffset: i64,
    /// "Yahoo", "Coinbase" ou "cofre" (fechamentos guardados).
    pub source: String,
    /// Motivo de ter usado o cofre, quando a rede falhou.
    pub offline: Option<String>,
}

/// Velas de um dia a partir dos fechamentos guardados.
fn from_history(detail: &AssetDetail, from: Option<NaiveDate>) -> Vec<Candle> {
    detail
        .history
        .iter()
        .filter(|q| from.is_none_or(|f| q.date >= f))
        .filter_map(|q| {
            let c = q.close.to_f64()?;
            Some(Candle {
                time: q.date.and_hms_opt(12, 0, 0)?.and_utc().timestamp(),
                open: c,
                high: c,
                low: c,
                close: c,
                volume: 0.0,
            })
        })
        .collect()
}

fn period_start(period: &str, today: NaiveDate) -> Option<NaiveDate> {
    let days = match period {
        "1d" => 1,
        "5d" => 7,
        "1mo" => 31,
        "6mo" => 183,
        "1y" => 366,
        "5y" => 5 * 366,
        _ => return None,
    };
    Some(today - Duration::days(days))
}

/// Tamanho da vela no Yahoo para cada período.
fn yahoo_interval<'a>(period: &str, interval: &'a str) -> &'a str {
    match period {
        "1d" => match interval {
            "1m" | "5m" | "15m" => interval,
            _ => "5m",
        },
        "5d" => "15m",
        "5y" => "1wk",
        "max" => "1mo",
        _ => "1d",
    }
}

fn yahoo_symbol(detail: &AssetDetail) -> String {
    match detail.venue {
        Venue::B3 => yahoo::b3_symbol(&detail.ticker),
        Venue::Us => yahoo::us_symbol(&detail.ticker),
        Venue::Cripto => crypto::yahoo_symbol(&detail.ticker),
        Venue::Fundo => detail.ticker.clone(),
    }
}

/// Busca o gráfico na rede. `net` é substituível nos testes.
pub trait ChartNet {
    fn yahoo(&self, symbol: &str, interval: &str, range: &str) -> Result<Candles, ProviderError>;
    fn coinbase(&self, ticker: &str, granularity: i64) -> Result<Candles, ProviderError>;
}

pub struct Internet;

impl ChartNet for Internet {
    fn yahoo(&self, symbol: &str, interval: &str, range: &str) -> Result<Candles, ProviderError> {
        yahoo::fetch_candles(symbol, interval, range)
    }
    fn coinbase(&self, ticker: &str, granularity: i64) -> Result<Candles, ProviderError> {
        coinbase::fetch_candles(ticker, granularity)
    }
}

pub fn chart(
    detail: &AssetDetail,
    net: &dyn ChartNet,
    period: &str,
    interval: &str,
    today: NaiveDate,
) -> ChartData {
    let intraday = matches!(period, "1d" | "5d");
    let stored = |offline: Option<String>| ChartData {
        candles: from_history(detail, period_start(period, today)),
        daily: true,
        price: detail.history.last().and_then(|q| q.close.to_f64()),
        price_time: None,
        previous_close: None,
        gmtoffset: 0,
        source: "cofre".into(),
        offline,
    };
    if detail.venue == Venue::Fundo {
        return stored(None);
    }
    let fetched = if detail.venue == Venue::Cripto && intraday {
        // Coinbase: até 300 velas; 5 dias com velas de 15 minutos não cabem,
        // então usam velas de 1 hora.
        let granularity = match (period, interval) {
            ("1d", "1m") => 60,
            ("1d", "15m") => 900,
            ("1d", _) => 300,
            _ => 3600,
        };
        net.coinbase(&detail.ticker, granularity)
            .map(|c| (c, "Coinbase"))
            .or_else(|_| {
                net.yahoo(
                    &yahoo_symbol(detail),
                    yahoo_interval(period, interval),
                    period,
                )
                .map(|c| (c, "Yahoo"))
            })
    } else {
        net.yahoo(
            &yahoo_symbol(detail),
            yahoo_interval(period, interval),
            period,
        )
        .map(|c| (c, "Yahoo"))
    };
    match fetched {
        Ok((mut c, source)) => {
            // A Coinbase devolve as últimas 300 velas; o período manda.
            if source == "Coinbase" {
                let span = if period == "1d" { 86_400 } else { 5 * 86_400 };
                if let Some(last) = c.candles.last().map(|x| x.time) {
                    c.candles.retain(|x| x.time > last - span);
                }
                c.previous_close = c.candles.first().map(|x| x.open);
            }
            if c.candles.is_empty() && !intraday {
                return stored(Some("a fonte não trouxe preços para o período".into()));
            }
            ChartData {
                candles: c.candles,
                daily: !intraday,
                price: c.price,
                price_time: c.price_time,
                previous_close: c.previous_close,
                gmtoffset: c.gmtoffset,
                source: source.into(),
                offline: None,
            }
        }
        Err(e) => stored(Some(e.to_string())),
    }
}

/// Variação de hoje de um ativo da carteira, para o Home broker.
#[derive(Debug, Serialize)]
pub struct DayMove {
    pub asset: String,
    pub name: String,
    pub currency: String,
    pub venue: Venue,
    pub quantity: Decimal,
    pub price: Option<f64>,
    pub previous_close: Option<f64>,
    /// Variação do preço no dia, em fração (0,012 = 1,2%).
    pub change: Option<f64>,
    /// Ganho ou perda do dia na moeda do ativo e em reais.
    pub result: Option<f64>,
    pub result_brl: Option<f64>,
    /// Valor da posição agora, em reais (para o peso de cada ativo).
    pub value_brl: Option<f64>,
    pub price_time: Option<i64>,
    /// Fundos: data da última cota (a CVM publica uma por dia).
    pub quote_date: Option<NaiveDate>,
    pub offline: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DayView {
    pub items: Vec<DayMove>,
    pub total_brl: Option<f64>,
    pub total_change: Option<f64>,
    /// PTAX usada para converter os ativos em dólar.
    pub fx: Option<f64>,
}

/// O que é preciso do cofre para buscar a variação do dia (sem segurar o
/// cofre durante a rede).
pub struct DayInput {
    detail: AssetDetail,
    quantity: Decimal,
    fx: Option<f64>,
}

pub fn day_inputs(v: &Vault, today: NaiveDate) -> Result<Vec<DayInput>, String> {
    let view = crate::views::portfolio(v, today)?;
    let fx = view.fx.as_ref().and_then(|f| f.rate.to_f64());
    let mut out = Vec::new();
    for p in view.positions.iter().filter(|p| !p.quantity.is_zero()) {
        if carteira_core::savings::is_savings(&p.asset) {
            continue;
        }
        let mut detail = detail(v, &p.asset, today)?;
        if detail.name.is_empty() {
            detail.name = view.names.get(&p.asset).cloned().unwrap_or_default();
        }
        out.push(DayInput {
            fx: if detail.currency == USD {
                fx
            } else {
                Some(1.0)
            },
            quantity: p.quantity,
            detail,
        });
    }
    Ok(out)
}

pub fn day_moves(inputs: Vec<DayInput>, net: &(dyn ChartNet + Sync)) -> DayView {
    let fetch = |i: &DayInput| -> DayMove {
        let d = &i.detail;
        let qty = i.quantity.to_f64().unwrap_or_default();
        let (price, previous_close, price_time, quote_date, offline) = if d.venue == Venue::Fundo {
            // Fundos: a última cota contra a anterior.
            let n = d.history.len();
            let last = d.history.last();
            let prev = n.checked_sub(2).and_then(|k| d.history.get(k));
            (
                last.and_then(|q| q.close.to_f64()),
                prev.and_then(|q| q.close.to_f64()),
                None,
                last.map(|q| q.date),
                None,
            )
        } else {
            match net.yahoo(&yahoo_symbol(d), "15m", "1d") {
                Ok(c) => (c.price, c.previous_close, c.price_time, None, None),
                Err(e) => (None, None, None, None, Some(e.to_string())),
            }
        };
        let change = price
            .zip(previous_close)
            .filter(|(_, p)| *p > 0.0)
            .map(|(a, b)| a / b - 1.0);
        let result = price.zip(previous_close).map(|(a, b)| (a - b) * qty);
        DayMove {
            asset: d.ticker.clone(),
            name: d.name.clone(),
            currency: d.currency.clone(),
            venue: d.venue,
            quantity: i.quantity,
            price,
            previous_close,
            change,
            result,
            result_brl: result.zip(i.fx).map(|(r, f)| r * f),
            value_brl: price.zip(i.fx).map(|(p, f)| p * qty * f),
            price_time,
            quote_date,
            offline,
        }
    };
    let items: Vec<DayMove> = std::thread::scope(|sc| {
        let handles: Vec<_> = inputs.iter().map(|i| sc.spawn(move || fetch(i))).collect();
        handles
            .into_iter()
            .zip(&inputs)
            .map(|(h, i)| {
                h.join().unwrap_or_else(|_| DayMove {
                    asset: i.detail.ticker.clone(),
                    name: i.detail.name.clone(),
                    currency: i.detail.currency.clone(),
                    venue: i.detail.venue,
                    quantity: i.quantity,
                    price: None,
                    previous_close: None,
                    change: None,
                    result: None,
                    result_brl: None,
                    value_brl: None,
                    price_time: None,
                    quote_date: None,
                    offline: Some("falha ao buscar".into()),
                })
            })
            .collect()
    });
    let known: Vec<&DayMove> = items.iter().filter(|m| m.result_brl.is_some()).collect();
    let total_brl = (!known.is_empty()).then(|| known.iter().filter_map(|m| m.result_brl).sum());
    let before: f64 = known
        .iter()
        .filter_map(|m| m.value_brl.zip(m.result_brl).map(|(v, r)| v - r))
        .sum();
    let fx = inputs
        .iter()
        .find(|i| i.detail.currency == USD)
        .and_then(|i| i.fx);
    DayView {
        total_change: total_brl.filter(|_| before > 0.0).map(|t: f64| t / before),
        total_brl,
        items,
        fx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use carteira_core::model::Side;
    use carteira_core::operations::Operation;
    use carteira_storage::{KdfParams, NewQuote};
    use rust_decimal_macros::dec;

    struct Net {
        up: bool,
    }

    impl ChartNet for Net {
        fn yahoo(
            &self,
            symbol: &str,
            interval: &str,
            range: &str,
        ) -> Result<Candles, ProviderError> {
            if !self.up {
                return Err(ProviderError::Network("sem internet".into()));
            }
            Ok(Candles {
                candles: vec![Candle {
                    time: 1,
                    open: 1.0,
                    high: 2.0,
                    low: 0.5,
                    close: 1.5,
                    volume: 0.0,
                }],
                price: Some(1.5),
                price_time: Some(1),
                previous_close: Some(1.0),
                gmtoffset: -10800,
            })
            .map(|mut c| {
                // Registra o pedido no volume, para o teste conferir.
                c.candles[0].volume =
                    (symbol.len() + interval.len() * 100 + range.len() * 10_000) as f64;
                c
            })
        }
        fn coinbase(&self, _ticker: &str, granularity: i64) -> Result<Candles, ProviderError> {
            if !self.up {
                return Err(ProviderError::Network("sem internet".into()));
            }
            Ok(Candles {
                candles: (0..300)
                    .map(|i| Candle {
                        time: i * granularity,
                        open: 1.0,
                        high: 1.0,
                        low: 1.0,
                        close: 1.0,
                        volume: 0.0,
                    })
                    .collect(),
                price: Some(1.0),
                price_time: None,
                previous_close: None,
                gmtoffset: 0,
            })
        }
    }

    fn op(asset: &str, day: u32, side: Side, q: Decimal, p: Decimal) -> Operation {
        Operation {
            id: String::new(),
            account: "Corretora X".into(),
            asset: asset.into(),
            date: NaiveDate::from_ymd_opt(2026, 3, day).unwrap(),
            side,
            quantity: q,
            price: p,
            manual_fees: Some(Decimal::ZERO),
            note: None,
            currency: "BRL".into(),
            from_account: None,
            residue: None,
            ir: None,
            iof: None,
        }
    }

    #[test]
    fn pagina_do_ativo_com_preco_medio_e_grafico() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.2.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        v.create_operation(op("TEST3", 2, Side::Compra, dec!(100), dec!(10)))
            .unwrap();
        v.create_operation(op("TEST3", 5, Side::Compra, dec!(100), dec!(20)))
            .unwrap();
        v.create_operation(op("TEST3", 9, Side::Venda, dec!(50), dec!(25)))
            .unwrap();
        v.create_operation(op("OUTRO3", 9, Side::Compra, dec!(1), dec!(1)))
            .unwrap();
        v.save_quotes(&[NewQuote {
            ticker: "TEST3".into(),
            date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            close: dec!(24),
            source: "B3-COTAHIST".into(),
        }])
        .unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let d = detail(&v, "test3", today).unwrap();
        assert_eq!(d.venue, Venue::B3);
        assert_eq!(d.operations.len(), 3);
        assert_eq!(
            d.operations[0].operation.side,
            Side::Venda,
            "mais recente primeiro"
        );
        let avg: Vec<Decimal> = d.average_steps.iter().map(|a| a.average_price).collect();
        assert_eq!(avg, vec![dec!(10), dec!(15), dec!(15)]);
        assert_eq!(d.position.as_ref().unwrap().quantity, dec!(150));
        assert_eq!(d.history.len(), 1);

        let online = chart(&d, &Net { up: true }, "1d", "1m", today);
        assert_eq!(online.source, "Yahoo");
        assert!(!online.daily);
        // TEST3.SA (8) + "1m" (2 × 100) + "1d" (2 × 10.000)
        assert_eq!(online.candles[0].volume, 20_208.0);
        let year = chart(&d, &Net { up: true }, "1y", "5m", today);
        assert!(year.daily);
        assert_eq!(year.candles[0].volume, 20_208.0, "1y usa velas de 1 dia");
        let offline = chart(&d, &Net { up: false }, "1mo", "", today);
        assert_eq!(offline.source, "cofre");
        assert!(offline.offline.is_some());
        assert_eq!(offline.candles.len(), 1);
    }

    #[test]
    fn cripto_no_dia_vem_da_coinbase_e_fundo_do_cofre() {
        let mut d = AssetDetail {
            ticker: "BTC".into(),
            name: "Bitcoin".into(),
            kind: "CRIPTO".into(),
            currency: "USD".into(),
            venue: venue_of("BTC", "USD"),
            position: None,
            operations: vec![],
            average_steps: vec![],
            history: vec![],
        };
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let c = chart(&d, &Net { up: true }, "1d", "1m", today);
        assert_eq!(c.source, "Coinbase");
        // 300 velas de 1 minuto: todas dentro das últimas 24 horas.
        assert_eq!(c.candles.len(), 300);
        let c = chart(&d, &Net { up: true }, "5d", "", today);
        assert_eq!(c.candles.len(), 120, "5 dias de velas de 1 hora");
        d.ticker = "11.222.333/0001-81".into();
        d.currency = "BRL".into();
        d.venue = venue_of(&d.ticker, "BRL");
        assert_eq!(d.venue, Venue::Fundo);
        assert_eq!(
            chart(&d, &Net { up: true }, "1y", "", today).source,
            "cofre"
        );
    }
}
