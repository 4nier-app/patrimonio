//! Lista oficial de ativos da Bolsa de Londres (LSE), publicada todo mês numa
//! planilha. O índice `latest.json` diz onde está a versão atual. O app usa só
//! as abas de ETFs, ETCs e ETNs e só as linhas negociadas em dólar.

use chrono::NaiveDate;

use crate::{http, xlsx, ProviderError, Result};

pub const SOURCE: &str = "LSE";
pub const INDEX_URL: &str = "https://docs.londonstockexchange.com/reports/instruments/latest.json";
const HOST: &str = "https://docs.londonstockexchange.com/";

/// ETF (fundo), ETC (título lastreado em mercadoria) ou ETN (nota do emissor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Etf,
    Etc,
    Etn,
}

impl Kind {
    pub fn code(self) -> &'static str {
        match self {
            Kind::Etf => "ETF",
            Kind::Etc => "ETC",
            Kind::Etn => "ETN",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Listing {
    /// Código com o sufixo do Yahoo: "CSPX.L".
    pub ticker: String,
    pub issuer: String,
    pub name: String,
    pub isin: String,
    /// País de constituição, em inglês, como na planilha ("Ireland").
    pub country: String,
    pub kind: Kind,
    pub start: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct List {
    /// "As at 31 October 2026": a data dos dados da planilha.
    pub as_of: Option<NaiveDate>,
    pub items: Vec<Listing>,
}

/// Endereço da planilha atual, pelo índice. Só aceita arquivo no mesmo site.
pub fn download_link(index: &str) -> Result<String> {
    let v: serde_json::Value = serde_json::from_str(index)
        .map_err(|e| ProviderError::Format(format!("índice da LSE: {e}")))?;
    let links: Vec<&str> = v["Data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["reportType"].as_str() == Some("latest-instrument-list"))
        .filter_map(|d| d["downloadLink"].as_str())
        .collect();
    match links.as_slice() {
        [one] if one.starts_with(HOST) => Ok(one.to_string()),
        [one] => Err(ProviderError::Format(format!(
            "planilha fora do site da LSE: {one}"
        ))),
        _ => Err(ProviderError::Format(
            "índice da LSE sem a lista de ativos".into(),
        )),
    }
}

fn kind_of(sheet: &str) -> Option<Kind> {
    let s = sheet.to_uppercase();
    if s.ends_with("ETFS") {
        Some(Kind::Etf)
    } else if s.ends_with("ETCS") {
        Some(Kind::Etc)
    } else if s.ends_with("ETNS") {
        Some(Kind::Etn)
    } else {
        None
    }
}

/// "As at 31 October 2026" → data.
fn as_at(text: &str) -> Option<NaiveDate> {
    let rest = text.trim().strip_prefix("As at ")?;
    NaiveDate::parse_from_str(rest.trim(), "%d %B %Y").ok()
}

/// Lê a planilha: abas de ETFs, ETCs e ETNs, linhas em dólar.
pub fn parse(bytes: &[u8]) -> Result<List> {
    let sheets = xlsx::sheets(bytes, |n| kind_of(n).is_some())?;
    if sheets.is_empty() {
        return Err(ProviderError::Format(
            "planilha da LSE sem as abas de ETFs".into(),
        ));
    }
    let mut as_of = None;
    let mut items = Vec::new();
    for (name, rows) in sheets {
        let kind = kind_of(&name).expect("filtrada acima");
        let Some(h) = rows
            .iter()
            .position(|r| r.first().map(|c| c.trim()) == Some("TIDM"))
        else {
            return Err(ProviderError::Format(format!("aba {name} sem o cabeçalho")));
        };
        if as_of.is_none() {
            as_of = rows[..h].iter().flatten().find_map(|c| as_at(c));
        }
        let header: Vec<String> = rows[h].iter().map(|c| c.trim().to_lowercase()).collect();
        let col = |n: &str| header.iter().position(|c| c == n);
        let need = |n: &str| {
            col(n).ok_or_else(|| ProviderError::Format(format!("aba {name} sem a coluna {n}")))
        };
        let (tidm, cur) = (need("tidm")?, need("trading currency")?);
        let (issuer, inst, isin) = (col("issuer name"), col("instrument name"), col("isin"));
        let (country, start) = (col("country of incorporation"), col("start date"));
        for r in &rows[h + 1..] {
            let get = |i: Option<usize>| i.and_then(|i| r.get(i)).map(|s| s.trim()).unwrap_or("");
            let code = get(Some(tidm));
            if code.is_empty() || get(Some(cur)) != "USD" {
                continue;
            }
            items.push(Listing {
                ticker: format!("{}.L", code.to_uppercase()),
                issuer: get(issuer).to_string(),
                name: get(inst).to_string(),
                isin: get(isin).to_uppercase(),
                country: get(country).to_string(),
                kind,
                start: NaiveDate::parse_from_str(get(start), "%d/%m/%Y").ok(),
            });
        }
    }
    items.sort_by(|a, b| a.ticker.cmp(&b.ticker));
    items.dedup_by(|a, b| a.ticker == b.ticker);
    Ok(List { as_of, items })
}

pub fn fetch() -> Result<List> {
    let index = http::get_bytes(INDEX_URL)?;
    let link = download_link(&String::from_utf8_lossy(&index))?;
    parse(&http::get_bytes(&link)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indice_aponta_para_a_planilha() {
        let ok = r#"{"Data":[{"reportType":"latest-instrument-list","downloadLink":"https://docs.londonstockexchange.com/reports/instruments/Lista.xlsx"}]}"#;
        assert!(download_link(ok).unwrap().ends_with("Lista.xlsx"));
        let fora = r#"{"Data":[{"reportType":"latest-instrument-list","downloadLink":"https://exemplo.com/x.xlsx"}]}"#;
        assert!(download_link(fora).is_err());
        assert!(download_link(r#"{"Data":[]}"#).is_err());
    }

    #[test]
    fn data_da_planilha() {
        assert_eq!(
            as_at("As at 31 October 2026"),
            NaiveDate::from_ymd_opt(2026, 10, 31)
        );
        assert_eq!(as_at("Number of Instruments: 3"), None);
        assert_eq!(kind_of("1.3 ETFs"), Some(Kind::Etf));
        assert_eq!(kind_of("2.3 ETNs"), Some(Kind::Etn));
        assert_eq!(kind_of("2.1 Bonds"), None);
    }
}

#[cfg(test)]
mod arquivo_real {
    /// Confere com a planilha baixada do site (fora do repositório):
    /// LSE_XLSX=/caminho/Instrument\ list.xlsx cargo test -p carteira-providers -- --ignored
    #[test]
    #[ignore]
    fn le_a_planilha_baixada() {
        let path = std::env::var("LSE_XLSX").expect("LSE_XLSX");
        let list = super::parse(&std::fs::read(path).unwrap()).unwrap();
        println!("{:?} {}", list.as_of, list.items.len());
        let cspx = list.items.iter().find(|l| l.ticker == "CSPX.L").unwrap();
        println!("{cspx:?}");
        assert!(list.items.len() > 1000);
        assert!(list.items.iter().all(|l| l.ticker != "IB1T.L"));
    }
}
