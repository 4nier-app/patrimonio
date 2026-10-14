//! Tipos básicos do domínio.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Side {
    Compra,
    Venda,
}

impl Side {
    pub fn as_str(&self) -> &'static str {
        match self {
            Side::Compra => "COMPRA",
            Side::Venda => "VENDA",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "COMPRA" => Some(Side::Compra),
            "VENDA" => Some(Side::Venda),
            _ => None,
        }
    }
}

/// Uma compra ou venda já com as taxas atribuídas, em reais.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub asset: String,
    pub date: NaiveDate,
    pub side: Side,
    pub quantity: Decimal,
    pub price: Decimal,
    /// Quantidade × preço.
    pub gross: Decimal,
    /// Taxas atribuídas a esta operação.
    pub fees: Decimal,
}

impl Trade {
    /// Fluxo de caixa para a carteira: compra entra com bruto + taxas;
    /// venda sai com bruto − taxas (valor negativo).
    pub fn cash_flow(&self) -> Decimal {
        match self.side {
            Side::Compra => self.gross + self.fees,
            Side::Venda => -(self.gross - self.fees),
        }
    }

    pub fn signed_quantity(&self) -> Decimal {
        match self.side {
            Side::Compra => self.quantity,
            Side::Venda => -self.quantity,
        }
    }
}

/// Cotação de fechamento usada num cálculo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub id: i64,
    pub asset: String,
    pub date: NaiveDate,
    pub close: Decimal,
    pub source: String,
}
