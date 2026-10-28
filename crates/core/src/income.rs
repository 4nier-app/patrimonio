//! Proventos: dividendos, juros sobre capital próprio, rendimentos de FIIs e
//! juros recebidos. Ficam guardados à parte e entram no lucro do mês do
//! pagamento na rentabilidade (seção 5.1 da arquitetura), como o CDI, que já
//! inclui todo o rendimento.

use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::model::{FxRate, FxSource, BRL};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum IncomeKind {
    /// Dividendo (isento no Brasil; no exterior, tributado no ajuste anual).
    Dividendo,
    /// Juros sobre capital próprio (15% retido na fonte).
    Jcp,
    /// Rendimento de fundo imobiliário.
    Rendimento,
    /// Juros (renda fixa, saldo remunerado).
    Juros,
    /// Outros (bonificação em dinheiro, restituição de capital, frações).
    Outro,
}

impl IncomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            IncomeKind::Dividendo => "DIVIDENDO",
            IncomeKind::Jcp => "JCP",
            IncomeKind::Rendimento => "RENDIMENTO",
            IncomeKind::Juros => "JUROS",
            IncomeKind::Outro => "OUTRO",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "DIVIDENDO" => IncomeKind::Dividendo,
            "JCP" => IncomeKind::Jcp,
            "RENDIMENTO" => IncomeKind::Rendimento,
            "JUROS" => IncomeKind::Juros,
            "OUTRO" => IncomeKind::Outro,
            _ => return None,
        })
    }
}

/// Provento como o usuário lançou: o valor total recebido na conta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Income {
    pub id: String,
    pub account: String,
    pub asset: String,
    pub kind: IncomeKind,
    /// Data em que o dinheiro caiu na conta.
    pub pay_date: NaiveDate,
    /// Data com (último dia com direito), quando informada.
    #[serde(default)]
    pub ex_date: Option<NaiveDate>,
    /// Valor bruto, antes do imposto retido.
    pub gross: Decimal,
    /// Imposto retido na fonte (no Brasil ou no exterior).
    #[serde(default)]
    pub withheld: Decimal,
    /// Moeda da conta.
    pub currency: String,
    pub note: Option<String>,
}

impl Income {
    /// Valor que caiu na conta.
    pub fn net(&self) -> Decimal {
        self.gross - self.withheld
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.asset.trim().is_empty() {
            return Err("informe o ativo".into());
        }
        if self.account.trim().is_empty() {
            return Err("informe a conta".into());
        }
        if self.gross <= Decimal::ZERO {
            return Err("o valor bruto precisa ser maior que zero".into());
        }
        if self.withheld < Decimal::ZERO {
            return Err("o imposto retido não pode ser negativo".into());
        }
        if self.withheld > self.gross {
            return Err("o imposto retido não pode ser maior que o valor bruto".into());
        }
        if self.ex_date.is_some_and(|d| d > self.pay_date) {
            return Err("a data com não pode ser depois do pagamento".into());
        }
        Ok(())
    }
}

/// Provento com os valores em reais (exterior pela PTAX do dia do pagamento).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PricedIncome {
    pub income: Income,
    pub net: Decimal,
    /// Vazios se faltar a PTAX do dia.
    pub gross_brl: Option<Decimal>,
    pub withheld_brl: Option<Decimal>,
    pub net_brl: Option<Decimal>,
    pub fx: Option<FxRate>,
}

pub fn price_income(income: Income, fx: &dyn FxSource) -> PricedIncome {
    let net = income.net();
    let (rate, used) = if income.currency == BRL {
        (Some(Decimal::ONE), None)
    } else {
        match fx.usd_brl_on_or_before(income.pay_date) {
            Some(r) => (Some(r.rate), Some(r)),
            None => (None, None),
        }
    };
    let brl = |v: Decimal| {
        rate.map(|r| (v * r).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero))
    };
    let gross_brl = brl(income.gross);
    let withheld_brl = brl(income.withheld);
    PricedIncome {
        // Líquido = bruto − retido em reais, para a soma sempre fechar.
        net_brl: gross_brl.zip(withheld_brl).map(|(g, w)| g - w),
        gross_brl,
        withheld_brl,
        net,
        fx: used,
        income,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    struct Fx;
    impl FxSource for Fx {
        fn usd_brl_on_or_before(&self, date: NaiveDate) -> Option<FxRate> {
            (date >= NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()).then(|| FxRate {
                date,
                rate: dec!(5.5),
                source: "teste".into(),
            })
        }
    }

    fn income(currency: &str, gross: Decimal, withheld: Decimal) -> Income {
        Income {
            id: "x".into(),
            account: "Conta".into(),
            asset: "TEST3".into(),
            kind: IncomeKind::Jcp,
            pay_date: NaiveDate::from_ymd_opt(2026, 3, 10).unwrap(),
            ex_date: Some(NaiveDate::from_ymd_opt(2026, 2, 20).unwrap()),
            gross,
            withheld,
            currency: currency.into(),
            note: None,
        }
    }

    #[test]
    fn jcp_em_reais_e_dividendo_em_dolar() {
        let p = price_income(income("BRL", dec!(100), dec!(15)), &Fx);
        assert_eq!(p.net, dec!(85));
        assert_eq!(p.net_brl, Some(dec!(85)));
        assert!(p.fx.is_none());

        let p = price_income(income("USD", dec!(10.33), dec!(3.10)), &Fx);
        assert_eq!(p.net, dec!(7.23));
        assert_eq!(p.gross_brl, Some(dec!(56.82)));
        assert_eq!(p.withheld_brl, Some(dec!(17.05)));
        assert_eq!(p.net_brl, Some(dec!(39.77)));

        let mut old = income("USD", dec!(1), dec!(0));
        old.pay_date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        old.ex_date = None;
        assert_eq!(price_income(old, &Fx).net_brl, None);
    }

    #[test]
    fn valida_valores_e_datas() {
        assert!(income("BRL", dec!(100), dec!(15)).validate().is_ok());
        assert!(income("BRL", dec!(0), dec!(0)).validate().is_err());
        assert!(income("BRL", dec!(10), dec!(11)).validate().is_err());
        assert!(income("BRL", dec!(10), dec!(-1)).validate().is_err());
        let mut late = income("BRL", dec!(10), dec!(0));
        late.ex_date = Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());
        assert!(late.validate().is_err());
        assert_eq!(IncomeKind::parse("JCP"), Some(IncomeKind::Jcp));
        assert_eq!(IncomeKind::parse("X"), None);
    }
}
