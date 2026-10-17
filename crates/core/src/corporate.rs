//! Eventos corporativos: bonificação, desdobramento e grupamento.
//!
//! Cada evento vira, em cada conta que tinha o ativo no fim da data com, um
//! ajuste de quantidade ([`Side::Ajuste`]) no dia seguinte (data ex), sem
//! dinheiro entrando ou saindo:
//!
//! - **Bonificação** de P%: entram `quantidade × P%` ações novas, com o custo
//!   por ação informado pela empresa somado ao custo da posição (regra da
//!   Receita; o valor é rendimento isento na declaração).
//! - **Desdobramento** de 1 para N: a quantidade é multiplicada por N e o
//!   custo total não muda (o preço médio cai na mesma proporção).
//! - **Grupamento** de N para 1: a quantidade é dividida por N, também sem
//!   mudar o custo total.
//!
//! Na B3 não existe fração de ação: a sobra é vendida em leilão pela empresa
//! e o dinheiro cai na conta. Por isso, nas contas em reais a quantidade nova
//! é arredondada para baixo; no exterior, a fração fica na posição.

use std::collections::BTreeMap;

use chrono::{Duration, NaiveDate};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::model::{Side, BRL};
use crate::operations::Operation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EventKind {
    Bonificacao,
    Desdobramento,
    Grupamento,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::Bonificacao => "BONIFICACAO",
            EventKind::Desdobramento => "DESDOBRAMENTO",
            EventKind::Grupamento => "GRUPAMENTO",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "BONIFICACAO" => EventKind::Bonificacao,
            "DESDOBRAMENTO" => EventKind::Desdobramento,
            "GRUPAMENTO" => EventKind::Grupamento,
            _ => return None,
        })
    }
}

/// Evento como o usuário lançou, copiando o anúncio da empresa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorporateEvent {
    pub id: String,
    pub asset: String,
    pub kind: EventKind,
    /// Último dia em que quem tinha a ação recebeu o evento.
    pub data_com: NaiveDate,
    /// Bonificação: percentual (10 = 10%). Desdobramento: cada ação vira N.
    /// Grupamento: cada N ações viram uma.
    pub proportion: Decimal,
    /// Bonificação: custo por ação nova informado pela empresa (na moeda do
    /// ativo). Zero quando a empresa não informar.
    #[serde(default)]
    pub cost_per_share: Decimal,
    pub note: Option<String>,
}

impl CorporateEvent {
    /// Primeiro pregão sem direito ao evento: é quando a quantidade muda.
    pub fn ex_date(&self) -> NaiveDate {
        self.data_com + Duration::days(1)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.asset.trim().is_empty() {
            return Err("informe o ativo".into());
        }
        if self.proportion <= Decimal::ZERO {
            return Err(match self.kind {
                EventKind::Bonificacao => "informe o percentual da bonificação".into(),
                _ => "informe a proporção".into(),
            });
        }
        if self.kind != EventKind::Bonificacao && self.proportion <= Decimal::ONE {
            return Err(match self.kind {
                EventKind::Desdobramento => {
                    "no desdobramento, cada ação vira mais de uma (por exemplo, 1 para 2)".into()
                }
                _ => "no grupamento, várias ações viram uma (por exemplo, 10 para 1)".into(),
            });
        }
        if self.cost_per_share < Decimal::ZERO {
            return Err("o custo por ação não pode ser negativo".into());
        }
        Ok(())
    }

    /// Quantidade depois do evento, a partir da quantidade da conta.
    pub fn quantity_after(&self, before: Decimal, currency: &str) -> Decimal {
        let raw = match self.kind {
            EventKind::Bonificacao => before + before * self.proportion / Decimal::ONE_HUNDRED,
            EventKind::Desdobramento => before * self.proportion,
            EventKind::Grupamento => before / self.proportion,
        };
        let dp = if currency == BRL { 0 } else { 8 };
        raw.round_dp_with_strategy(dp, RoundingStrategy::ToZero)
    }
}

