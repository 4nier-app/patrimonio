//! Fontes de dados externas. Cada função de rede só baixa e interpreta; quem
//! grava no cofre e registra o que foi atualizado é o aplicativo.
//!
//! Todas são gratuitas e públicas:
//! - B3, séries históricas COTAHIST: catálogo de ativos e fechamentos oficiais.
//! - Yahoo Finance (gráfico diário): fechamentos do mês corrente e último preço.
//! - Banco Central (Olinda): PTAX do dólar.
//! - Banco Central (SGS): CDI e IPCA, para comparar com a carteira.
//! - Nasdaq Trader: catálogo de ações e ETFs dos EUA.
//! - CVM (dados abertos): cota diária e cadastro dos fundos de investimento.

pub mod coinbase;
pub mod cotahist;
pub mod cvm;
pub mod http;
pub mod nasdaq;
pub mod ptax;
pub mod sgs;
pub mod yahoo;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("sem conexão ou servidor indisponível: {0}")]
    Network(String),
    #[error("arquivo não encontrado na fonte: {0}")]
    NotFound(String),
    #[error("resposta em formato inesperado: {0}")]
    Format(String),
}

pub type Result<T> = std::result::Result<T, ProviderError>;
