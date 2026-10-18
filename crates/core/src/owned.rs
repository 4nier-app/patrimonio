//! Bens (imóvel e veículo) informados à mão. O valor no patrimônio é o custo:
//! o preço pago na compra mais as obras (só no imóvel), como pede a ficha
//! Bens e Direitos. Não há valor de mercado nem estimativa.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OwnedKind {
    #[default]
    Imovel,
    Veiculo,
}

impl OwnedKind {
    pub fn as_str(self) -> &'static str {
        match self {
            OwnedKind::Imovel => "IMOVEL",
            OwnedKind::Veiculo => "VEICULO",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "IMOVEL" => OwnedKind::Imovel,
            "VEICULO" => OwnedKind::Veiculo,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            OwnedKind::Imovel => "Imóvel",
            OwnedKind::Veiculo => "Veículo",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnedAsset {
    pub id: String,
    pub kind: OwnedKind,
    /// "Apartamento Rua X", "Carro Onix".
    pub name: String,
    pub bought_on: NaiveDate,
    /// Valor pago na compra.
    pub price: Decimal,
    /// Data da venda: depois dela o bem sai do patrimônio.
    pub sold_on: Option<NaiveDate>,
    pub note: Option<String>,
}

/// Obra ou reforma do imóvel: soma ao custo a partir da data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Improvement {
    pub id: String,
    pub owned_id: String,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub description: String,
}

impl OwnedAsset {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("informe um nome para o bem".into());
        }
        if self.price <= Decimal::ZERO {
            return Err("o valor da compra precisa ser maior que zero".into());
        }
        if self.sold_on.is_some_and(|s| s < self.bought_on) {
            return Err("a venda é antes da compra".into());
        }
        Ok(())
    }

    /// Custo no fim de `date` (compra + obras até a data); nada antes da
    /// compra nem depois da venda.
    pub fn cost_on(&self, improvements: &[Improvement], date: NaiveDate) -> Option<Decimal> {
        if date < self.bought_on || self.sold_on.is_some_and(|s| date >= s) {
            return None;
        }
        let works: Decimal = improvements
            .iter()
            .filter(|i| i.owned_id == self.id && i.date <= date)
            .map(|i| i.amount)
            .sum();
        Some(self.price + works)
    }
}

impl Improvement {
    pub fn validate(&self, owner: &OwnedAsset) -> Result<(), String> {
        if owner.kind != OwnedKind::Imovel {
            return Err("obras só valem para imóvel".into());
        }
        if self.amount <= Decimal::ZERO {
            return Err("o valor da obra precisa ser maior que zero".into());
        }
        if self.description.trim().is_empty() {
            return Err("descreva a obra (ex.: reforma da cozinha)".into());
        }
        if self.date < owner.bought_on {
            return Err("a obra é antes da compra do imóvel".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn custo_da_compra_mais_obras() {
        let casa = OwnedAsset {
            id: "b1".into(),
            kind: OwnedKind::Imovel,
            name: "Casa".into(),
            bought_on: d(2020, 5, 10),
            price: dec!(300000),
            sold_on: Some(d(2026, 6, 1)),
            note: None,
        };
        let obra = Improvement {
            id: "o1".into(),
            owned_id: "b1".into(),
            date: d(2022, 3, 1),
            amount: dec!(50000),
            description: "Reforma da cozinha".into(),
        };
        let works = vec![obra.clone()];
        assert_eq!(casa.cost_on(&works, d(2020, 5, 9)), None);
        assert_eq!(casa.cost_on(&works, d(2021, 12, 31)), Some(dec!(300000)));
        assert_eq!(casa.cost_on(&works, d(2022, 12, 31)), Some(dec!(350000)));
        assert_eq!(casa.cost_on(&works, d(2026, 6, 1)), None);
        assert!(obra.validate(&casa).is_ok());
        let carro = OwnedAsset {
            kind: OwnedKind::Veiculo,
            ..casa
        };
        assert!(obra.validate(&carro).is_err());
    }
}
