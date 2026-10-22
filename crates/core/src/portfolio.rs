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
    /// PTAX da data de avaliação (a dos impostos).
    pub fx: Option<FxRate>,
    /// Câmbio usado no valor de mercado em reais: a PTAX ou, nas contas com
    /// custo para trazer ao Brasil, o valor de resgate. Com mais de uma
    /// conta, o da conta com mais quantidade.
    pub value_fx: Option<FxRate>,
    /// Contas com quantidade em aberto deste ativo.
    pub accounts: Vec<String>,
    /// Quantidade por conta, para avaliar cada parte pelo câmbio da conta.
    #[serde(skip)]
    pub by_account: BTreeMap<String, Decimal>,
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
    // No mesmo dia: eventos, depois compras, depois vendas e baixas.
    sorted.sort_by_key(|t| (t.date, t.side.day_order()));

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
            value_fx: None,
            accounts: Vec::new(),
            by_account: BTreeMap::new(),
        });
        let r = rate(&t.currency, t.date);
        match t.side {
            // No evento, a quantidade pode cair (grupamento); o custo total só
            // cresce pelo custo atribuído às ações bonificadas.
            Side::Compra | Side::Ajuste => {
                *p.by_account.entry(t.account.clone()).or_default() += t.quantity;
                p.quantity += t.quantity;
                p.cost += t.gross + t.fees;
                p.cost_brl = p.cost_brl.zip(r).map(|(c, r)| c + (t.gross + t.fees) * r);
            }
            Side::Venda | Side::Transferencia => {
                let out = if t.quantity > p.quantity {
                    p.oversold = true;
                    p.quantity
                } else if crate::model::is_dust(p.quantity - t.quantity) {
                    // A sobra de arredondamento sai junto.
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
                if t.side == Side::Transferencia {
                    // Muda de conta e o resíduo sai da conta de origem.
                    let from = t.from_account.as_deref().unwrap_or(&t.account);
                    let available = p.by_account.get(from).copied().unwrap_or_default();
                    let moved = t.moved.min(available).max(Decimal::ZERO);
                    take_from_accounts(&mut p.by_account, from, moved + out);
                    *p.by_account.entry(t.account.clone()).or_default() += moved;
                } else {
                    take_from_accounts(&mut p.by_account, &t.account, out);
                }
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
        // Sobra de fração sem venda que a zere (troca de custodiante, por exemplo).
        if crate::model::is_dust(p.quantity) {
            p.quantity = Decimal::ZERO;
            p.average_price = Decimal::ZERO;
            p.cost = Decimal::ZERO;
            p.cost_brl = p.cost_brl.map(|_| Decimal::ZERO);
        }
        if p.quantity.is_zero() {
            continue;
        }
        p.quote = prices.close_on_or_before(&p.asset, as_of);
        p.market_value = p.quote.as_ref().map(|q| q.close * p.quantity);
        p.unrealized = p.market_value.map(|v| v - p.cost);
        p.by_account
            .retain(|_, q| !crate::model::is_dust(*q) && *q > Decimal::ZERO);
        p.accounts = p.by_account.keys().cloned().collect();
        if p.currency == BRL {
            p.market_value_brl = p.market_value;
        } else {
            p.fx = fx.and_then(|f| f.usd_brl_on_or_before(as_of));
            let close = p.quote.as_ref().map(|q| q.close);
            let (value, main) = value_in_brl(&p.by_account, p.quantity, close, fx, as_of);
            p.market_value_brl = value;
            p.value_fx = main;
        }
        p.unrealized_brl = p.market_value_brl.zip(p.cost_brl).map(|(v, c)| v - c);
    }
    map.into_values().collect()
}

/// Baixa uma saída da conta da operação; o que faltar sai das outras contas
/// (venda lançada numa conta diferente da compra).
fn take_from_accounts(by_account: &mut BTreeMap<String, Decimal>, account: &str, out: Decimal) {
    let mut rest = out;
    if let Some(q) = by_account.get_mut(account) {
        let take = rest.min(*q);
        *q -= take;
        rest -= take;
    }
    for q in by_account.values_mut() {
        if rest <= Decimal::ZERO {
            break;
        }
        let take = rest.min(*q);
        *q -= take;
        rest -= take;
    }
}

/// Valor em reais de uma posição em dólar: cada conta pelo câmbio dela.
/// Devolve também o câmbio da conta com mais quantidade, para exibir.
fn value_in_brl(
    by_account: &BTreeMap<String, Decimal>,
    quantity: Decimal,
    close: Option<Decimal>,
    fx: Option<&dyn FxSource>,
    date: NaiveDate,
) -> (Option<Decimal>, Option<FxRate>) {
    let (Some(close), Some(fx)) = (close, fx) else {
        return (None, None);
    };
    // Sem conta conhecida (lançamentos antigos em testes), tudo pela PTAX.
    let parts: Vec<(&str, Decimal)> = if by_account.is_empty() {
        vec![("", quantity)]
    } else {
        by_account.iter().map(|(a, q)| (a.as_str(), *q)).collect()
    };
    let main = parts
        .iter()
        .max_by_key(|(_, q)| *q)
        .and_then(|(a, _)| fx.value_rate(a, date));
    let mut total = Decimal::ZERO;
    for (account, q) in parts {
        let Some(r) = fx.value_rate(account, date) else {
            return (None, main);
        };
        total += close * q * r.rate;
    }
    (Some(total), main)
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
            account: String::new(),
            from_account: None,
            moved: Decimal::ZERO,
        }
    }

    fn usd(mut tr: Trade) -> Trade {
        tr.asset = "TEST".into();
        tr.currency = "USD".into();
        tr
    }

    /// PTAX 5; a conta "Resgate" vale 4,90 por dólar.
    struct ByAccount;
    impl FxSource for ByAccount {
        fn usd_brl_on_or_before(&self, date: NaiveDate) -> Option<FxRate> {
            Some(FxRate {
                date,
                rate: dec!(5),
                source: "PTAX".into(),
            })
        }
        fn value_rate(&self, account: &str, date: NaiveDate) -> Option<FxRate> {
            if account == "Resgate" {
                Some(FxRate {
                    date,
                    rate: dec!(4.90),
                    source: "RESGATE: teste".into(),
                })
            } else {
                self.usd_brl_on_or_before(date)
            }
        }
    }

    #[test]
    fn cada_conta_pelo_seu_cambio_e_transferencia_muda_a_conta() {
        let mut buy = usd(t(4, Side::Compra, dec!(10), dec!(100), dec!(0)));
        buy.account = "Antiga".into();
        let mut other = usd(t(5, Side::Compra, dec!(2), dec!(100), dec!(0)));
        other.account = "Resgate".into();
        let as_of = NaiveDate::from_ymd_opt(2026, 5, 31).unwrap();
        let p = positions_with_fx(
            &[buy.clone(), other.clone()],
            &One(dec!(100)),
            Some(&ByAccount),
            as_of,
        )
        .remove(0);
        // 10 × 100 × 5 + 2 × 100 × 4,90.
        assert_eq!(p.market_value_brl, Some(dec!(5980)));
        assert_eq!(
            p.value_fx.unwrap().rate,
            dec!(5),
            "a conta com mais quantidade"
        );
        assert_eq!(p.fx.unwrap().rate, dec!(5));

        // Troca de custodiante: 9,9 passam para "Resgate" e 0,1 de resíduo sai.
        let mut tr = usd(t(6, Side::Transferencia, dec!(0.1), dec!(0), dec!(0)));
        tr.account = "Resgate".into();
        tr.from_account = Some("Antiga".into());
        tr.moved = dec!(9.9);
        let p = positions_with_fx(&[buy, other, tr], &One(dec!(100)), Some(&ByAccount), as_of)
            .remove(0);
        assert_eq!(p.quantity, dec!(11.9));
        assert_eq!(p.accounts, vec!["Resgate".to_string()]);
        assert_eq!(
            p.market_value_brl,
            Some(dec!(11.9) * dec!(100) * dec!(4.90))
        );
        assert!(p.value_fx.unwrap().source.starts_with("RESGATE"));
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
    fn sobra_de_fracao_conta_como_zerada() {
        let at = NaiveDate::from_ymd_opt(2026, 5, 31).unwrap();
        // Venda deixa 0,00001: sai junto, com todo o custo.
        let vendida = vec![
            t(4, Side::Compra, dec!(1.50001), dec!(10), dec!(0)),
            t(5, Side::Venda, dec!(1.5), dec!(12), dec!(0)),
        ];
        let p = &positions(&vendida, &One(dec!(13)), at)[0];
        assert_eq!(p.quantity, dec!(0));
        assert_eq!(p.realized, dec!(18) - dec!(15.0001));
        assert!(p.market_value.is_none());
        // Sobra sem venda nenhuma também não aparece como posição.
        let sobra = vec![t(4, Side::Compra, dec!(0.00003), dec!(10), dec!(0))];
        assert_eq!(positions(&sobra, &One(dec!(13)), at)[0].quantity, dec!(0));
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
