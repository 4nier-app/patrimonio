//! Criptomoedas em carteira própria (autocustódia): lançadas à mão, em reais,
//! sem as taxas da B3. A cotação vem do par em reais no Yahoo ("BTC-BRL").

pub struct Coin {
    pub ticker: &'static str,
    pub name: &'static str,
}

const fn coin(ticker: &'static str, name: &'static str) -> Coin {
    Coin { ticker, name }
}

/// Moedas reconhecidas pelo código. Outra cripto pode entrar nesta lista.
pub const COINS: &[Coin] = &[
    coin("BTC", "Bitcoin"),
    coin("ETH", "Ethereum"),
    coin("USDT", "Tether (dólar digital)"),
    coin("USDC", "USD Coin (dólar digital)"),
    coin("BNB", "BNB"),
    coin("SOL", "Solana"),
    coin("XRP", "XRP"),
    coin("ADA", "Cardano"),
    coin("DOGE", "Dogecoin"),
    coin("TRX", "Tron"),
    coin("TON", "Toncoin"),
    coin("DOT", "Polkadot"),
    coin("POL", "Polygon"),
    coin("LTC", "Litecoin"),
    coin("BCH", "Bitcoin Cash"),
    coin("LINK", "Chainlink"),
    coin("AVAX", "Avalanche"),
    coin("XLM", "Stellar"),
    coin("ATOM", "Cosmos"),
    coin("UNI", "Uniswap"),
    coin("NEAR", "NEAR Protocol"),
    coin("XMR", "Monero"),
    coin("ETC", "Ethereum Classic"),
    coin("SHIB", "Shiba Inu"),
];

pub const KIND: &str = "CRIPTO";

pub fn is_crypto(ticker: &str) -> bool {
    COINS.iter().any(|c| c.ticker == ticker)
}

pub fn name_of(ticker: &str) -> Option<&'static str> {
    COINS.iter().find(|c| c.ticker == ticker).map(|c| c.name)
}

/// Símbolo do par em reais no Yahoo.
pub fn yahoo_symbol(ticker: &str) -> String {
    format!("{ticker}-BRL")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_pelo_codigo() {
        assert!(is_crypto("BTC"));
        assert!(!is_crypto("ABCD3"));
        assert_eq!(name_of("ETH"), Some("Ethereum"));
        assert_eq!(yahoo_symbol("BTC"), "BTC-BRL");
        let mut codes: Vec<_> = COINS.iter().map(|c| c.ticker).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), COINS.len(), "código repetido");
    }
}
