//! Tipos básicos do domínio.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Side {
    Compra,
    Venda,
    /// Mudança de custodiante: a posição muda de conta sem compra nem venda.
    /// Como operação do núcleo, representa só o resíduo baixado na migração.
    Transferencia,
    /// Evento corporativo (bonificação, desdobramento, grupamento): a
    /// quantidade muda (para mais ou para menos) sem dinheiro entrar ou sair.
    /// Gerado a partir dos eventos; nunca é digitado como lançamento.
    Ajuste,
}

impl Side {
    pub fn as_str(&self) -> &'static str {
        match self {
            Side::Compra => "COMPRA",
            Side::Venda => "VENDA",
            Side::Transferencia => "TRANSFERENCIA",
            Side::Ajuste => "AJUSTE",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "COMPRA" => Some(Side::Compra),
            "VENDA" => Some(Side::Venda),
            "TRANSFERENCIA" => Some(Side::Transferencia),
            "AJUSTE" => Some(Side::Ajuste),
            _ => None,
        }
    }
}

pub const BRL: &str = "BRL";

/// Sobra de fração abaixo disto (0,0001 cota) conta como posição zerada:
/// é o arredondamento da corretora, não um ativo na carteira.
pub const DUST: Decimal = Decimal::from_parts(1, 0, 0, false, 4);

/// Quantidade que, para a carteira, é zero.
pub fn is_dust(q: Decimal) -> bool {
    q.abs() < DUST
}

impl Side {
    /// Ordem dentro do mesmo dia: o evento corporativo vale desde a abertura
    /// do pregão, depois as compras, depois vendas e baixas.
    pub fn day_order(&self) -> u8 {
        match self {
            Side::Ajuste => 0,
            Side::Compra => 1,
            Side::Venda | Side::Transferencia => 2,
        }
    }
}
pub const USD: &str = "USD";

fn brl() -> String {
    BRL.to_string()
}

/// Uma compra ou venda já com as taxas atribuídas, na moeda do ativo.
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
    /// Moeda de preço, valor e taxas: "BRL" ou "USD".
    #[serde(default = "brl")]
    pub currency: String,
    /// Conta (corretora) da operação; decide o câmbio do saldo em dólar.
    #[serde(default)]
    pub account: String,
    /// Transferência de custódia: conta de origem e a quantidade que passou
    /// para `account` (o resíduo, se houver, é `quantity`).
    #[serde(default)]
    pub from_account: Option<String>,
    #[serde(default)]
    pub moved: Decimal,
}

impl Trade {
    /// Fluxo de caixa para a carteira: compra entra com bruto + taxas;
    /// venda sai com bruto − taxas (valor negativo).
    pub fn cash_flow(&self) -> Decimal {
        match self.side {
            Side::Compra => self.gross + self.fees,
            Side::Venda => -(self.gross - self.fees),
            Side::Transferencia | Side::Ajuste => Decimal::ZERO,
        }
    }

    pub fn signed_quantity(&self) -> Decimal {
        match self.side {
            Side::Compra | Side::Ajuste => self.quantity,
            Side::Venda | Side::Transferencia => -self.quantity,
        }
    }
}

/// Taxa de câmbio usada num cálculo (PTAX venda de fechamento).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxRate {
    /// Data do boletim usado (último dia útil até a data pedida).
    pub date: NaiveDate,
    /// Reais por dólar.
    pub rate: Decimal,
    pub source: String,
}

/// Fonte de câmbio USD/BRL.
pub trait FxSource {
    /// PTAX: usada nas operações e nos impostos.
    fn usd_brl_on_or_before(&self, date: NaiveDate) -> Option<FxRate>;

    /// Câmbio para avaliar o saldo em dólar de uma conta numa data. Por
    /// padrão é a PTAX; contas com custo para trazer o dinheiro ao Brasil
    /// usam o valor de resgate (dólar comercial menos as taxas).
    fn value_rate(&self, _account: &str, date: NaiveDate) -> Option<FxRate> {
        self.usd_brl_on_or_before(date)
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
