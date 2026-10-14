//! Posição atual por ativo: quantidade, preço médio, valor de mercado e
//! resultado realizado e não realizado.
//!
//! Preço médio pelo método da Receita Federal: cada compra soma valor + taxas
//! ao custo; a venda baixa o custo pelo preço médio e não altera o médio.
//!
//! Ativos no exterior têm os valores na moeda original e, em paralelo, em
//! reais: cada compra entra no custo pela PTAX da data da compra, cada venda
//! pela PTAX da data da venda, e o valor de mercado pela PTAX da data de
//! avaliação (Lei 14.754, art. 15). Sem a PTAX de alguma data, os campos em
//! reais ficam vazios em vez de mostrar um número errado.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::model::{FxRate, FxSource, Quote, Side, Trade, BRL};
use crate::returns::PriceSource;

#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub asset: String,
    /// Moeda do ativo ("BRL" ou "USD"). Os campos abaixo estão nessa moeda.
    pub currency: String,
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
    /// Quantidade baixada como resíduo de transferência de custódia.
    pub transfer_residue: Decimal,
    /// Valores em reais. Iguais aos de cima para ativos em reais; vazios se
    /// faltar a PTAX de alguma data.
    pub cost_brl: Option<Decimal>,
    pub market_value_brl: Option<Decimal>,
    pub unrealized_brl: Option<Decimal>,
    pub realized_brl: Option<Decimal>,
    /// PTAX usada no valor de mercado.
    pub fx: Option<FxRate>,
}

/// Posições na moeda de cada ativo, sem conversão (ativos em dólar ficam
/// com os campos em reais vazios).
pub fn positions(trades: &[Trade], prices: &dyn PriceSource, as_of: NaiveDate) -> Vec<Position> {
    positions_with_fx(trades, prices, None, as_of)
}

