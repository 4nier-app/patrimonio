//! Perfil do ativo no Yahoo Finance (serviço `quoteSummary`): as maiores
//! posições e o peso de cada setor de um ETF, a taxa de administração, a
//! gestora e o setor de uma ação.
//!
//! O serviço é gratuito, mas pede um cookie de sessão anônima (criado ao abrir
//! `fc.yahoo.com`) e um código ("crumb") ligado a esse cookie. Os dois ficam
//! guardados na memória enquanto o app está aberto.
//!
//! Os textos do Yahoo vêm em inglês: daqui só saem números, códigos e nomes
//! próprios (empresas, gestora) e as chaves dos setores, que o app traduz.

use std::str::FromStr;
use std::sync::Mutex;

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{http, ProviderError, Result};

pub const SOURCE: &str = "Yahoo Finance";

/// Uma das maiores posições do ETF.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Holding {
    pub symbol: String,
    pub name: String,
    /// Fração do ETF (0,0835 = 8,35%).
    pub weight: Decimal,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Profile {
    /// Taxa de administração ao ano, em fração (0,0003 = 0,03%).
    pub expense_ratio: Option<Decimal>,
    /// Gestora (Vanguard, iShares...).
    pub family: Option<String>,
    /// Setor da ação, como o Yahoo escreve ("Technology").
    pub sector: Option<String>,
    pub holdings: Vec<Holding>,
    /// (chave do setor, fração do ETF), do maior para o menor.
    pub sectors: Vec<(String, Decimal)>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    quote_summary: Summary,
}

