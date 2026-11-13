//! ETFs da Bolsa de Londres (LSE) cotados em dólar: em geral fundos
//! irlandeses (UCITS), como o CSPX. Entram como os ativos do exterior, em
//! conta em dólar, com o imposto da Lei 14.754. O código leva o sufixo ".L",
//! o mesmo do Yahoo, para não se confundir com um código dos EUA.

pub struct Etf {
    pub ticker: &'static str,
    pub name: &'static str,
}

const fn etf(ticker: &'static str, name: &'static str) -> Etf {
    Etf { ticker, name }
}

/// Mercado gravado no catálogo.
pub const MARKET: &str = "LSE";

/// Linhas em dólar dos ETFs mais procurados. Só linhas em dólar: as em libra
/// (pence) teriam outro câmbio. Outro ETF em dólar de Londres pode entrar aqui.
pub const ETFS: &[Etf] = &[
    etf("CSPX.L", "iShares Core S&P 500 UCITS ETF USD (Acc)"),
    etf("VUAA.L", "Vanguard S&P 500 UCITS ETF USD (Acc)"),
    etf("VUSD.L", "Vanguard S&P 500 UCITS ETF USD (Dist)"),
    etf("CNDX.L", "iShares NASDAQ 100 UCITS ETF USD (Acc)"),
    etf("IWDA.L", "iShares Core MSCI World UCITS ETF USD (Acc)"),
    etf("SSAC.L", "iShares MSCI ACWI UCITS ETF USD (Acc)"),
    etf("VWRA.L", "Vanguard FTSE All-World UCITS ETF USD (Acc)"),
    etf("VWRD.L", "Vanguard FTSE All-World UCITS ETF USD (Dist)"),
    etf(
        "VHVE.L",
        "Vanguard FTSE Developed World UCITS ETF USD (Acc)",
    ),
    etf("EIMI.L", "iShares Core MSCI EM IMI UCITS ETF USD (Acc)"),
    etf(
        "IB01.L",
        "iShares $ Treasury Bond 0-1yr UCITS ETF USD (Acc)",
    ),
    etf("IGLN.L", "iShares Physical Gold ETC (USD)"),
    etf("IB1T.L", "iShares Bitcoin ETP (USD)"),
];

/// Código de Londres: termina em ".L".
pub fn is_london(ticker: &str) -> bool {
    ticker.len() > 2 && ticker.ends_with(".L")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_de_londres() {
        assert!(is_london("CSPX.L"));
        assert!(!is_london("VOO"));
        assert!(!is_london("BRK.B"));
        assert!(ETFS.iter().all(|e| is_london(e.ticker)));
    }
}