/// Efeito do evento numa conta.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventEffect {
    pub event_id: String,
    pub account: String,
    pub currency: String,
    pub before: Decimal,
    pub after: Decimal,
    /// Bonificação: custo das ações novas (quantidade nova × custo por ação),
    /// que é o valor isento na declaração.
    pub cost_added: Decimal,
}

impl EventEffect {
    pub fn delta(&self) -> Decimal {
        self.after - self.before
    }
}

/// Quantidade de cada (conta, ativo) com as operações dadas, incluindo o
/// que entrou e saiu nas transferências de custódia.
fn add(map: &mut BTreeMap<(String, String), (Decimal, String)>, op: &Operation) {
    let mut put = |account: &str, q: Decimal| {
        let e = map
            .entry((account.to_string(), op.asset.clone()))
            .or_insert((Decimal::ZERO, op.currency.clone()));
        e.0 += q;
    };
    match op.side {
        Side::Compra | Side::Ajuste => put(&op.account, op.quantity),
        Side::Venda => put(&op.account, -op.quantity),
        Side::Transferencia => {
            put(&op.account, op.quantity);
            if let Some(from) = op.from_account.as_deref() {
                put(from, -(op.quantity + op.residue.unwrap_or_default()));
            }
        }
    }
}

/// Efeitos de todos os eventos, em ordem de data (um evento posterior já
/// considera o anterior), e os ajustes correspondentes para os cálculos.
pub fn apply(ops: &[Operation], events: &[CorporateEvent]) -> (Vec<EventEffect>, Vec<Operation>) {
    let mut sorted: Vec<&CorporateEvent> = events.iter().collect();
    sorted.sort_by_key(|e| (e.data_com, e.id.clone()));
    let mut effects = Vec::new();
    let mut adjustments: Vec<Operation> = Vec::new();
    for ev in sorted {
        let mut held: BTreeMap<(String, String), (Decimal, String)> = BTreeMap::new();
        for op in ops
            .iter()
            .chain(adjustments.iter())
            .filter(|o| o.asset == ev.asset && o.date <= ev.data_com)
        {
            add(&mut held, op);
        }
        let mut new_adjustments = Vec::new();
        for ((account, _), (before, currency)) in held {
            if crate::model::is_dust(before) || before < Decimal::ZERO {
                continue;
            }
            let after = ev.quantity_after(before, &currency);
            let delta = after - before;
            let cost_added = if ev.kind == EventKind::Bonificacao {
                (delta * ev.cost_per_share).round_dp(2)
            } else {
                Decimal::ZERO
            };
            effects.push(EventEffect {
                event_id: ev.id.clone(),
                account: account.clone(),
                currency: currency.clone(),
                before,
                after,
                cost_added,
            });
            if delta.is_zero() {
                continue;
            }
            new_adjustments.push(Operation {
                id: format!("{}:{}", ev.id, account),
                account,
                asset: ev.asset.clone(),
                date: ev.ex_date(),
                side: Side::Ajuste,
                quantity: delta,
                // Valor do ajuste = custo atribuído (só na bonificação).
                price: if delta.is_zero() {
                    Decimal::ZERO
                } else {
                    cost_added / delta
                },
                manual_fees: Some(Decimal::ZERO),
                note: Some(label(ev)),
                currency,
                from_account: None,
                residue: None,
                ir: None,
                iof: None,
            });
        }
        adjustments.extend(new_adjustments);
    }
    (effects, adjustments)
}