pub fn positions_with_fx(
    trades: &[Trade],
    prices: &dyn PriceSource,
    fx: Option<&dyn FxSource>,
    as_of: NaiveDate,
) -> Vec<Position> {
    let mut sorted: Vec<&Trade> = trades.iter().filter(|t| t.date <= as_of).collect();
    // No mesmo dia, compras antes das vendas e baixas.
    sorted.sort_by_key(|t| (t.date, !matches!(t.side, Side::Compra)));

    // Câmbio de uma data para a moeda do ativo (1 para reais).
    let rate = |currency: &str, date: NaiveDate| -> Option<Decimal> {
        if currency == BRL {
            Some(Decimal::ONE)
        } else {
            fx.and_then(|f| f.usd_brl_on_or_before(date))
                .map(|r| r.rate)
        }
    };

    let mut map: BTreeMap<String, Position> = BTreeMap::new();
    for t in sorted {
        let p = map.entry(t.asset.clone()).or_insert_with(|| Position {
            asset: t.asset.clone(),
            currency: t.currency.clone(),
            quantity: Decimal::ZERO,
            cost: Decimal::ZERO,
            average_price: Decimal::ZERO,
            quote: None,
            market_value: None,
            unrealized: None,
            realized: Decimal::ZERO,
            oversold: false,
            transfer_residue: Decimal::ZERO,
            cost_brl: Some(Decimal::ZERO),
            market_value_brl: None,
            unrealized_brl: None,
            realized_brl: Some(Decimal::ZERO),
            fx: None,
        });
        let r = rate(&t.currency, t.date);
        match t.side {
            Side::Compra => {
                p.quantity += t.quantity;
                p.cost += t.gross + t.fees;
                p.cost_brl = p.cost_brl.zip(r).map(|(c, r)| c + (t.gross + t.fees) * r);
            }
            Side::Venda | Side::Transferencia => {
                let out = if t.quantity > p.quantity {
                    p.oversold = true;
                    p.quantity
                } else {
                    t.quantity
                };
                // Multiplica antes de dividir para não perder precisão.
                let part = |c: Decimal| {
                    if p.quantity.is_zero() {
                        Decimal::ZERO
                    } else {
                        c * out / p.quantity
                    }
                };
                let cost_out = part(p.cost);
                let cost_out_brl = p.cost_brl.map(part);
                if t.side == Side::Venda {
                    let net = t.gross - t.fees;
                    p.realized += net - cost_out;
                    p.realized_brl = match (p.realized_brl, r, cost_out_brl) {
                        (Some(acc), Some(r), Some(c)) => Some(acc + net * r - c),
                        _ => None,
                    };
                } else {
                    // Resíduo da transferência sai sem venda: o custo
                    // correspondente simplesmente deixa a posição.
                    p.transfer_residue += out;
                }
                p.cost -= cost_out;
                p.cost_brl = p.cost_brl.zip(cost_out_brl).map(|(c, o)| c - o);
                p.quantity -= out;
            }
        }
        p.average_price = if p.quantity.is_zero() {
            Decimal::ZERO
        } else {
            p.cost / p.quantity
        };
        if p.quantity.is_zero() {
            p.cost = Decimal::ZERO;
            p.cost_brl = p.cost_brl.map(|_| Decimal::ZERO);
        }
    }
    for p in map.values_mut() {
        if p.quantity.is_zero() {
            continue;
        }
        p.quote = prices.close_on_or_before(&p.asset, as_of);
        p.market_value = p.quote.as_ref().map(|q| q.close * p.quantity);
        p.unrealized = p.market_value.map(|v| v - p.cost);
        if p.currency == BRL {
            p.market_value_brl = p.market_value;
        } else {
            p.fx = fx.and_then(|f| f.usd_brl_on_or_before(as_of));
            p.market_value_brl = p.market_value.zip(p.fx.as_ref()).map(|(v, f)| v * f.rate);
        }
        p.unrealized_brl = p.market_value_brl.zip(p.cost_brl).map(|(v, c)| v - c);
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
            Some(Quote {
                id: 1,
                asset: asset.into(),
                date,
                close: self.0,
                source: "teste".into(),
            })
        }
    }

    /// Dólar a 5 até o dia 9 e a 6 a partir do dia 10.
    struct Fx;
    impl FxSource for Fx {
        fn usd_brl_on_or_before(&self, date: NaiveDate) -> Option<FxRate> {
            let rate = if date.day0() < 9 { dec!(5) } else { dec!(6) };
            Some(FxRate {
                date,
                rate,
                source: "teste".into(),
            })
        }
    }
    use chrono::Datelike;

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
            currency: "BRL".into(),
        }
    }

    fn usd(mut tr: Trade) -> Trade {
        tr.asset = "TEST".into();
        tr.currency = "USD".into();
        tr
    }

    #[test]
    fn preco_medio_e_resultado() {
        let trades = vec![
            t(4, Side::Compra, dec!(100), dec!(10), dec!(1)),
            t(5, Side::Compra, dec!(100), dec!(12), dec!(1)),
            t(6, Side::Venda, dec!(50), dec!(15), dec!(0.50)),
        ];
        let p = &positions(
            &trades,
            &One(dec!(13)),
            NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
        )[0];
        // Custo 2.202,00 / 200 = 11,01. Venda: 749,50 − 550,50 = 199,00.
        assert_eq!(p.quantity, dec!(150));
        assert_eq!(p.average_price, dec!(11.01));
        assert_eq!(p.realized, dec!(199.00));
        assert_eq!(p.cost, dec!(1651.50));
        assert_eq!(p.unrealized, Some(dec!(1950) - dec!(1651.50)));
        assert_eq!(p.cost_brl, Some(dec!(1651.50)));
        assert_eq!(p.realized_brl, Some(dec!(199.00)));
        assert!(!p.oversold);
    }

    #[test]
    fn venda_maior_que_posicao_e_sinalizada() {
        let trades = vec![
            t(4, Side::Compra, dec!(10), dec!(10), dec!(0)),
            t(5, Side::Venda, dec!(20), dec!(10), dec!(0)),
        ];
        let p = &positions(
            &trades,
            &One(dec!(10)),
            NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
        )[0];
        assert!(p.oversold);
        assert_eq!(p.quantity, dec!(0));
        assert!(p.market_value.is_none());
    }

    #[test]
    fn exterior_em_reais_pela_ptax_de_cada_data() {
        let trades = vec![
            usd(t(4, Side::Compra, dec!(10), dec!(100), dec!(0))), // US$ 1.000 a 5 = R$ 5.000
            usd(t(12, Side::Venda, dec!(5), dec!(120), dec!(0))),  // US$ 600 a 6 = R$ 3.600
        ];
        let p = &positions_with_fx(
            &trades,
            &One(dec!(110)),
            Some(&Fx),
            NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
        )[0];
        assert_eq!(p.currency, "USD");
        assert_eq!(p.realized, dec!(100)); // 600 − 500
        assert_eq!(p.realized_brl, Some(dec!(1100))); // 3.600 − 2.500
        assert_eq!(p.cost_brl, Some(dec!(2500)));
        assert_eq!(p.market_value, Some(dec!(550)));
        assert_eq!(p.market_value_brl, Some(dec!(3300)));
        assert_eq!(p.unrealized_brl, Some(dec!(800)));

        // Sem PTAX os valores em reais ficam vazios.
        let p = &positions(
            &trades,
            &One(dec!(110)),
            NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
        )[0];
        assert_eq!(p.cost_brl, None);
        assert_eq!(p.market_value_brl, None);
    }

    #[test]
    fn residuo_de_transferencia_sai_sem_resultado() {
        let trades = vec![
            usd(t(4, Side::Compra, dec!(10.5), dec!(100), dec!(0))),
            usd(t(11, Side::Transferencia, dec!(0.5), dec!(0), dec!(0))),
        ];
        let p = &positions_with_fx(
            &trades,
            &One(dec!(100)),
            Some(&Fx),
            NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
        )[0];
        assert_eq!(p.quantity, dec!(10));
        assert_eq!(p.realized, dec!(0));
        assert_eq!(p.realized_brl, Some(dec!(0)));
        assert_eq!(p.average_price, dec!(100));
        assert_eq!(p.transfer_residue, dec!(0.5));
        assert_eq!(p.cost_brl, Some(dec!(5000)));
    }
}
