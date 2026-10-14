//! Taxas da B3 cobradas por nota de corretagem (pregão).
//!
//! Regra (pessoa física, mercado à vista, operação normal): cada taxa é a
//! alíquota vigente aplicada ao valor total das operações do pregão,
//! **truncada** no centavo. Depois, cada taxa é rateada entre as operações na
//! proporção do valor de cada uma; o resíduo de centavos vai para a operação
//! de maior valor. Ver docs/ARQUITETURA.md, seção 7.

use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FeeKind {
    Liquidacao,
    Emolumentos,
    Transferencia,
}

impl FeeKind {
    pub fn label(&self) -> &'static str {
        match self {
            FeeKind::Liquidacao => "Taxa de liquidação/CCP",
            FeeKind::Emolumentos => "Emolumentos",
            FeeKind::Transferencia => "Taxa de transferência de ativos",
        }
    }
}

/// Alíquota de uma taxa, válida a partir de uma data (inclusive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeeRule {
    pub id: String,
    pub kind: FeeKind,
    /// Fração decimal: 0,0224% = 0.000224.
    pub rate: Decimal,
    pub valid_from: NaiveDate,
    pub source: String,
}

/// Tabela padrão. Conferida centavo a centavo contra notas reais de corretora.
pub fn default_rules() -> Vec<FeeRule> {
    let from = NaiveDate::from_ymd_opt(2024, 1, 1).expect("data válida");
    let src = "Tabela de tarifas B3, pessoa física, à vista, operação normal".to_string();
    vec![
        FeeRule { id: "b3-liq-2024".into(), kind: FeeKind::Liquidacao, rate: Decimal::new(224, 6), valid_from: from, source: src.clone() },
        FeeRule { id: "b3-emol-2024".into(), kind: FeeKind::Emolumentos, rate: Decimal::new(5, 5), valid_from: from, source: src.clone() },
        FeeRule { id: "b3-transf-2024".into(), kind: FeeKind::Transferencia, rate: Decimal::new(26, 6), valid_from: from, source: src },
    ]
}

/// Regra vigente para um tipo de taxa numa data: a de `valid_from` mais recente
/// que não seja posterior à data do pregão.
pub fn rule_for<'a>(rules: &'a [FeeRule], kind: FeeKind, date: NaiveDate) -> Option<&'a FeeRule> {
    rules
        .iter()
        .filter(|r| r.kind == kind && r.valid_from <= date)
        .max_by_key(|r| r.valid_from)
}

/// Taxa calculada sobre o total da nota, com a memória de cálculo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteFee {
    pub kind: FeeKind,
    pub rule_id: String,
    pub base: Decimal,
    pub rate: Decimal,
    pub amount: Decimal,
    /// Fórmula com os números substituídos, para exibição ao usuário.
    pub formula: String,
}

/// Parcela de uma taxa atribuída a uma operação da nota.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeeShare {
    pub operation_index: usize,
    pub kind: FeeKind,
    pub amount: Decimal,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteFees {
    pub total_operations: Decimal,
    pub fees: Vec<NoteFee>,
    pub shares: Vec<FeeShare>,
}

impl NoteFees {
    pub fn total_fees(&self) -> Decimal {
        self.fees.iter().map(|f| f.amount).sum()
    }

    /// Soma das taxas atribuídas a uma operação.
    pub fn fees_of(&self, operation_index: usize) -> Decimal {
        self.shares.iter().filter(|s| s.operation_index == operation_index).map(|s| s.amount).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeeError {
    NoOperations,
    MissingRule(FeeKind),
}


impl std::fmt::Display for FeeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeeError::NoOperations => write!(f, "a nota não tem operações"),
            FeeError::MissingRule(k) => write!(f, "não há alíquota vigente para {}", k.label()),
        }
    }
}

impl std::error::Error for FeeError {}

fn trunc_cents(v: Decimal) -> Decimal {
    v.round_dp_with_strategy(2, RoundingStrategy::ToZero)
}

fn fmt_brl(v: Decimal) -> String {
    let s = format!("{:.2}", v);
    let (int, frac) = s.split_once('.').unwrap_or((&s, "00"));
    let neg = int.starts_with('-');
    let digits = int.trim_start_matches('-');
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(c);
    }
    format!("R$ {}{},{}", if neg { "-" } else { "" }, grouped, frac)
}

fn fmt_pct(rate: Decimal) -> String {
    let pct = (rate * Decimal::ONE_HUNDRED).normalize();
    format!("{}%", pct.to_string().replace('.', ","))
}

