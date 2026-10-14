//! Atualização sob demanda do catálogo e das cotações.
//!
//! Ordem: (1) catálogo da B3 pelo último pregão; (2) fechamentos oficiais dos
//! meses já encerrados pelo arquivo mensal COTAHIST, uma vez por mês;
//! (3) mês corrente e último preço pelo Yahoo. Uma fonte que falha não impede
//! as outras; sem internet, os cálculos usam as últimas cotações gravadas.
//!
//! A rede é acessada sem segurar o cofre: primeiro se planeja o que falta,
//! depois se baixa, e só então se grava.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use carteira_core::operations::Operation;
use carteira_providers::{cotahist, yahoo, ProviderError};
use carteira_storage::{Asset, NewQuote, Vault};
use chrono::{Datelike, Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::Serialize;

pub const CATALOG_SOURCE: &str = "B3-CATALOGO";
const CATALOG_MAX_AGE_DAYS: i64 = 7;

/// Acesso à rede, substituível nos testes.
pub trait Fetcher {
    fn latest_daily(&self, today: NaiveDate) -> Result<(NaiveDate, Vec<cotahist::Record>), ProviderError>;
    fn monthly(&self, year: i32, month: u32) -> Result<Vec<cotahist::Record>, ProviderError>;
    fn yahoo(&self, symbol: &str, from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError>;
}

pub struct Internet;

impl Fetcher for Internet {
    fn latest_daily(&self, today: NaiveDate) -> Result<(NaiveDate, Vec<cotahist::Record>), ProviderError> {
        cotahist::fetch_latest_daily(today, 10)
    }
    fn monthly(&self, year: i32, month: u32) -> Result<Vec<cotahist::Record>, ProviderError> {
        cotahist::fetch(&cotahist::monthly_url(year, month))
    }
    fn yahoo(&self, symbol: &str, from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError> {
        yahoo::fetch_closes(symbol, from, to)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateLine {
    pub source: String,
    pub ok: bool,
    pub message: String,
}

type Shared<'a> = &'a Mutex<Option<Vault>>;

fn with<T>(vault: Shared, f: impl FnOnce(&mut Vault) -> Result<T, String>) -> Result<T, String> {
    let mut guard = vault.lock().map_err(|_| "estado interno indisponível".to_string())?;
    let v = guard.as_mut().ok_or_else(|| "o cofre está bloqueado".to_string())?;
    f(v)
}

fn month_key(year: i32, month: u32) -> String {
    format!("{year}-{month:02}")
}

fn month_end(year: i32, month: u32) -> NaiveDate {
    let (y, m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(y, m, 1).expect("data válida") - Duration::days(1)
}

/// Ativos com posição em algum momento de cada mês, desde a primeira operação.
fn tickers_by_month(ops: &[Operation], today: NaiveDate) -> BTreeMap<(i32, u32), BTreeSet<String>> {
    let mut by_asset: BTreeMap<&str, Vec<&Operation>> = BTreeMap::new();
    for op in ops {
        by_asset.entry(op.asset.as_str()).or_default().push(op);
    }
    let mut out: BTreeMap<(i32, u32), BTreeSet<String>> = BTreeMap::new();
    for (asset, mut list) in by_asset {
        list.sort_by_key(|o| o.date);
        let first = list[0].date;
        let mut cursor = NaiveDate::from_ymd_opt(first.year(), first.month(), 1).expect("dia 1");
        while cursor <= today {
            let end = month_end(cursor.year(), cursor.month());
            let start_qty: Decimal = list.iter().filter(|o| o.date < cursor).map(|o| signed(o)).sum();
            let traded = list.iter().any(|o| o.date >= cursor && o.date <= end);
            if traded || !start_qty.is_zero() {
                out.entry((cursor.year(), cursor.month())).or_default().insert(asset.to_string());
            }
            cursor = end + Duration::days(1);
        }
    }
    out
}

fn signed(o: &Operation) -> Decimal {
    match o.side {
        carteira_core::model::Side::Compra => o.quantity,
        carteira_core::model::Side::Venda => -o.quantity,
    }
}

fn closes_for(records: &[cotahist::Record], tickers: &BTreeSet<String>) -> Vec<NewQuote> {
    records
        .iter()
        .filter(|r| r.is_spot() && tickers.contains(&r.ticker))
        .map(|r| NewQuote { ticker: r.ticker.clone(), date: r.date, close: r.close, source: cotahist::SOURCE.into() })
        .collect()
}

fn describe(e: &ProviderError) -> String {
    match e {
        ProviderError::Network(_) => format!("sem acesso à fonte ({e})"),
        _ => e.to_string(),
    }
}

pub fn update_all(vault: Shared, fetcher: &dyn Fetcher, today: NaiveDate) -> Result<Vec<UpdateLine>, String> {
    let mut report = Vec::new();

    // 1. Catálogo.
    let (catalog_age, ops) = with(vault, |v| {
        let last = v.last_successful_update(CATALOG_SOURCE, None).map_err(|e| e.to_string())?;
        let count = v.asset_count().map_err(|e| e.to_string())?;
        let ops = v.list_operations().map_err(|e| e.to_string())?;
        let stale = count < 100
            || last
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| (today - d.date_naive()).num_days() >= CATALOG_MAX_AGE_DAYS)
                .unwrap_or(true);
        Ok((stale, ops))
    })?;
    let held = tickers_by_month(&ops, today);
    let all_tickers: BTreeSet<String> = held.values().flatten().cloned().collect();

    if catalog_age {
        match fetcher.latest_daily(today) {
            Ok((day, records)) => {
                let assets: Vec<Asset> = records
                    .iter()
                    .filter(|r| r.is_spot())
                    .map(|r| Asset {
                        ticker: r.ticker.clone(),
                        name: r.short_name.clone(),
                        kind: r.kind().into(),
                        market: "B3".into(),
                        currency: "BRL".into(),
                        isin: Some(r.isin.clone()).filter(|s| !s.is_empty()),
                        source: cotahist::SOURCE.into(),
                    })
                    .collect();
                let quotes = closes_for(&records, &all_tickers);
                let msg = format!("{} ativos do pregão de {}", assets.len(), day.format("%d/%m/%Y"));
                with(vault, |v| {
                    v.upsert_assets(&assets).map_err(|e| e.to_string())?;
                    v.save_quotes(&quotes).map_err(|e| e.to_string())?;
                    v.record_quote_update(CATALOG_SOURCE, Some(&day.to_string()), true, assets.len() as i64, &msg)
                        .map_err(|e| e.to_string())
                })?;
                report.push(UpdateLine { source: "Catálogo B3".into(), ok: true, message: msg });
            }
            Err(e) => {
                let msg = describe(&e);
                with(vault, |v| v.record_quote_update(CATALOG_SOURCE, None, false, 0, &msg).map_err(|e| e.to_string()))?;
                report.push(UpdateLine { source: "Catálogo B3".into(), ok: false, message: msg });
            }
        }
    }

    // 2. Meses encerrados: arquivo mensal oficial, uma vez por mês.
    let current = (today.year(), today.month());
    let mut missing_since: Option<NaiveDate> = None;
    let mut months_ok = 0;
    let mut month_errors = Vec::new();
    for (&(y, m), tickers) in held.iter().filter(|(k, _)| **k < current) {
        let key = month_key(y, m);
        let done = with(vault, |v| v.last_successful_update(cotahist::SOURCE, Some(&key)).map_err(|e| e.to_string()))?;
        if done.is_some() {
            continue;
        }
        match fetcher.monthly(y, m) {
            Ok(records) => {
                let quotes = closes_for(&records, tickers);
                let msg = format!("{} fechamentos de {m:02}/{y}", quotes.len());
                with(vault, |v| {
                    v.save_quotes(&quotes).map_err(|e| e.to_string())?;
                    v.record_quote_update(cotahist::SOURCE, Some(&key), true, quotes.len() as i64, &msg).map_err(|e| e.to_string())
                })?;
                months_ok += 1;
            }
            Err(e) => {
                let msg = format!("{m:02}/{y}: {}", describe(&e));
                with(vault, |v| v.record_quote_update(cotahist::SOURCE, Some(&key), false, 0, &msg).map_err(|e| e.to_string()))?;
                missing_since.get_or_insert(NaiveDate::from_ymd_opt(y, m, 1).expect("dia 1"));
                month_errors.push(msg);
                // Sem acesso à B3 não adianta tentar os outros meses agora.
                if matches!(e, ProviderError::Network(_)) {
                    break;
                }
            }
        }
    }
    if months_ok > 0 || !month_errors.is_empty() {
        let mut msg = format!("{months_ok} mês(es) baixado(s)");
        if !month_errors.is_empty() {
            msg.push_str(&format!("; falhas: {}", month_errors.join("; ")));
        }
        report.push(UpdateLine { source: "Fechamentos oficiais B3".into(), ok: month_errors.is_empty(), message: msg });
    }

    // 3. Mês corrente e último preço (e meses que a B3 não entregou).
    let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).expect("dia 1");
    let from = missing_since.unwrap_or(month_start).min(month_start) - Duration::days(10);
    let current_tickers = held.get(&current).cloned().unwrap_or_default();
    let targets: BTreeSet<String> = if missing_since.is_some() { all_tickers.clone() } else { current_tickers };
    let mut saved = 0;
    let mut failures = Vec::new();
    for t in &targets {
        match fetcher.yahoo(&yahoo::b3_symbol(t), from, today) {
            Ok(closes) => {
                let quotes: Vec<NewQuote> = closes
                    .into_iter()
                    .map(|(date, close)| NewQuote { ticker: t.clone(), date, close, source: yahoo::SOURCE.into() })
                    .collect();
                saved += quotes.len();
                with(vault, |v| v.save_quotes(&quotes).map(|_| ()).map_err(|e| e.to_string()))?;
            }
            Err(e) => failures.push(format!("{t}: {}", describe(&e))),
        }
    }
    if !targets.is_empty() {
        let mut msg = format!("{saved} cotações de {} ativo(s)", targets.len() - failures.len());
        if !failures.is_empty() {
            msg.push_str(&format!("; falhas: {}", failures.join("; ")));
        }
        let ok = failures.is_empty();
        with(vault, |v| v.record_quote_update(yahoo::SOURCE, None, ok, saved as i64, &msg).map_err(|e| e.to_string()))?;
        report.push(UpdateLine { source: "Últimos preços (Yahoo)".into(), ok, message: msg });
    }
    if report.is_empty() {
        report.push(UpdateLine { source: "Cotações".into(), ok: true, message: "nada a atualizar".into() });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use carteira_core::model::Side;
    use carteira_core::returns::PriceSource;
    use carteira_storage::KdfParams;
    use std::cell::RefCell;

    struct Fake {
        b3_up: bool,
        monthly_calls: RefCell<Vec<(i32, u32)>>,
    }

    fn rec(ticker: &str, date: NaiveDate, close: Decimal) -> cotahist::Record {
        cotahist::Record {
            date,
            bdi: "02".into(),
            ticker: ticker.into(),
            market: "010".into(),
            short_name: "TESTE".into(),
            specification: "ON".into(),
            close,
            isin: String::new(),
        }
    }

    impl Fetcher for Fake {
        fn latest_daily(&self, today: NaiveDate) -> Result<(NaiveDate, Vec<cotahist::Record>), ProviderError> {
            if !self.b3_up {
                return Err(ProviderError::Network("bloqueado".into()));
            }
            Ok((today, (0..150).map(|i| rec(&format!("T{i:03}3"), today, Decimal::ONE)).collect()))
        }
        fn monthly(&self, year: i32, month: u32) -> Result<Vec<cotahist::Record>, ProviderError> {
            self.monthly_calls.borrow_mut().push((year, month));
            if !self.b3_up {
                return Err(ProviderError::Network("bloqueado".into()));
            }
            Ok(vec![rec("TEST3", month_end(year, month), Decimal::from(month)), rec("OUTRO3", month_end(year, month), Decimal::ONE)])
        }
        fn yahoo(&self, _symbol: &str, _from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError> {
            Ok(vec![(to, Decimal::from(99))])
        }
    }

    fn setup(dir: &std::path::Path) -> Mutex<Option<Vault>> {
        let mut v = Vault::create_with_params(dir, "senha-de-teste-123", "0.2.0", KdfParams::fast_for_tests()).unwrap();
        v.create_operation(Operation {
            id: String::new(),
            account: "Corretora X".into(),
            asset: "TEST3".into(),
            date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            side: Side::Compra,
            quantity: Decimal::from(10),
            price: Decimal::from(10),
            manual_fees: None,
            note: None,
        })
        .unwrap();
        Mutex::new(Some(v))
    }

    #[test]
    fn baixa_meses_encerrados_uma_vez_e_o_mes_corrente() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = setup(tmp.path());
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fake = Fake { b3_up: true, monthly_calls: RefCell::new(vec![]) };
        let report = update_all(&vault, &fake, today).unwrap();
        assert!(report.iter().all(|l| l.ok), "{report:?}");
        assert_eq!(*fake.monthly_calls.borrow(), vec![(2026, 1), (2026, 2)]);

        update_all(&vault, &fake, today).unwrap();
        assert_eq!(fake.monthly_calls.borrow().len(), 2, "mês já baixado não é baixado de novo");

        let g = vault.lock().unwrap();
        let v = g.as_ref().unwrap();
        let table = v.price_table(&["TEST3".into(), "OUTRO3".into()]).unwrap();
        assert_eq!(table.close_on_or_before("TEST3", NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()).unwrap().close, Decimal::from(2));
        assert_eq!(table.close_on_or_before("TEST3", today).unwrap().close, Decimal::from(99));
        assert!(table.latest("OUTRO3").is_none(), "só grava ativos da carteira");
        assert!(v.asset_count().unwrap() >= 150);
    }

    #[test]
    fn sem_b3_usa_yahoo_desde_o_primeiro_mes() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = setup(tmp.path());
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fake = Fake { b3_up: false, monthly_calls: RefCell::new(vec![]) };
        let report = update_all(&vault, &fake, today).unwrap();
        assert_eq!(fake.monthly_calls.borrow().len(), 1, "para no primeiro erro de rede");
        assert!(!report[0].ok);
        assert!(report.iter().any(|l| l.source.contains("Yahoo") && l.ok));
    }
}
