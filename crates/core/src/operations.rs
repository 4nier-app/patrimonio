//! Lançamentos de compra e venda e a atribuição das taxas B3.
//!
//! As operações de uma mesma conta no mesmo pregão formam uma nota. As taxas
//! são calculadas sobre o total da nota e rateadas entre as operações (ver
//! [`crate::fees`]). Uma operação com taxa informada à mão fica fora do rateio
//! e usa o valor informado.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::fees::{compute_note_fees, FeeError, FeeRule, NoteFees};
use crate::model::{Side, Trade};

/// Operação como o usuário lançou.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub account: String,
    pub asset: String,
    pub date: NaiveDate,
    pub side: Side,
    pub quantity: Decimal,
    pub price: Decimal,
    /// Taxas informadas à mão (substituem o cálculo automático).
    pub manual_fees: Option<Decimal>,
    pub note: Option<String>,
}

impl Operation {
    /// Valor da operação: quantidade × preço, arredondado ao centavo.
    pub fn gross(&self) -> Decimal {
        (self.quantity * self.price).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
    }
}

/// Validação dos campos digitados.
pub fn validate(op: &Operation) -> Result<(), String> {
    if op.asset.trim().is_empty() {
        return Err("informe o ativo".into());
    }
    if op.account.trim().is_empty() {
        return Err("informe a conta (corretora)".into());
    }
    if op.quantity <= Decimal::ZERO {
        return Err("a quantidade precisa ser maior que zero".into());
    }
    if op.price <= Decimal::ZERO {
        return Err("o preço precisa ser maior que zero".into());
    }
    if let Some(f) = op.manual_fees {
        if f < Decimal::ZERO {
            return Err("a taxa não pode ser negativa".into());
        }
    }
    Ok(())
}

/// Uma nota (conta + pregão) com o cálculo das taxas.
#[derive(Debug, Clone, Serialize)]
pub struct NoteGroup {
    pub account: String,
    pub date: NaiveDate,
    /// Ids das operações que entraram no rateio, na ordem do cálculo.
    pub operation_ids: Vec<String>,
    pub fees: Option<NoteFees>,
}

/// Operação com as taxas atribuídas e a origem delas.
#[derive(Debug, Clone, Serialize)]
pub struct PricedOperation {
    pub operation: Operation,
    pub gross: Decimal,
    pub fees: Decimal,
    /// "automática" ou "informada".
    pub fee_origin: &'static str,
}

impl PricedOperation {
    pub fn trade(&self) -> Trade {
        Trade {
            id: self.operation.id.clone(),
            asset: self.operation.asset.clone(),
            date: self.operation.date,
            side: self.operation.side,
            quantity: self.operation.quantity,
            price: self.operation.price,
            gross: self.gross,
            fees: self.fees,
        }
    }
}

/// Atribui as taxas a cada operação, agrupando por conta e pregão.
pub fn price_operations(rules: &[FeeRule], ops: &[Operation]) -> Result<(Vec<PricedOperation>, Vec<NoteGroup>), FeeError> {
    let mut groups: BTreeMap<(String, NaiveDate), Vec<&Operation>> = BTreeMap::new();
    for op in ops.iter().filter(|o| o.manual_fees.is_none()) {
        groups.entry((op.account.clone(), op.date)).or_default().push(op);
    }
    let mut auto: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut notes = Vec::new();
    for ((account, date), members) in groups {
        let values: Vec<Decimal> = members.iter().map(|o| o.gross()).collect();
        let fees = compute_note_fees(rules, date, &values)?;
        for (i, op) in members.iter().enumerate() {
            auto.insert(op.id.clone(), fees.fees_of(i));
        }
        notes.push(NoteGroup {
            account,
            date,
            operation_ids: members.iter().map(|o| o.id.clone()).collect(),
            fees: Some(fees),
        });
    }
    let priced = ops
        .iter()
        .map(|op| {
            let (fees, fee_origin) = match op.manual_fees {
                Some(f) => (f, "informada"),
                None => (auto.get(&op.id).copied().unwrap_or_default(), "automática"),
            };
            PricedOperation { operation: op.clone(), gross: op.gross(), fees, fee_origin }
        })
        .collect();
    Ok((priced, notes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fees::default_rules;
    use rust_decimal_macros::dec;

    fn op(id: &str, account: &str, d: u32, q: Decimal, p: Decimal, manual: Option<Decimal>) -> Operation {
        Operation {
            id: id.into(),
            account: account.into(),
            asset: "TEST3".into(),
            date: NaiveDate::from_ymd_opt(2026, 3, d).unwrap(),
            side: Side::Compra,
            quantity: q,
            price: p,
            manual_fees: manual,
            note: None,
        }
    }

    #[test]
    fn agrupa_por_conta_e_pregao() {
        let ops = vec![
            op("a", "Corretora X", 2, dec!(100), dec!(480.00), None),
            op("b", "Corretora X", 2, dec!(3), dec!(50.0033), None),
            op("c", "Corretora Y", 2, dec!(10), dec!(10), None),
            op("d", "Corretora X", 3, dec!(10), dec!(10), Some(dec!(1.23))),
        ];
        let (priced, notes) = price_operations(&default_rules(), &ops).unwrap();
        assert_eq!(notes.len(), 2);
        // 48.000,00 + 150,01: soma das taxas da nota = soma das partes.
        let x = &notes[0];
        assert_eq!(x.operation_ids, vec!["a", "b"]);
        let note_total = x.fees.as_ref().unwrap().total_fees();
        assert_eq!(priced[0].fees + priced[1].fees, note_total);
        assert_eq!(priced[1].gross, dec!(150.01));
        assert_eq!(priced[3].fees, dec!(1.23));
        assert_eq!(priced[3].fee_origin, "informada");
    }

    #[test]
    fn valida_campos() {
        assert!(validate(&op("a", "X", 2, dec!(0), dec!(1), None)).is_err());
        assert!(validate(&op("a", "X", 2, dec!(1), dec!(-1), None)).is_err());
        assert!(validate(&op("a", "", 2, dec!(1), dec!(1), None)).is_err());
        assert!(validate(&op("a", "X", 2, dec!(1), dec!(1), None)).is_ok());
    }
}
