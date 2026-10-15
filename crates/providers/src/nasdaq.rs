//! Catálogo de ações e ETFs listados nos EUA, pelos arquivos públicos do
//! Nasdaq Trader (NASDAQ e demais bolsas: NYSE, NYSE American, Arca, Cboe).

use crate::{http, ProviderError, Result};

pub const SOURCE: &str = "NASDAQ-TRADER";
pub const URLS: &[&str] = &[
    "https://www.nasdaqtrader.com/dynamic/SymDir/nasdaqlisted.txt",
    "https://www.nasdaqtrader.com/dynamic/SymDir/otherlisted.txt",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Listing {
    pub ticker: String,
    pub name: String,
    pub etf: bool,
}

/// Interpreta um dos arquivos (separado por "|", com cabeçalho). Ignora
/// emissões de teste e classes com "$" (ações preferenciais e direitos).
pub fn parse(text: &str) -> Result<Vec<Listing>> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines
        .next()
        .ok_or_else(|| ProviderError::Format("arquivo vazio".into()))?
        .split('|')
        .collect();
    let col = |names: &[&str]| header.iter().position(|h| names.contains(&h.trim()));
    let symbol = col(&["Symbol", "ACT Symbol"])
        .ok_or_else(|| ProviderError::Format("sem coluna de código".into()))?;
    let name = col(&["Security Name"])
        .ok_or_else(|| ProviderError::Format("sem coluna de nome".into()))?;
    let etf = col(&["ETF"]);
    let test = col(&["Test Issue"]);
    let mut out = Vec::new();
    for line in lines {
        if line.starts_with("File Creation Time") || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('|').collect();
        let get = |i: Option<usize>| i.and_then(|i| f.get(i)).map(|s| s.trim()).unwrap_or("");
        let ticker = get(Some(symbol));
        if ticker.is_empty() || ticker.contains('$') || get(test) == "Y" {
            continue;
        }
        out.push(Listing {
            ticker: ticker.to_uppercase(),
            name: get(Some(name)).to_string(),
            etf: get(etf) == "Y",
        });
    }
    Ok(out)
}

pub fn fetch_all() -> Result<Vec<Listing>> {
    let mut all = Vec::new();
    for url in URLS {
        let bytes = http::get_bytes(url)?;
        all.extend(parse(&String::from_utf8_lossy(&bytes))?);
    }
    all.sort_by(|a, b| a.ticker.cmp(&b.ticker));
    all.dedup_by(|a, b| a.ticker == b.ticker);
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_os_dois_formatos() {
        // Linhas sintéticas no formato dos arquivos.
        let nasdaq = "Symbol|Security Name|Market Category|Test Issue|Financial Status|Round Lot Size|ETF|NextShares\n\
            AAAA|Empresa A Inc. - Common Stock|Q|N|N|100|N|N\n\
            ZXZZT|Teste|G|Y|N|100|N|N\n\
            File Creation Time: 0101202600:00|||||||\n";
        let other = "ACT Symbol|Security Name|Exchange|CQS Symbol|ETF|Round Lot Size|Test Issue|NASDAQ Symbol\n\
            BBB.B|Empresa B Class B|N|BBB.B|N|100|N|BBB.B\n\
            CCC$A|Empresa C Pref|N|CCCpA|N|100|N|CCC-A\n\
            ETFX|Fundo X ETF|P|ETFX|Y|100|N|ETFX\n";
        let a = parse(nasdaq).unwrap();
        assert_eq!(
            a,
            vec![Listing {
                ticker: "AAAA".into(),
                name: "Empresa A Inc. - Common Stock".into(),
                etf: false
            }]
        );
        let b = parse(other).unwrap();
        assert_eq!(
            b.iter().map(|l| l.ticker.as_str()).collect::<Vec<_>>(),
            vec!["BBB.B", "ETFX"]
        );
        assert!(b[1].etf);
    }
}
