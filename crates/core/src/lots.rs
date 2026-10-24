//! Compras em aberto de um ativo, pelo critério FIFO (a venda sai da compra
//! mais antiga). Só para mostrar o ganho de cada compra: o custo, o
//! resultado e o imposto continuam pelo preço médio ([`crate::portfolio`]).

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::model::{is_dust, Side, Trade};

/// O que sobrou de uma compra.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Lot {
    /// Lançamento da compra.
    pub id: String,
    pub date: NaiveDate,
    pub account: String,
    /// Quantidade comprada (já ajustada por desdobramento ou grupamento).
    pub bought: Decimal,
    /// Quantidade que ainda resta desta compra.
    pub quantity: Decimal,
    /// Custo do que resta, com as taxas, na moeda do ativo.
    pub cost: Decimal,
}

impl Lot {
    /// Custo por unidade, com as taxas.
    pub fn unit_cost(&self) -> Decimal {
        if self.quantity.is_zero() {
            Decimal::ZERO
        } else {
            self.cost / self.quantity
        }
    }
}

/// Tira `qty` das compras, das mais antigas para as mais novas, começando
/// pela conta indicada. Devolve as partes tiradas (quantidade, custo e a
/// compra de origem).
fn take(lots: &mut [Lot], account: &str, qty: Decimal) -> Vec<(usize, Decimal, Decimal)> {
    let mut rest = qty;
    let mut out = Vec::new();
    for pass in [true, false] {
        for (i, l) in lots.iter_mut().enumerate() {
            if rest <= Decimal::ZERO {
                return out;
            }
            if (l.account == account) != pass || l.quantity <= Decimal::ZERO {
                continue;
            }
            let q = rest.min(l.quantity);
            let c = if q == l.quantity {
                l.cost
            } else {
                l.cost * q / l.quantity
            };
            l.quantity -= q;
            l.cost -= c;
            rest -= q;
            out.push((i, q, c));
        }
    }
    out
}

/// Compras em aberto, da mais antiga para a mais nova, de um único ativo.
pub fn open_lots(trades: &[Trade]) -> Vec<Lot> {
    let mut sorted: Vec<&Trade> = trades.iter().collect();
    // Compras do dia antes das vendas do mesmo dia (como no preço médio).
    sorted.sort_by_key(|t| (t.date, t.side.day_order()));
    let mut lots: Vec<Lot> = Vec::new();
    for t in sorted {
        match t.side {
            Side::Compra => lots.push(Lot {
                id: t.id.clone(),
                date: t.date,
                account: t.account.clone(),
                bought: t.quantity,
                quantity: t.quantity,
                cost: t.gross + t.fees,
            }),
            Side::Venda => {
                take(&mut lots, &t.account, t.quantity);
            }
            Side::Transferencia => {
                let from = t.from_account.as_deref().unwrap_or(&t.account);
                // A parte transferida muda de conta, mantendo data e custo.
                let moved = take(&mut lots, from, t.moved);
                for (i, q, c) in moved {
                    let mut part = lots[i].clone();
                    part.account = t.account.clone();
                    part.quantity = q;
                    part.cost = c;
                    part.bought = q;
                    lots.push(part);
                }
                take(&mut lots, from, t.quantity);
                lots.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
            }
            Side::Ajuste => {
                // Desdobramento, grupamento ou bonificação: vale para todas
                // as compras da conta, na proporção da quantidade.
                let held: Decimal = lots
                    .iter()
                    .filter(|l| l.account == t.account)
                    .map(|l| l.quantity)
                    .sum();
                if held <= Decimal::ZERO {
                    continue;
                }
                let factor = (held + t.quantity) / held;
                let added = t.gross + t.fees;
                for l in lots.iter_mut().filter(|l| l.account == t.account) {
                    let share = l.quantity / held;
                    l.cost += added * share;
                    l.quantity *= factor;
                    l.bought *= factor;
                }
            }
        }
    }
    lots.retain(|l| !is_dust(l.quantity) && l.quantity > Decimal::ZERO);
    lots
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn t(id: &str, day: u32, side: Side, q: Decimal, p: Decimal, account: &str) -> Trade {
        Trade {
            id: id.into(),
            asset: "MGLU3".into(),
            date: NaiveDate::from_ymd_opt(2026, 10, day).unwrap(),
            side,
            quantity: q,
            price: p,
            gross: q * p,
            fees: Decimal::ZERO,
            currency: "BRL".into(),
            account: account.into(),
            from_account: None,
            moved: Decimal::ZERO,
        }
    }

    #[test]
    fn venda_sai_da_compra_mais_antiga() {
        let mut b2 = t("b", 23, Side::Compra, dec!(8000), dec!(12.48), "Itaú");
        b2.fees = dec!(30);
        let trades = vec![
            t("a", 9, Side::Compra, dec!(4540), dec!(11.02), "Itaú"),
            b2,
            t("v", 24, Side::Venda, dec!(5000), dec!(13), "Itaú"),
        ];
        let lots = open_lots(&trades);
        assert_eq!(lots.len(), 1);
        assert_eq!(lots[0].id, "b");
        assert_eq!(lots[0].quantity, dec!(7540));
        assert_eq!(lots[0].bought, dec!(8000));
        // Custo com taxas, proporcional ao que sobrou.
        assert_eq!(lots[0].cost, (dec!(99870) * dec!(7540) / dec!(8000)));
        assert_eq!(lots[0].unit_cost().round_dp(6), dec!(12.48375));
    }

    #[test]
    fn desdobramento_e_transferencia() {
        let mut tr = t("t", 12, Side::Transferencia, dec!(0), dec!(0), "Nova");
        tr.from_account = Some("Antiga".into());
        tr.moved = dec!(15);
        let trades = vec![
            t("a", 1, Side::Compra, dec!(10), dec!(10), "Antiga"),
            t("b", 2, Side::Compra, dec!(10), dec!(20), "Antiga"),
            // Desdobramento 1 para 2.
            Trade {
                gross: Decimal::ZERO,
                ..t("d", 10, Side::Ajuste, dec!(20), dec!(0), "Antiga")
            },
            tr,
        ];
        let lots = open_lots(&trades);
        // 20 + 20 após o desdobramento; 15 da primeira compra vão para "Nova".
        let q: Vec<(&str, &str, Decimal, Decimal)> = lots
            .iter()
            .map(|l| (l.id.as_str(), l.account.as_str(), l.quantity, l.cost))
            .collect();
        assert_eq!(
            q,
            vec![
                ("a", "Antiga", dec!(5), dec!(25)),
                ("a", "Nova", dec!(15), dec!(75)),
                ("b", "Antiga", dec!(20), dec!(200)),
            ]
        );
    }
}
