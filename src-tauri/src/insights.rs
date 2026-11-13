//! Informações do ativo no Home broker: rentabilidade por período (na moeda do
//! ativo, em reais e descontada a inflação), a simulação "se você tivesse
//! investido" comparada com os índices, os proventos pagos e o "Sobre"
//! (composição, setores, taxa e texto em português).
//!
//! Tudo vem de fontes gratuitas, buscado na hora e guardado só na memória
//! durante o dia: Yahoo (fechamento ajustado pelos proventos, dólar,
//! Ibovespa, IVVB11, proventos e perfil), Banco Central (CDI e IPCA) e B3
//! (atividade das empresas). Nada vai para o cofre.
//!
//! Cada número leva a conta com os valores usados (`explanation`).

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use carteira_core::benchmark::{IndexKind, IndexSeries};
use carteira_core::format::{number, pct};
use carteira_providers::b3_empresa::{self, Company};
use carteira_providers::yahoo::{self, Dividends};
use carteira_providers::yahoo_perfil::{self, Profile};
use carteira_providers::{sgs, ProviderError};
use chrono::{Datelike, Duration, Months, NaiveDate};
use rust_decimal::{Decimal, MathematicalOps};
use serde::Serialize;

use crate::asset::{AssetDetail, Venue};

/// Busca os dados na rede. Substituível nos testes.
pub trait InsightsNet {
    fn adjusted(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError>;
    fn closes(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
        dp: u32,
    ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError>;
    fn sgs(
        &self,
        code: u32,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError>;
    fn dividends(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Dividends, ProviderError>;
    fn profile(&self, symbol: &str) -> Result<Profile, ProviderError>;
    fn company(&self, ticker: &str) -> Result<Option<Company>, ProviderError>;
}

pub struct Internet;

impl InsightsNet for Internet {
    fn adjusted(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError> {
        yahoo::fetch_adjusted(symbol, from, to)
    }
    fn closes(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
        dp: u32,
    ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError> {
        yahoo::fetch_closes_dp(symbol, from, to, dp)
    }
    fn sgs(
        &self,
        code: u32,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError> {
        sgs::fetch(code, from, to)
    }
    fn dividends(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Dividends, ProviderError> {
        yahoo::fetch_dividends(symbol, from, to)
    }
    fn profile(&self, symbol: &str) -> Result<Profile, ProviderError> {
        yahoo_perfil::fetch(symbol)
    }
    fn company(&self, ticker: &str) -> Result<Option<Company>, ProviderError> {
        b3_empresa::fetch(ticker)
    }
}

/// Um número com a conta que o produziu.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Figure {
    pub rate: Decimal,
    pub explanation: String,
}

#[derive(Debug, Serialize)]
pub struct ReturnRow {
    pub label: String,
    /// Uma por período de `PERIODS`; `None` quando o ativo é mais novo que o período.
    pub cells: Vec<Option<Figure>>,
}

#[derive(Debug, Serialize)]
pub struct SimItem {
    pub name: String,
    /// O próprio ativo (destacado na tela).
    pub asset: bool,
    pub figure: Option<Figure>,
}

#[derive(Debug, Serialize)]
pub struct SimPeriod {
    pub years: u32,
    pub start: NaiveDate,
    pub items: Vec<SimItem>,
}

#[derive(Debug, Serialize)]
pub struct DividendRow {
    pub date: NaiveDate,
    /// Valor por cota na época, na moeda do ativo.
    pub value: Decimal,
}

#[derive(Debug, Serialize)]
pub struct SectorWeight {
    pub name: String,
    pub weight: Decimal,
}

#[derive(Debug, Serialize)]
pub struct HoldingRow {
    pub symbol: String,
    pub name: String,
    pub weight: Decimal,
}

#[derive(Debug, Default, Serialize)]
pub struct About {
    /// Texto em português e de onde veio.
    pub text: Option<String>,
    pub text_source: Option<String>,
    /// Data do texto: quando foi escrito (textos do app, que não vêm da
    /// internet) ou quando foi buscado (cadastro da B3).
    pub text_date: Option<NaiveDate>,
    /// O texto é do app (escrito à mão), não buscado agora na internet.
    pub text_fixed: bool,
    /// Setor / subsetor / segmento (B3) ou o setor traduzido (Yahoo).
    pub sector: Option<String>,
    pub manager: Option<String>,
    /// Taxa de administração ao ano, em fração.
    pub expense_ratio: Option<Decimal>,
    pub website: Option<String>,
    pub holdings: Vec<HoldingRow>,
    pub sectors: Vec<SectorWeight>,
    /// Fonte da composição, setores e taxa, e o dia em que foram buscados.
    pub data_source: Option<String>,
    pub data_date: Option<NaiveDate>,
    /// Ficha da lista oficial da Bolsa de Londres.
    pub listing: Option<Ficha>,
}

/// Dados oficiais de um ativo de Londres, em português.
#[derive(Debug, Default, Serialize)]
pub struct Ficha {
    pub name: String,
    pub issuer: String,
    pub isin: String,
    /// Tipo explicado: "ETF (fundo de índice)" etc.
    pub kind: String,
    /// Código do tipo: ETF, ETC ou ETN.
    pub kind_code: String,
    /// País de constituição: o da tabela do IR quando o app conhece.
    pub country: String,
    pub start: Option<NaiveDate>,
    /// "Acumula" ou "Distribui", quando o nome diz.
    pub income: Option<String>,
    /// Data dos dados da lista ("As at").
    pub list_date: Option<NaiveDate>,
}

fn ficha(l: &carteira_storage::LseListing) -> Ficha {
    let date = |d: &Option<String>| {
        d.as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    };
    let n = l.name.to_uppercase();
    let has = |w: &str| {
        n.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|p| p == w)
    };
    let income = if has("ACC") || has("ACCUMULATING") {
        Some("Acumula: reinveste os proventos no próprio fundo".into())
    } else if has("DIST") || has("DISTRIBUTING") || has("INC") {
        Some("Distribui: paga os proventos em dinheiro".into())
    } else {
        None
    };
    Ficha {
        name: l.name.clone(),
        issuer: l.issuer.clone(),
        isin: l.isin.clone(),
        kind: match l.kind.as_str() {
            "ETC" => "ETC: título de dívida lastreado em mercadoria (como ouro)",
            "ETN" => "ETN: nota do emissor, que promete pagar o retorno de um índice",
            _ => "ETF: fundo de índice",
        }
        .into(),
        kind_code: l.kind.clone(),
        country: carteira_core::london::pais_ir(&l.country)
            .map(|c| c.split(" - ").nth(1).unwrap_or(c).to_string())
            .unwrap_or_else(|| l.country.clone()),
        start: date(&l.start_date),
        income,
        list_date: date(&l.list_date),
    }
}

#[derive(Debug, Serialize)]
pub struct Insights {
    pub ticker: String,
    pub currency: String,
    pub price: Option<Decimal>,
    pub price_date: Option<NaiveDate>,
    pub periods: Vec<String>,
    pub returns: Vec<ReturnRow>,
    pub simulation: Vec<SimPeriod>,
    pub dividends: Vec<DividendRow>,
    /// Proventos por cota nos últimos 12 meses e a fração da cotação (dividend yield).
    pub dividends_12m: Option<Decimal>,
    pub yield_12m: Option<Decimal>,
    pub about: About,
    /// O que não deu para buscar (cada parte falha sozinha).
    pub warnings: Vec<String>,
}

pub const PERIODS: &[(&str, u32)] = &[
    ("1 mês", 1),
    ("3 meses", 3),
    ("1 ano", 12),
    ("2 anos", 24),
    ("5 anos", 60),
    ("10 anos", 120),
];
pub const SIM_YEARS: &[u32] = &[1, 2, 5, 10];

/// Série de preços (ou pontos) com o valor de cada data.
type Levels = BTreeMap<NaiveDate, Decimal>;

fn on_or_before(s: &Levels, d: NaiveDate) -> Option<(NaiveDate, Decimal)> {
    s.range(..=d).next_back().map(|(k, v)| (*k, *v))
}

fn br(d: NaiveDate) -> String {
    d.format("%d/%m/%Y").to_string()
}

/// Valor perto da data pedida: no máximo 7 dias antes (feriados, fins de
/// semana). Uma série que começa depois da data não cobre o período.
fn near(s: &Levels, d: NaiveDate) -> Option<(NaiveDate, Decimal)> {
    let first = *s.keys().next()?;
    if first > d {
        return None;
    }
    on_or_before(s, d).filter(|(k, v)| *k + Duration::days(7) >= d && !v.is_zero())
}

fn back(end: NaiveDate, months: u32) -> NaiveDate {
    end.checked_sub_months(Months::new(months)).unwrap_or(end)
}

/// Variação do ativo de `start` a `end`, na moeda dele.
fn asset_return(px: &Levels, start: NaiveDate, end: NaiveDate, cur: &str) -> Option<Figure> {
    let (d0, v0) = near(px, start)?;
    let (d1, v1) = near(px, end)?;
    let rate = v1 / v0 - Decimal::ONE;
    let sym = if cur == "USD" { "US$ " } else { "R$ " };
    Some(Figure {
        rate,
        explanation: format!(
            "{sym}{} ({}) ÷ {sym}{} ({}) − 1 = {}. Preços ajustados pelos proventos e desdobramentos (proventos reinvestidos).",
            number(v1, 4),
            br(d1),
            number(v0, 4),
            br(d0),
            pct(rate, 2)
        ),
    })
}

/// Variação em reais de um ativo em dólar: preço × dólar comercial do dia.
fn brl_return(px: &Levels, fx: &Levels, start: NaiveDate, end: NaiveDate) -> Option<Figure> {
    let (d0, v0) = near(px, start)?;
    let (d1, v1) = near(px, end)?;
    let (_, f0) = near(fx, d0)?;
    let (_, f1) = near(fx, d1)?;
    let rate = (v1 * f1) / (v0 * f0) - Decimal::ONE;
    Some(Figure {
        rate,
        explanation: format!(
            "(US$ {} × R$ {}) ({}) ÷ (US$ {} × R$ {}) ({}) − 1 = {}. Preço ajustado pelos proventos vezes o dólar comercial do dia.",
            number(v1, 4),
            number(f1, 4),
            br(d1),
            number(v0, 4),
            number(f0, 4),
            br(d0),
            pct(rate, 2)
        ),
    })
}

/// IPCA acumulado de `start` (exclusive) a `end`, proporcional aos dias do
/// primeiro mês. Os meses ainda não publicados pelo IBGE ficam de fora e a
/// explicação diz até quando vai a conta.
fn ipca(series: &Levels, start: NaiveDate, end: NaiveDate) -> Option<Figure> {
    let last = *series.keys().next_back()?;
    let first = *series.keys().next()?;
    let from = start + Duration::days(1);
    if from.with_day(1)? < first {
        return None;
    }
    let mut factor = Decimal::ONE;
    let mut cursor = from;
    let mut used_until = None;
    while cursor <= end {
        let month = cursor.with_day(1)?;
        let next = month.checked_add_months(Months::new(1))?;
        let dim = (next - month).num_days();
        let to = (next - Duration::days(1)).min(end);
        let covered = (to - cursor).num_days() + 1;
        match series.get(&month) {
            Some(t) => {
                let f = Decimal::ONE + t / Decimal::ONE_HUNDRED;
                factor *= if covered == dim {
                    f
                } else {
                    f.powd(Decimal::from(covered) / Decimal::from(dim))
                };
                used_until = Some(month);
            }
            // Mês depois do último publicado: ainda não saiu.
            None if month > last => break,
            None => return None,
        }
        cursor = next;
    }
    let rate = factor - Decimal::ONE;
    let until = used_until?;
    Some(Figure {
        rate,
        explanation: format!(
            "IPCA de {} a {} = {} (o IPCA sai uma vez por mês; os meses ainda não publicados ficam de fora).",
            from.format("%m/%Y"),
            until.format("%m/%Y"),
            pct(rate, 2)
        ),
    })
}

fn real(nominal: &Figure, inflation: &Figure) -> Figure {
    let rate = (Decimal::ONE + nominal.rate) / (Decimal::ONE + inflation.rate) - Decimal::ONE;
    Figure {
        rate,
        explanation: format!(
            "(1 + {}) ÷ (1 + {}) − 1 = {}. Em reais: {} {}",
            pct(nominal.rate, 2),
            pct(inflation.rate, 2),
            pct(rate, 2),
            nominal.explanation,
            inflation.explanation
        ),
    }
}

fn index(id: &str, name: &str, kind: IndexKind, points: &Levels) -> IndexSeries {
    IndexSeries {
        id: id.into(),
        name: name.into(),
        kind,
        source: String::new(),
        points: points.clone(),
    }
}

/// Séries usadas na conta, já baixadas.
#[derive(Default, Clone)]
pub struct Market {
    pub price: Levels,
    pub fx: Levels,
    pub cdi: Levels,
    pub ipca: Levels,
    pub ibov: Levels,
    pub ivvb: Levels,
}

/// Rentabilidade e simulação a partir das séries (sem rede).
pub fn compute(currency: &str, m: &Market) -> (Vec<ReturnRow>, Vec<SimPeriod>) {
    let Some(end) = m.price.keys().next_back().copied() else {
        return (Vec::new(), Vec::new());
    };
    let usd = currency == "USD";
    let mut rows = vec![ReturnRow {
        label: if usd {
            "Em dólar, com proventos".into()
        } else {
            "Em reais, com proventos".into()
        },
        cells: PERIODS
            .iter()
            .map(|(_, mo)| asset_return(&m.price, back(end, *mo), end, currency))
            .collect(),
    }];
    let brl = |mo: u32| -> Option<Figure> {
        let start = back(end, mo);
        if usd {
            brl_return(&m.price, &m.fx, start, end)
        } else {
            asset_return(&m.price, start, end, currency)
        }
    };
    if usd {
        rows.push(ReturnRow {
            label: "Em reais (com o dólar)".into(),
            cells: PERIODS.iter().map(|(_, mo)| brl(*mo)).collect(),
        });
    }
    rows.push(ReturnRow {
        label: "Em reais, descontada a inflação (IPCA)".into(),
        cells: PERIODS
            .iter()
            .map(|(_, mo)| {
                let n = brl(*mo)?;
                let i = ipca(&m.ipca, back(end, *mo), end)?;
                Some(real(&n, &i))
            })
            .collect(),
    });

    let cdi = index("CDI", "CDI", IndexKind::DailyRate, &m.cdi);
    let ibov = index("IBOV", "Ibovespa", IndexKind::Level, &m.ibov);
    let ivvb = index(
        "IVVB11",
        "S&P 500 em reais (IVVB11)",
        IndexKind::Level,
        &m.ivvb,
    );
    let dolar = index("USD", "Dólar comercial", IndexKind::Level, &m.fx);
    let from_index = |s: &IndexSeries, start: NaiveDate| -> Option<Figure> {
        let r = s.period_return(start + Duration::days(1), end)?;
        Some(Figure {
            rate: r.rate,
            explanation: r.explanation,
        })
    };
    let sims = SIM_YEARS
        .iter()
        .map(|y| {
            let start = back(end, y * 12);
            SimPeriod {
                years: *y,
                start,
                items: vec![
                    SimItem {
                        name: String::new(),
                        asset: true,
                        figure: brl(y * 12),
                    },
                    SimItem {
                        name: "CDI".into(),
                        asset: false,
                        figure: from_index(&cdi, start),
                    },
                    SimItem {
                        name: "Ibovespa".into(),
                        asset: false,
                        figure: from_index(&ibov, start),
                    },
                    SimItem {
                        name: "S&P 500 (IVVB11)".into(),
                        asset: false,
                        figure: from_index(&ivvb, start),
                    },
                    SimItem {
                        name: "Dólar".into(),
                        asset: false,
                        figure: from_index(&dolar, start),
                    },
                    SimItem {
                        name: "IPCA".into(),
                        asset: false,
                        figure: ipca(&m.ipca, start, end),
                    },
                ],
            }
        })
        .collect();
    (rows, sims)
}

/// Proventos dos últimos 5 anos (mais recente primeiro) e o total de 12 meses.
pub fn dividend_rows(
    d: &Dividends,
    price: Option<Decimal>,
    end: NaiveDate,
) -> (Vec<DividendRow>, Option<Decimal>, Option<Decimal>) {
    let mut rows: Vec<DividendRow> = d
        .unadjusted()
        .into_iter()
        .map(|(date, value)| DividendRow { date, value })
        .collect();
    rows.sort_by(|a, b| b.date.cmp(&a.date));
    // O dividend yield usa o valor ajustado aos desdobramentos, como a cotação de hoje.
    let year_ago = back(end, 12);
    let sum: Decimal = d
        .dividends
        .iter()
        .filter(|(date, _)| *date > year_ago && *date <= end)
        .map(|(_, v)| *v)
        .sum();
    let total = (!sum.is_zero()).then_some(sum);
    let dy = total
        .zip(price)
        .and_then(|(s, p)| (!p.is_zero()).then(|| s / p));
    (rows, total, dy)
}

fn about(
    detail: &AssetDetail,
    profile: Option<Profile>,
    company: Option<Company>,
    today: NaiveDate,
) -> About {
    let mut a = About {
        listing: detail.listing.as_ref().map(ficha),
        ..About::default()
    };
    if let Some(s) = carteira_core::etf_textos::sobre(&detail.ticker) {
        a.text = Some(s.texto.into());
        a.text_source = Some("Texto do 4Nier, a partir do documento da gestora".into());
        a.text_date = NaiveDate::parse_from_str(s.revisado, "%Y-%m-%d").ok();
        a.text_fixed = true;
    }
    if let Some(c) = company {
        if a.text.is_none() {
            a.text_date = c.activity.as_ref().map(|_| today);
            a.text = c.activity.clone();
            a.text_source = c.activity.as_ref().map(|_| {
                format!(
                    "Atividade principal de {} no cadastro da B3",
                    c.trading_name
                )
            });
        }
        a.sector = c.classification;
        a.website = c.website;
    }
    if let Some(p) = profile {
        let translated: Vec<SectorWeight> = p
            .sectors
            .iter()
            .filter_map(|(k, w)| {
                Some(SectorWeight {
                    name: carteira_core::etf_textos::setor(k)?.to_string(),
                    weight: *w,
                })
            })
            .collect();
        let has_data =
            !p.holdings.is_empty() || !translated.is_empty() || p.expense_ratio.is_some();
        a.holdings = p
            .holdings
            .into_iter()
            .take(10)
            .map(|h| HoldingRow {
                name: if h.name.is_empty() {
                    h.symbol.clone()
                } else {
                    h.name
                },
                symbol: h.symbol,
                weight: h.weight,
            })
            .collect();
        a.sectors = translated;
        a.expense_ratio = p.expense_ratio;
        a.manager = p.family;
        if a.sector.is_none() {
            a.sector = p
                .sector
                .as_deref()
                .and_then(carteira_core::etf_textos::setor)
                .map(String::from);
        }
        if has_data || a.manager.is_some() || a.sector.is_some() {
            a.data_source = Some(yahoo_perfil::SOURCE.into());
            a.data_date = Some(today);
        }
    }
    a
}

type Cache<T> = Mutex<HashMap<String, (NaiveDate, T)>>;
static SERIES: std::sync::LazyLock<Cache<Levels>> = std::sync::LazyLock::new(Default::default);
static RESULTS: std::sync::LazyLock<Cache<std::sync::Arc<String>>> =
    std::sync::LazyLock::new(Default::default);

/// Série compartilhada (CDI, IPCA, índices, dólar), baixada uma vez por dia.
fn shared(
    key: &str,
    today: NaiveDate,
    fetch: impl FnOnce() -> Result<Vec<(NaiveDate, Decimal)>, ProviderError>,
) -> Result<Levels, ProviderError> {
    if let Ok(c) = SERIES.lock() {
        if let Some((d, s)) = c.get(key) {
            if *d == today {
                return Ok(s.clone());
            }
        }
    }
    let s: Levels = fetch()?.into_iter().collect();
    if let Ok(mut c) = SERIES.lock() {
        c.insert(key.into(), (today, s.clone()));
    }
    Ok(s)
}

/// Resultado já calculado hoje (em JSON), para abrir o mesmo ativo de novo sem rede.
pub fn cached(ticker: &str, today: NaiveDate) -> Option<std::sync::Arc<String>> {
    let c = RESULTS.lock().ok()?;
    c.get(ticker)
        .filter(|(d, _)| *d == today)
        .map(|(_, v)| v.clone())
}

pub fn remember(ticker: &str, today: NaiveDate, json: String) {
    if let Ok(mut c) = RESULTS.lock() {
        c.insert(ticker.into(), (today, std::sync::Arc::new(json)));
    }
}

pub fn build(detail: &AssetDetail, net: &dyn InsightsNet, today: NaiveDate) -> Insights {
    let mut warnings = Vec::new();
    let mut out = Insights {
        ticker: detail.ticker.clone(),
        currency: detail.currency.clone(),
        price: None,
        price_date: None,
        periods: PERIODS.iter().map(|(l, _)| l.to_string()).collect(),
        returns: Vec::new(),
        simulation: Vec::new(),
        dividends: Vec::new(),
        dividends_12m: None,
        yield_12m: None,
        about: About::default(),
        warnings: Vec::new(),
    };
    // Fundos e Tesouro têm telas próprias, com os dados do cofre.
    if matches!(detail.venue, Venue::Fundo | Venue::Tesouro) {
        return out;
    }
    let symbol = crate::asset::yahoo_symbol(detail);
    let from = back(today, 121) - Duration::days(15);
    let mut fail = |what: &str, e: &ProviderError| warnings.push(format!("{what}: {e}"));

    let mut m = Market::default();
    match net.adjusted(&symbol, from, today) {
        Ok(v) => m.price = v.into_iter().collect(),
        Err(e) => fail("Cotações do ativo (Yahoo)", &e),
    }
    let mut get = |key: &str,
                   what: &str,
                   f: &dyn Fn() -> Result<Vec<(NaiveDate, Decimal)>, ProviderError>|
     -> Levels {
        match shared(key, today, f) {
            Ok(s) => s,
            Err(e) => {
                fail(what, &e);
                Levels::new()
            }
        }
    };
    m.fx = get("USD", "Dólar comercial (Yahoo)", &|| {
        net.closes(yahoo::USD_BRL_SYMBOL, from, today, 4)
    });
    m.cdi = get("CDI", "CDI (Banco Central)", &|| {
        net.sgs(sgs::CDI, from, today)
    });
    m.ipca = get("IPCA", "IPCA (Banco Central)", &|| {
        net.sgs(sgs::IPCA, from.with_day(1).unwrap_or(from), today)
    });
    m.ibov = get("IBOV", "Ibovespa (Yahoo)", &|| {
        net.closes("^BVSP", from, today, 2)
    });
    m.ivvb = get("IVVB11", "IVVB11 (Yahoo)", &|| {
        net.closes("IVVB11.SA", from, today, 2)
    });

    if let Some((d, p)) = m.price.iter().next_back() {
        out.price = Some(*p);
        out.price_date = Some(*d);
    }
    let (rows, sims) = compute(&detail.currency, &m);
    out.returns = rows;
    out.simulation = sims;

    if detail.venue != Venue::Cripto {
        match net.dividends(&symbol, back(today, 60), today) {
            Ok(d) => {
                let (rows, total, dy) =
                    dividend_rows(&d, out.price, out.price_date.unwrap_or(today));
                out.dividends = rows;
                out.dividends_12m = total;
                out.yield_12m = dy;
            }
            Err(e) => fail("Proventos (Yahoo)", &e),
        }
    }

    let profile = match detail.venue {
        Venue::Us | Venue::Lse => match net.profile(&symbol) {
            Ok(p) => Some(p),
            Err(e) => {
                fail("Composição e taxa (Yahoo)", &e);
                None
            }
        },
        _ => None,
    };
    let company = if detail.venue == Venue::B3
        && carteira_core::etf_textos::sobre(&detail.ticker).is_none()
    {
        match net.company(&detail.ticker) {
            Ok(c) => c,
            Err(e) => {
                fail("Atividade da empresa (B3)", &e);
                None
            }
        }
    } else {
        None
    };
    out.about = about(detail, profile, company, today);
    out.warnings = warnings;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn ficha_da_lista_de_londres() {
        let l = carteira_storage::LseListing {
            ticker: "XYZ.L".into(),
            issuer: "EMISSORA TESTE".into(),
            name: "FUNDO TESTE WORLD UCITS ETF USD (ACC)".into(),
            isin: "LU0000000001".into(),
            country: "Luxembourg".into(),
            kind: "ETF".into(),
            start_date: Some("2020-01-02".into()),
            list_date: Some("2026-10-31".into()),
        };
        let f = ficha(&l);
        assert_eq!(f.country, "Luxemburgo");
        assert!(f.income.unwrap().starts_with("Acumula"));
        assert_eq!(f.list_date, Some(d(2026, 10, 31)));
        let n = ficha(&carteira_storage::LseListing {
            kind: "ETN".into(),
            name: "NOTA TESTE 3X".into(),
            country: "Atlantis".into(),
            ..l
        });
        assert!(n.kind.starts_with("ETN"));
        assert_eq!(n.income, None);
        assert_eq!(n.country, "Atlantis");
    }

    #[test]
    fn rentabilidade_em_dolar_reais_e_real() {
        // Dados sintéticos: o ativo vai de 100 a 110 em um ano; o dólar de 5 a 5,5.
        let mut m = Market::default();
        m.price.insert(d(2025, 11, 12), dec!(100));
        m.price.insert(d(2026, 10, 12), dec!(108));
        m.price.insert(d(2026, 11, 12), dec!(110));
        m.fx.insert(d(2025, 11, 10), dec!(5));
        m.fx.insert(d(2026, 11, 12), dec!(5.5));
        // IPCA de 1% em cada mês de 11/2025 a 09/2026 (outubro ainda não saiu).
        let mut month = d(2025, 11, 1);
        while month <= d(2026, 9, 1) {
            m.ipca.insert(month, dec!(1));
            month = month.checked_add_months(Months::new(1)).unwrap();
        }
        let (rows, sims) = compute("USD", &m);
        assert_eq!(rows.len(), 3);
        let year = 2; // "1 ano"
        assert_eq!(rows[0].cells[year].as_ref().unwrap().rate, dec!(0.1));
        // 110 × 5,5 ÷ (100 × 5) − 1 = 21%
        assert_eq!(rows[1].cells[year].as_ref().unwrap().rate, dec!(0.21));
        // O ativo é mais novo que 2 anos: sem número.
        assert!(rows[0].cells[3].is_none());
        // 1 mês: 110 ÷ 108 − 1
        assert!(rows[0].cells[0]
            .as_ref()
            .unwrap()
            .explanation
            .contains("12/10/2026"));
        let r = rows[2].cells[year].as_ref().unwrap();
        assert!(r.rate > dec!(0.07) && r.rate < dec!(0.10), "{}", r.rate);
        assert!(r.explanation.contains("09/2026"));
        // Simulação de 1 ano: o ativo em reais e o dólar.
        let one = &sims[0];
        assert_eq!(one.items[0].figure.as_ref().unwrap().rate, dec!(0.21));
        assert_eq!(one.items[4].name, "Dólar");
        assert_eq!(one.items[4].figure.as_ref().unwrap().rate, dec!(0.1));
        assert!(one.items[1].figure.is_none()); // sem CDI baixado
    }

    #[test]
    fn ativo_em_reais_tem_duas_linhas_e_proventos() {
        let mut m = Market::default();
        m.price.insert(d(2026, 8, 12), dec!(10));
        m.price.insert(d(2026, 11, 12), dec!(12));
        let (rows, _) = compute("BRL", &m);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].cells[1].as_ref().unwrap().rate, dec!(0.2));
        let div = Dividends {
            dividends: vec![
                (d(2025, 10, 1), dec!(0.5)),
                (d(2026, 3, 1), dec!(0.3)),
                (d(2026, 9, 1), dec!(0.2)),
            ],
            splits: vec![],
        };
        let (rows, total, dy) = dividend_rows(&div, Some(dec!(10)), d(2026, 11, 12));
        assert_eq!(rows[0].date, d(2026, 9, 1));
        assert_eq!(total, Some(dec!(0.5)));
        assert_eq!(dy, Some(dec!(0.05)));
    }
}