/// Descrição curta: "Bonificação de 10%", "Desdobramento 1 para 2".
pub fn label(ev: &CorporateEvent) -> String {
    let n = crate::format::qty(ev.proportion);
    match ev.kind {
        EventKind::Bonificacao => format!("Bonificação de {n}%"),
        EventKind::Desdobramento => format!("Desdobramento 1 para {n}"),
        EventKind::Grupamento => format!("Grupamento {n} para 1"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, day).unwrap()
    }

    fn buy(account: &str, date: NaiveDate, q: Decimal, p: Decimal, currency: &str) -> Operation {
        Operation {
            id: format!("op-{account}-{date}"),
            account: account.into(),
            asset: "TEST3".into(),
            date,
            side: Side::Compra,
            quantity: q,
            price: p,
            manual_fees: Some(Decimal::ZERO),
            note: None,
            currency: currency.into(),
            from_account: None,
            residue: None,
            ir: None,
            iof: None,
        }
    }

    fn event(
        kind: EventKind,
        date: NaiveDate,
        proportion: Decimal,
        cost: Decimal,
    ) -> CorporateEvent {
        CorporateEvent {
            id: format!("ev-{}", kind.as_str()),
            asset: "TEST3".into(),
            kind,
            data_com: date,
            proportion,
            cost_per_share: cost,
            note: None,
        }
    }

    #[test]
    fn bonificacao_arredonda_para_baixo_na_b3() {
        let ops = vec![
            buy("A", d(1, 5), dec!(155), dec!(20), "BRL"),
            buy("B", d(1, 6), dec!(3), dec!(20), "USD"),
            // Comprado depois da data com: não recebe.
            buy("A", d(3, 2), dec!(100), dec!(20), "BRL"),
        ];
        let ev = event(EventKind::Bonificacao, d(3, 1), dec!(10), dec!(4.5));
        let (effects, adj) = apply(&ops, &[ev]);
        assert_eq!(effects.len(), 2);
        assert_eq!(effects[0].after, dec!(170)); // 155 + 15,5 → 170
        assert_eq!(effects[0].cost_added, dec!(67.50));
        assert_eq!(effects[1].after, dec!(3.3)); // exterior mantém a fração
        assert_eq!(adj[0].date, d(3, 2));
        assert_eq!(adj[0].side, Side::Ajuste);
        assert_eq!(adj[0].quantity * adj[0].price, dec!(67.50));
    }

    #[test]
    fn desdobramento_e_grupamento_em_sequencia() {
        let ops = vec![buy("A", d(1, 5), dec!(105), dec!(30), "BRL")];
        let split = event(EventKind::Desdobramento, d(2, 1), dec!(2), dec!(0));
        let group = event(EventKind::Grupamento, d(4, 1), dec!(10), dec!(0));
        let (effects, adj) = apply(&ops, &[group, split]);
        assert_eq!(effects[0].after, dec!(210));
        assert_eq!(effects[1].before, dec!(210));
        assert_eq!(effects[1].after, dec!(21));
        assert_eq!(adj[1].quantity, dec!(-189));
        assert!(adj.iter().all(|a| a.price.is_zero()));
    }

    #[test]
    fn transferencia_leva_a_posicao_para_a_outra_conta() {
        let mut t = buy("Nova", d(1, 10), dec!(9), dec!(0), "USD");
        t.side = Side::Transferencia;
        t.from_account = Some("Velha".into());
        t.residue = Some(dec!(1));
        let ops = vec![buy("Velha", d(1, 5), dec!(10), dec!(100), "USD"), t];
        let ev = event(EventKind::Desdobramento, d(2, 1), dec!(4), dec!(0));
        let (effects, _) = apply(&ops, &[ev]);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].account, "Nova");
        assert_eq!(effects[0].after, dec!(36));
    }

    #[test]
    fn valida_proporcao() {
        assert!(event(EventKind::Desdobramento, d(1, 1), dec!(1), dec!(0))
            .validate()
            .is_err());
        assert!(event(EventKind::Grupamento, d(1, 1), dec!(0.5), dec!(0))
            .validate()
            .is_err());
        assert!(event(EventKind::Bonificacao, d(1, 1), dec!(0), dec!(1))
            .validate()
            .is_err());
        assert!(event(EventKind::Bonificacao, d(1, 1), dec!(5), dec!(-1))
            .validate()
            .is_err());
        assert!(event(EventKind::Bonificacao, d(1, 1), dec!(5), dec!(1))
            .validate()
            .is_ok());
    }
}