/// Calcula as taxas B3 de uma nota a partir dos valores brutos das operações.
pub fn compute_note_fees(
    rules: &[FeeRule],
    trade_date: NaiveDate,
    operation_values: &[Decimal],
) -> Result<NoteFees, FeeError> {
    if operation_values.is_empty() {
        return Err(FeeError::NoOperations);
    }
    let total: Decimal = operation_values.iter().sum();
    let largest = operation_values
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0)))
        .map(|(i, _)| i)
        .expect("lista não vazia");

    let mut fees = Vec::new();
    let mut shares = Vec::new();
    for kind in [FeeKind::Liquidacao, FeeKind::Emolumentos, FeeKind::Transferencia] {
        let rule = rule_for(rules, kind, trade_date).ok_or(FeeError::MissingRule(kind))?;
        let amount = trunc_cents(total * rule.rate);
        fees.push(NoteFee {
            kind,
            rule_id: rule.id.clone(),
            base: total,
            rate: rule.rate,
            amount,
            formula: format!(
                "trunca_centavo({} × {}) = {}",
                fmt_brl(total),
                fmt_pct(rule.rate),
                fmt_brl(amount)
            ),
        });

        let mut allocated = Decimal::ZERO;
        let mut kind_shares: Vec<FeeShare> = operation_values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let part = if total.is_zero() { Decimal::ZERO } else { trunc_cents(amount * v / total) };
                allocated += part;
                FeeShare { operation_index: i, kind, amount: part }
            })
            .collect();
        kind_shares[largest].amount += amount - allocated;
        shares.extend(kind_shares);
    }

    Ok(NoteFees { total_operations: total, fees, shares })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn exemplo_ficticio_da_documentacao() {
        let r = compute_note_fees(&default_rules(), d(2026, 1, 15), &[dec!(100000.00)]).unwrap();
        let amounts: Vec<_> = r.fees.iter().map(|f| f.amount).collect();
        assert_eq!(amounts, vec![dec!(22.40), dec!(5.00), dec!(2.60)]);
        assert_eq!(r.total_fees(), dec!(30.00));
        assert_eq!(r.fees[0].formula, "trunca_centavo(R$ 100.000,00 × 0,0224%) = R$ 22,40");
    }

    #[test]
    fn exemplo_petr4_da_secao_5() {
        let r = compute_note_fees(&default_rules(), d(2026, 1, 15), &[dec!(3800.00)]).unwrap();
        let amounts: Vec<_> = r.fees.iter().map(|f| f.amount).collect();
        assert_eq!(amounts, vec![dec!(0.85), dec!(0.19), dec!(0.09)]);
        assert_eq!(r.total_fees(), dec!(1.13));
    }

    #[test]
    fn trunca_em_vez_de_arredondar() {
        // 99.990,00 × 0,005% = 4,9995 → 4,99 (arredondar daria 5,00).
        let r = compute_note_fees(&default_rules(), d(2026, 1, 15), &[dec!(99990.00)]).unwrap();
        assert_eq!(r.fees[1].amount, dec!(4.99));
    }

    #[test]
    fn rateio_fecha_com_o_total_e_residuo_vai_para_a_maior() {
        let values = [dec!(150.01), dec!(48000.00), dec!(3333.33)];
        let r = compute_note_fees(&default_rules(), d(2026, 1, 15), &values).unwrap();
        for fee in &r.fees {
            let sum: Decimal = r.shares.iter().filter(|s| s.kind == fee.kind).map(|s| s.amount).sum();
            assert_eq!(sum, fee.amount, "{:?}", fee.kind);
        }
        let all: Decimal = (0..values.len()).map(|i| r.fees_of(i)).sum();
        assert_eq!(all, r.total_fees());
        assert!(r.fees_of(1) > r.fees_of(2));
    }

    #[test]
    fn usa_a_regra_vigente_na_data() {
        let mut rules = default_rules();
        rules.push(FeeRule {
            id: "b3-emol-2027".into(),
            kind: FeeKind::Emolumentos,
            rate: dec!(0.0001),
            valid_from: d(2027, 1, 1),
            source: "teste".into(),
        });
        let antes = compute_note_fees(&rules, d(2026, 12, 31), &[dec!(10000)]).unwrap();
        let depois = compute_note_fees(&rules, d(2027, 1, 2), &[dec!(10000)]).unwrap();
        assert_eq!(antes.fees[1].amount, dec!(0.50));
        assert_eq!(depois.fees[1].amount, dec!(1.00));
        assert_eq!(depois.fees[1].rule_id, "b3-emol-2027");
    }

    #[test]
    fn nota_vazia_e_erro() {
        assert_eq!(compute_note_fees(&default_rules(), d(2026, 1, 1), &[]), Err(FeeError::NoOperations));
    }
}