#[derive(Deserialize)]
struct Summary {
    result: Option<Vec<Modules>>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Modules {
    top_holdings: Option<TopHoldings>,
    fund_profile: Option<FundProfile>,
    asset_profile: Option<AssetProfile>,
}

#[derive(Deserialize)]
struct Raw {
    raw: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TopHoldings {
    #[serde(default)]
    holdings: Vec<RawHolding>,
    #[serde(default)]
    sector_weightings: Vec<std::collections::BTreeMap<String, Raw>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawHolding {
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    holding_name: String,
    holding_percent: Option<Raw>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FundProfile {
    family: Option<String>,
    fees_expenses_investment: Option<Fees>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fees {
    annual_report_expense_ratio: Option<Raw>,
}

#[derive(Deserialize)]
struct AssetProfile {
    sector: Option<String>,
}

fn dec(v: f64) -> Option<Decimal> {
    Decimal::from_str(&format!("{v:.8}"))
        .ok()
        .map(|d| d.normalize())
}

pub fn parse(json: &[u8]) -> Result<Profile> {
    let env: Envelope =
        serde_json::from_slice(json).map_err(|e| ProviderError::Format(e.to_string()))?;
    if let Some(err) = env.quote_summary.error.filter(|e| !e.is_null()) {
        return Err(ProviderError::NotFound(err.to_string()));
    }
    let m = env
        .quote_summary
        .result
        .and_then(|r| r.into_iter().next())
        .ok_or_else(|| ProviderError::Format("resposta sem resultado".into()))?;
    let mut out = Profile::default();
    if let Some(t) = m.top_holdings {
        out.holdings = t
            .holdings
            .into_iter()
            .filter_map(|h| {
                let w = h.holding_percent.and_then(|r| r.raw).and_then(dec)?;
                (w > Decimal::ZERO).then(|| Holding {
                    symbol: h.symbol.trim().to_string(),
                    name: h.holding_name.trim().to_string(),
                    weight: w,
                })
            })
            .collect();
        out.sectors = t
            .sector_weightings
            .into_iter()
            .flat_map(|m| m.into_iter())
            .filter_map(|(k, r)| Some((k, r.raw.and_then(dec)?)))
            .filter(|(_, w)| *w > Decimal::ZERO)
            .collect();
        out.sectors.sort_by(|a, b| b.1.cmp(&a.1));
    }
    if let Some(f) = m.fund_profile {
        out.family = f.family.filter(|s| !s.trim().is_empty());
        out.expense_ratio = f
            .fees_expenses_investment
            .and_then(|x| x.annual_report_expense_ratio)
            .and_then(|r| r.raw)
            .and_then(dec)
            .filter(|v| *v >= Decimal::ZERO);
    }
    if let Some(a) = m.asset_profile {
        out.sector = a.sector.filter(|s| !s.trim().is_empty());
    }
    Ok(out)
}

/// Codifica o "crumb" para ir no endereço (pode ter "/", "+" ou "=").
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

static SESSION: Mutex<Option<(String, String)>> = Mutex::new(None);

fn session(fresh: bool) -> Result<(String, String)> {
    let mut guard = SESSION
        .lock()
        .map_err(|_| ProviderError::Network("estado interno".into()))?;
    if !fresh {
        if let Some(s) = guard.as_ref() {
            return Ok(s.clone());
        }
    }
    let first = http::get_reply("https://fc.yahoo.com", None)?;
    let cookie = first.cookies.join("; ");
    if cookie.is_empty() {
        return Err(ProviderError::Network(
            "o Yahoo não abriu a sessão anônima".into(),
        ));
    }
    let crumb = http::get_reply(
        "https://query2.finance.yahoo.com/v1/test/getcrumb",
        Some(&cookie),
    )?;
    let text = String::from_utf8_lossy(&crumb.body).trim().to_string();
    if crumb.status != 200 || text.is_empty() || text.contains('<') || text.len() > 64 {
        return Err(ProviderError::Network(format!(
            "o Yahoo recusou a sessão (HTTP {})",
            crumb.status
        )));
    }
    let s = (cookie, text);
    *guard = Some(s.clone());
    Ok(s)
}

pub fn url(symbol: &str, crumb: &str) -> String {
    format!(
        "https://query2.finance.yahoo.com/v10/finance/quoteSummary/{}?modules=topHoldings%2CfundProfile%2CassetProfile&crumb={}",
        symbol.replace('^', "%5E"),
        encode(crumb)
    )
}

pub fn fetch(symbol: &str) -> Result<Profile> {
    for fresh in [false, true] {
        let (cookie, crumb) = session(fresh)?;
        let reply = http::get_reply(&url(symbol, &crumb), Some(&cookie))?;
        match reply.status {
            200 => return parse(&reply.body),
            // Sessão vencida: abre outra e tenta de novo, uma vez.
            401 | 403 if !fresh => continue,
            404 => return Err(ProviderError::NotFound(symbol.to_string())),
            s => {
                return Err(ProviderError::Network(format!(
                    "HTTP {s} no perfil do Yahoo"
                )))
            }
        }
    }
    Err(ProviderError::Network("o Yahoo recusou a sessão".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn interpreta_perfil_de_etf() {
        // Resposta sintética no formato do serviço.
        let json = br#"{"quoteSummary":{"result":[{
            "topHoldings":{"holdings":[
                {"symbol":"AAAA","holdingName":"Empresa A","holdingPercent":{"raw":0.0835,"fmt":"8.35%"}},
                {"symbol":"BBBB","holdingName":"Empresa B","holdingPercent":{"raw":0.05,"fmt":"5%"}}],
              "sectorWeightings":[{"realestate":{"raw":0.02}},{"technology":{"raw":0.33}},{"energy":{"raw":0}}]},
            "fundProfile":{"family":"Gestora X","feesExpensesInvestment":{"annualReportExpenseRatio":{"raw":0.0003}}}
        }],"error":null}}"#;
        let p = parse(json).unwrap();
        assert_eq!(p.holdings.len(), 2);
        assert_eq!(p.holdings[0].weight, dec!(0.0835));
        assert_eq!(
            p.sectors,
            vec![
                ("technology".into(), dec!(0.33)),
                ("realestate".into(), dec!(0.02))
            ]
        );
        assert_eq!(p.expense_ratio, Some(dec!(0.0003)));
        assert_eq!(p.family.as_deref(), Some("Gestora X"));
    }

    #[test]
    fn interpreta_perfil_de_acao_e_erro() {
        let json = br#"{"quoteSummary":{"result":[{"assetProfile":{"sector":"Technology"}}],"error":null}}"#;
        assert_eq!(parse(json).unwrap().sector.as_deref(), Some("Technology"));
        let err = br#"{"quoteSummary":{"result":null,"error":{"code":"Not Found"}}}"#;
        assert!(matches!(parse(err), Err(ProviderError::NotFound(_))));
        assert_eq!(encode("a/b+c="), "a%2Fb%2Bc%3D");
    }
}
