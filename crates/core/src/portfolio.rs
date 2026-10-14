//! Posição atual por ativo: quantidade, preço médio, valor de mercado e
//! resultado realizado e não realizado.
//!
//! Preço médio pelo método da Receita Federal: cada compra soma valor + taxas
//! ao custo; a venda baixa o custo pelo preço médio e não altera o médio.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::model::{Quote, Side, Trade};
use crate::returns::PriceSource;

#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub asset: String,
    pub quantity: Decimal,
    /// Custo total da posição aberta (inclui taxas de compra).
    pub cost: Decimal,
    /// Custo ÷ quantidade; zero sem posição.
    pub average_price: Decimal,
    pub quote: Option<Quote>,
    pub market_value: Option<Decimal>,
    pub unrealized: Option<Decimal>,
    /// Lucro já realizado nas vendas (venda líquida − custo médio).
    pub realized: Decimal,
    /// Venda maior que a posição em alguma data.
    pub oversold: bool,
}

pub fn positions(trades: &[Trade], prices: &dyn PriceSource, as_of: NaiveDate) -> Vec<Position> {
    let mut sorted: Vec<&Trade> = trades.iter().filter(|t| t.date <= as_of).collect();
    // No mesmo dia, compras antes das vendas.
    sorted.sort_by_key(|t| (t.date, matches!(t.side, Side::Venda)));

    let mut map: BTreeMap<String, Position> = BTreeMap::new();
    for t in sorted {
        let p = map.entry(t.asset.clone()).or_insert_with(|| Position {
            asset: t.asset.clone(),
            quantity: Decimal::ZERO,
            cost: Decimal::ZERO,
            average_price: Decimal::ZERO,
            quote: None,
            market_value: None,
            unrealized: None,
            realized: Decimal::ZERO,
            oversold: false,
        });
        match t.side {
            Side::Compra => {
                p.quantity += t.quantity;
                p.cost += t.gross + t.fees;
            }
            Side::Venda => {
                let sold = if t.quantity > p.quantity {
                    p.oversold = true;
                    p.quantity
                } else {
                    t.quantity
                };
                let cost_out = if p.quantity.is_zero() { Decimal::ZERO } else { p.cost * sold / p.quantity };
                p.realized += (t.gross - t.fees) - cost_out;
                p.cost -= cost_out;
                p.quantity -= sold;
            }
        }
        p.average_price = if p.quantity.is_zero() { Decimal::ZERO } else { p.cost / p.quantity };
        if p.quantity.is_zero() {
            p.cost = Decimal::ZERO;
        }
    }
    for p in map.values_mut() {
        if p.quantity.is_zero() {
            continue;
        }
        p.quote = prices.close_on_or_before(&p.asset, as_of);
        p.market_value = p.quote.as_ref().map(|q| q.close * p.quantity);
        p.unrealized = p.market_value.map(|v| v - p.cost);
    }
    map.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    struct One(Decimal);
    impl PriceSource for One {
        fn close_on_or_before(&self, asset: &str, date: NaiveDate) -> Option<Quote> {
            Some(Quote { id: 1, asset: asset.into(), date, close: self.0, source: "teste".into() })
        }
    }

    fn t(d: u32, side: Side, q: Decimal, p: Decimal, fees: Decimal) -> Trade {
        Trade {
            id: format!("{d}"),
            asset: "TEST3".into(),
            date: NaiveDate::from_ymd_opt(2026, 5, d).unwrap(),
            side,
            quantity: q,
            price: p,
            gross: q * p,
            fees,
        }
    }

    #[test]
    fn preco_medio_e_resultado() {
        let trades = vec![
            t(4, Side::Compra, dec!(100), dec!(10), dec!(1)),
            t(5, Side::Compra, dec!(100), dec!(12), dec!(1)),
            t(6, Side::Venda, dec!(50), dec!(15), dec!(0.50)),
        ];
        let p = &positions(&trades, &One(dec!(13)), NaiveDate::from_ymd_opt(2026, 5, 31).unwrap())[0];
        // Custo 2.202,00 / 200 = 11,01. Venda: 749,50 − 550,50 = 199,00.
        assert_eq!(p.quantity, dec!(150));
        assert_eq!(p.average_price, dec!(11.01));
        assert_eq!(p.realized, dec!(199.00));
        assert_eq!(p.cost, dec!(1651.50));
        assert_eq!(p.unrealized, Some(dec!(1950) - dec!(1651.50)));
        assert!(!p.oversold);
    }

    #[test]
    fn venda_maior_que_posicao_e_sinalizada() {
        let trades = vec![t(4, Side::Compra, dec!(10), dec!(10), dec!(0)), t(5, Side::Venda, dec!(20), dec!(10), dec!(0))];
        let p = &positions(&trades, &One(dec!(10)), NaiveDate::from_ymd_opt(2026, 5, 31).unwrap())[0];
        assert!(p.oversold);
        assert_eq!(p.quantity, dec!(0));
        assert!(p.market_value.is_none());
    }
}
