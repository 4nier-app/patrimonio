//! Atualização sob demanda do catálogo e das cotações.
//!
//! Ordem: (1) catálogos da B3 (último pregão) e dos EUA (Nasdaq Trader);
//! (2) fechamentos oficiais dos meses já encerrados pelo arquivo mensal
//! COTAHIST, uma vez por mês; (3) mês corrente e último preço pelo Yahoo, e
//! todo o histórico dos ativos dos EUA, que não têm arquivo oficial;
//! (4) PTAX do Banco Central desde a primeira operação em dólar. Uma fonte que
//! falha não impede as outras; sem internet, os cálculos usam o que já está
//! gravado.
//!
//! A rede é acessada sem segurar o cofre: primeiro se planeja o que falta,
//! depois se baixa, e só então se grava.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use carteira_core::crypto;
use carteira_core::model::{FxSource, Side, BRL};
use carteira_core::operations::Operation;
use carteira_providers::{cotahist, nasdaq, ptax, sgs, yahoo, ProviderError};
use carteira_storage::{Asset, NewFxRate, NewQuote, Vault};
use chrono::{Datelike, Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::Serialize;

pub const CATALOG_SOURCE: &str = "B3-CATALOGO";
pub const US_CATALOG_SOURCE: &str = "EUA-CATALOGO";
const CATALOG_MAX_AGE_DAYS: i64 = 7;

/// Acesso à rede, substituível nos testes.
pub trait Fetcher {
    fn latest_daily(
        &self,
        today: NaiveDate,
    ) -> Result<(NaiveDate, Vec<cotahist::Record>), ProviderError>;
    fn monthly(&self, year: i32, month: u32) -> Result<Vec<cotahist::Record>, ProviderError>;
    fn yahoo(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError>;
    fn us_catalog(&self) -> Result<Vec<nasdaq::Listing>, ProviderError>;
    fn ptax(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<ptax::Ptax>, ProviderError>;
    fn sgs(
        &self,
        code: u32,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError>;
}

/// De onde vem cada índice de comparação.
#[derive(Clone, Copy)]
pub enum IndexOrigin {
    Sgs(u32),
    Yahoo(&'static str),
}

/// Índice de comparação: código no cofre, nome, tipo e origem.
pub struct IndexDef {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: carteira_core::benchmark::IndexKind,
    pub origin: IndexOrigin,
}

pub const INDEXES: &[IndexDef] = &[
    IndexDef {
        id: "CDI",
        name: "CDI",
        kind: carteira_core::benchmark::IndexKind::DailyRate,
        origin: IndexOrigin::Sgs(sgs::CDI),
    },
    IndexDef {
        id: "IPCA",
        name: "IPCA",
        kind: carteira_core::benchmark::IndexKind::MonthlyRate,
        origin: IndexOrigin::Sgs(sgs::IPCA),
    },
    IndexDef {
        id: "IBOV",
        name: "Ibovespa",
        kind: carteira_core::benchmark::IndexKind::Level,
        origin: IndexOrigin::Yahoo("^BVSP"),
    },
    IndexDef {
        id: "IVVB11",
        name: "IVVB11 (S&P 500 em reais)",
        kind: carteira_core::benchmark::IndexKind::Level,
        origin: IndexOrigin::Yahoo("IVVB11.SA"),
    },
];

impl IndexOrigin {
    pub fn source(&self) -> &'static str {
        match self {
            IndexOrigin::Sgs(_) => sgs::SOURCE,
            IndexOrigin::Yahoo(_) => yahoo::SOURCE,
        }
    }
}

pub struct Internet;

impl Fetcher for Internet {
    fn latest_daily(
        &self,
        today: NaiveDate,
    ) -> Result<(NaiveDate, Vec<cotahist::Record>), ProviderError> {
        cotahist::fetch_latest_daily(today, 10)
    }
    fn monthly(&self, year: i32, month: u32) -> Result<Vec<cotahist::Record>, ProviderError> {
        cotahist::fetch(&cotahist::monthly_url(year, month))
    }
    fn yahoo(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError> {
        yahoo::fetch_closes(symbol, from, to)
    }
    fn us_catalog(&self) -> Result<Vec<nasdaq::Listing>, ProviderError> {
        nasdaq::fetch_all()
    }
    fn ptax(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<ptax::Ptax>, ProviderError> {
        ptax::fetch(from, to)
    }
    fn sgs(
        &self,
        code: u32,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError> {
        sgs::fetch(code, from, to)
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
    let mut guard = vault
        .lock()
        .map_err(|_| "estado interno indisponível".to_string())?;
    let v = guard
        .as_mut()
        .ok_or_else(|| "o cofre está bloqueado".to_string())?;
    f(v)
}

/// Data e hora ISO (UTC) no fuso do computador: "14/10/2026 às 18:37".
fn local_stamp(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%d/%m/%Y às %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| iso.to_string())
}

fn month_key(year: i32, month: u32) -> String {
    format!("{year}-{month:02}")
}

fn month_end(year: i32, month: u32) -> NaiveDate {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
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
            let start_qty: Decimal = list
                .iter()
                .filter(|o| o.date < cursor)
                .map(|o| signed(o))
                .sum();
            let traded = list.iter().any(|o| o.date >= cursor && o.date <= end);
            if traded || !carteira_core::model::is_dust(start_qty) {
                out.entry((cursor.year(), cursor.month()))
                    .or_default()
                    .insert(asset.to_string());
            }
            cursor = end + Duration::days(1);
        }
    }
    out
}

/// Efeito na quantidade total da carteira. A transferência só tira o resíduo.
fn signed(o: &Operation) -> Decimal {
    match o.side {
        Side::Compra => o.quantity,
        Side::Venda => -o.quantity,
        Side::Transferencia => -o.residue.unwrap_or_default(),
    }
}

fn closes_for(records: &[cotahist::Record], tickers: &BTreeSet<String>) -> Vec<NewQuote> {
    records
        .iter()
        .filter(|r| r.is_spot() && tickers.contains(&r.ticker))
        .map(|r| NewQuote {
            ticker: r.ticker.clone(),
            date: r.date,
            close: r.close,
            source: cotahist::SOURCE.into(),
        })
        .collect()
}

fn describe(e: &ProviderError) -> String {
    match e {
        ProviderError::Network(_) => format!("sem acesso à fonte ({e})"),
        _ => e.to_string(),
    }
}

pub fn update_all(
    vault: Shared,
    fetcher: &dyn Fetcher,
    today: NaiveDate,
) -> Result<Vec<UpdateLine>, String> {
    let mut report = Vec::new();
    with(vault, |v| {
        v.seed_crypto().map(|_| ()).map_err(|e| e.to_string())
    })?;

    // 1. Catálogo.
    let catalog_state = |source: &str, market: &str| {
        with(vault, |v| {
            let last = v
                .last_successful_update(source, None)
                .map_err(|e| e.to_string())?;
            let count = v.asset_count_in(market).map_err(|e| e.to_string())?;
            let last_date = last
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.date_naive());
            let stale = count < 100
                || last_date
                    .map(|d| (today - d).num_days() >= CATALOG_MAX_AGE_DAYS)
                    .unwrap_or(true);
            let info = last.as_deref().map(|iso| {
                let next = last_date
                    .map(|d| {
                        (d + Duration::days(CATALOG_MAX_AGE_DAYS))
                            .format("%d/%m/%Y")
                            .to_string()
                    })
                    .unwrap_or_default();
                format!(
                    "em dia: {count} ativos, baixado em {}; próxima atualização em {next}",
                    local_stamp(iso)
                )
            });
            Ok((stale, info))
        })
    };
    let (catalog_age, catalog_info) = catalog_state(CATALOG_SOURCE, "B3")?;
    let ops = with(vault, |v| v.list_operations().map_err(|e| e.to_string()))?;
    let ops_dates: Vec<NaiveDate> = ops.iter().map(|o| o.date).collect();
    let (crypto_ops, ops): (Vec<Operation>, Vec<Operation>) =
        ops.into_iter().partition(|o| crypto::is_crypto(&o.asset));
    let (b3_ops, us_ops): (Vec<Operation>, Vec<Operation>) =
        ops.into_iter().partition(|o| o.currency == BRL);
    let held = tickers_by_month(&b3_ops, today);
    let all_tickers: BTreeSet<String> = held.values().flatten().cloned().collect();

    if !catalog_age {
        report.push(UpdateLine {
            source: "Catálogo B3 (lista de ativos)".into(),
            ok: true,
            message: catalog_info.unwrap_or_default(),
        });
    } else {
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
                let msg = format!(
                    "{} ativos do pregão de {}, atualizado em {}",
                    assets.len(),
                    day.format("%d/%m/%Y"),
                    chrono::Local::now().format("%d/%m/%Y às %H:%M")
                );
                with(vault, |v| {
                    v.upsert_assets(&assets).map_err(|e| e.to_string())?;
                    v.save_quotes(&quotes).map_err(|e| e.to_string())?;
                    v.record_quote_update(
                        CATALOG_SOURCE,
                        Some(&day.to_string()),
                        true,
                        assets.len() as i64,
                        &msg,
                    )
                    .map_err(|e| e.to_string())
                })?;
                report.push(UpdateLine {
                    source: "Catálogo B3 (lista de ativos)".into(),
                    ok: true,
                    message: msg,
                });
            }
            Err(e) => {
                let msg = describe(&e);
                with(vault, |v| {
                    v.record_quote_update(CATALOG_SOURCE, None, false, 0, &msg)
                        .map_err(|e| e.to_string())
                })?;
                report.push(UpdateLine {
                    source: "Catálogo B3 (lista de ativos)".into(),
                    ok: false,
                    message: msg,
                });
            }
        }
    }

    // 1b. Catálogo dos EUA.
    let (us_stale, us_info) = catalog_state(US_CATALOG_SOURCE, "US")?;
    if !us_stale {
        report.push(UpdateLine {
            source: "Catálogo EUA (lista de ativos)".into(),
            ok: true,
            message: us_info.unwrap_or_default(),
        });
    } else {
        match fetcher.us_catalog() {
            Ok(listings) => {
                let assets: Vec<Asset> = listings
                    .iter()
                    .map(|l| Asset {
                        ticker: l.ticker.clone(),
                        name: l.name.clone(),
                        kind: if l.etf { "ETF" } else { "ACAO" }.into(),
                        market: "US".into(),
                        currency: "USD".into(),
                        isin: None,
                        source: nasdaq::SOURCE.into(),
                    })
                    .collect();
                let msg = format!(
                    "{} ações e ETFs listados nos EUA, atualizado em {}",
                    assets.len(),
                    chrono::Local::now().format("%d/%m/%Y às %H:%M")
                );
                with(vault, |v| {
                    v.upsert_assets(&assets).map_err(|e| e.to_string())?;
                    v.record_quote_update(
                        US_CATALOG_SOURCE,
                        Some(&today.to_string()),
                        true,
                        assets.len() as i64,
                        &msg,
                    )
                    .map_err(|e| e.to_string())
                })?;
                report.push(UpdateLine {
                    source: "Catálogo EUA (lista de ativos)".into(),
                    ok: true,
                    message: msg,
                });
            }
            Err(e) => {
                let msg = describe(&e);
                with(vault, |v| {
                    v.record_quote_update(US_CATALOG_SOURCE, None, false, 0, &msg)
                        .map_err(|e| e.to_string())
                })?;
                report.push(UpdateLine {
                    source: "Catálogo EUA (lista de ativos)".into(),
                    ok: false,
                    message: msg,
                });
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
        // Mês já baixado só conta se tem todos os ativos de agora: uma nota
        // antiga importada depois traz ativos que não estavam no download.
        let with_tickers = format!(
            "{key}|{}",
            tickers.iter().cloned().collect::<Vec<_>>().join(",")
        );
        let first = NaiveDate::from_ymd_opt(y, m, 1).expect("dia 1");
        let done = with(vault, |v| {
            let e = |e: carteira_storage::VaultError| e.to_string();
            if v.last_successful_update(cotahist::SOURCE, Some(&key))
                .map_err(e)?
                .is_none()
            {
                return Ok(false);
            }
            if v.last_successful_update(cotahist::SOURCE, Some(&with_tickers))
                .map_err(e)?
                .is_some()
            {
                return Ok(true);
            }
            for t in tickers {
                if !v.has_quote_between(t, first, month_end(y, m)).map_err(e)? {
                    return Ok(false);
                }
            }
            Ok(true)
        })?;
        if done {
            continue;
        }
        match fetcher.monthly(y, m) {
            Ok(records) => {
                let quotes = closes_for(&records, tickers);
                let msg = format!("{} fechamentos de {m:02}/{y}", quotes.len());
                with(vault, |v| {
                    v.save_quotes(&quotes).map_err(|e| e.to_string())?;
                    for k in [&key, &with_tickers] {
                        v.record_quote_update(
                            cotahist::SOURCE,
                            Some(k),
                            true,
                            quotes.len() as i64,
                            &msg,
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    Ok(())
                })?;
                months_ok += 1;
            }
            Err(e) => {
                let msg = format!("{m:02}/{y}: {}", describe(&e));
                with(vault, |v| {
                    v.record_quote_update(cotahist::SOURCE, Some(&key), false, 0, &msg)
                        .map_err(|e| e.to_string())
                })?;
                missing_since.get_or_insert(NaiveDate::from_ymd_opt(y, m, 1).expect("dia 1"));
                month_errors.push(msg);
                // Sem acesso à B3 não adianta tentar os outros meses agora.
                if matches!(e, ProviderError::Network(_)) {
                    break;
                }
            }
        }
    }
    let closed_months = held.keys().filter(|k| **k < current).count();
    if months_ok == 0 && month_errors.is_empty() && closed_months > 0 {
        report.push(UpdateLine {
            source: "Fechamentos oficiais B3".into(),
            ok: true,
            message: format!("em dia: {closed_months} mês(es) encerrado(s) já baixado(s)"),
        });
    }
    if months_ok > 0 || !month_errors.is_empty() {
        let mut msg = format!("{months_ok} mês(es) baixado(s)");
        if !month_errors.is_empty() {
            msg.push_str(&format!("; falhas: {}", month_errors.join("; ")));
        }
        report.push(UpdateLine {
            source: "Fechamentos oficiais B3".into(),
            ok: month_errors.is_empty(),
            message: msg,
        });
    }

    // 3. Mês corrente e último preço (e meses que a B3 não entregou).
    let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).expect("dia 1");
    let from = missing_since.unwrap_or(month_start).min(month_start) - Duration::days(10);
    let current_tickers = held.get(&current).cloned().unwrap_or_default();
    let targets: BTreeSet<String> = if missing_since.is_some() {
        all_tickers.clone()
    } else {
        current_tickers
    };
    let mut saved = 0;
    let mut failures = Vec::new();
    // Ativos dos EUA: desde a última cotação gravada (ou a primeira operação).
    let mut us_first: BTreeMap<String, NaiveDate> = BTreeMap::new();
    for o in &us_ops {
        let e = us_first.entry(o.asset.clone()).or_insert(o.date);
        *e = (*e).min(o.date);
    }
    let mut plan: Vec<(String, String, NaiveDate)> = targets
        .iter()
        .map(|t| (t.clone(), yahoo::b3_symbol(t), from))
        .collect();
    for (t, first) in &us_first {
        let (oldest, last) = with(vault, |v| {
            Ok::<_, String>((
                v.first_quote_date(t, yahoo::SOURCE)
                    .map_err(|e| e.to_string())?,
                v.last_quote_date(t, yahoo::SOURCE)
                    .map_err(|e| e.to_string())?,
            ))
        })?;
        // Uma operação anterior às cotações gravadas (nota antiga importada
        // depois) pede o histórico desde ela.
        let start = match (oldest, last) {
            (Some(o), Some(l)) if o <= *first + Duration::days(7) => l - Duration::days(5),
            _ => *first - Duration::days(10),
        };
        plan.push((t.clone(), yahoo::us_symbol(t), start));
    }
    for (t, symbol, from) in &plan {
        match fetcher.yahoo(symbol, *from, today) {
            Ok(closes) => {
                let quotes: Vec<NewQuote> = closes
                    .into_iter()
                    .map(|(date, close)| NewQuote {
                        ticker: t.clone(),
                        date,
                        close,
                        source: yahoo::SOURCE.into(),
                    })
                    .collect();
                saved += quotes.len();
                with(vault, |v| {
                    v.save_quotes(&quotes)
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                })?;
            }
            Err(e) => failures.push(format!("{t}: {}", describe(&e))),
        }
    }
    if !plan.is_empty() {
        let mut msg = format!(
            "{saved} cotações de {} ativo(s)",
            plan.len() - failures.len()
        );
        if !failures.is_empty() {
            msg.push_str(&format!("; falhas: {}", failures.join("; ")));
        }
        let ok = failures.is_empty();
        with(vault, |v| {
            v.record_quote_update(yahoo::SOURCE, None, ok, saved as i64, &msg)
                .map_err(|e| e.to_string())
        })?;
        report.push(UpdateLine {
            source: "Últimos preços (Yahoo)".into(),
            ok,
            message: msg,
        });
    }
    // 4. PTAX desde a primeira operação em dólar (ou cripto, para a cotação
    // em dólar quando não houver par em reais).
    if let Some(first) = us_ops.iter().chain(&crypto_ops).map(|o| o.date).min() {
        let range = with(vault, |v| v.fx_range().map_err(|e| e.to_string()))?;
        // Começa alguns dias antes da primeira operação para cobrir fins de
        // semana e feriados; se o histórico já cobre, só busca o que falta.
        let wanted = first - Duration::days(10);
        let start = match range {
            Some((lo, hi)) if lo <= first => hi + Duration::days(1),
            _ => wanted,
        };
        if start > today {
            report.push(UpdateLine {
                source: "PTAX (Banco Central)".into(),
                ok: true,
                message: "em dia".into(),
            });
        } else {
            match fetcher.ptax(start, today) {
                Ok(rates) => {
                    let rows: Vec<NewFxRate> = rates
                        .iter()
                        .map(|r| NewFxRate {
                            date: r.date,
                            buy: r.buy,
                            sell: r.sell,
                            source: ptax::SOURCE.into(),
                        })
                        .collect();
                    let msg = match rows.last() {
                        Some(r) => format!(
                            "{} boletim(ns); último: {} em {}",
                            rows.len(),
                            r.sell,
                            r.date.format("%d/%m/%Y")
                        ),
                        None => "em dia".into(),
                    };
                    with(vault, |v| {
                        v.save_fx_rates(&rows).map_err(|e| e.to_string())?;
                        v.record_quote_update(ptax::SOURCE, None, true, rows.len() as i64, &msg)
                            .map_err(|e| e.to_string())
                    })?;
                    report.push(UpdateLine {
                        source: "PTAX (Banco Central)".into(),
                        ok: true,
                        message: msg,
                    });
                }
                Err(e) => {
                    let msg = describe(&e);
                    with(vault, |v| {
                        v.record_quote_update(ptax::SOURCE, None, false, 0, &msg)
                            .map_err(|e| e.to_string())
                    })?;
                    report.push(UpdateLine {
                        source: "PTAX (Banco Central)".into(),
                        ok: false,
                        message: msg,
                    });
                }
            }
        }
    }

    // 5. Criptomoedas: par em reais; sem ele, par em dólar × PTAX.
    if !crypto_ops.is_empty() {
        report.push(update_crypto(vault, fetcher, &crypto_ops, today)?);
    }

    // 6. Índices de comparação (CDI, IPCA, Ibovespa, IVVB11) desde a primeira operação.
    if let Some(first) = ops_dates.iter().min().copied() {
        report.push(update_indexes(vault, fetcher, first, today)?);
    }

    // Sem ativos na carteira não há cotação a buscar; diz isso em vez de
    // simplesmente omitir as linhas.
    if plan.is_empty() && us_ops.is_empty() && crypto_ops.is_empty() {
        report.push(UpdateLine {
            source: "Cotações".into(),
            ok: true,
            message: "nenhum ativo na carteira para buscar cotação".into(),
        });
    }
    Ok(report)
}

/// Cotações das criptomoedas em reais, desde a primeira operação de cada uma.
fn update_crypto(
    vault: Shared,
    fetcher: &dyn Fetcher,
    ops: &[Operation],
    today: NaiveDate,
) -> Result<UpdateLine, String> {
    let mut first: BTreeMap<String, NaiveDate> = BTreeMap::new();
    for o in ops {
        let e = first.entry(o.asset.clone()).or_insert(o.date);
        *e = (*e).min(o.date);
    }
    let mut saved = 0;
    let mut failures = Vec::new();
    for (t, first) in &first {
        let (oldest, last) = with(vault, |v| {
            Ok::<_, String>((
                v.first_quote_date(t, yahoo::SOURCE)
                    .map_err(|e| e.to_string())?,
                v.last_quote_date(t, yahoo::SOURCE)
                    .map_err(|e| e.to_string())?,
            ))
        })?;
        let from = match (oldest, last) {
            (Some(o), Some(l)) if o <= *first + Duration::days(7) => l - Duration::days(5),
            _ => *first - Duration::days(10),
        };
        let closes: Result<Vec<(NaiveDate, Decimal)>, String> =
            match fetcher.yahoo(&crypto::yahoo_symbol(t), from, today) {
                Ok(c) if !c.is_empty() => Ok(c),
                // Sem par em reais: em dólar, convertido pela PTAX do dia.
                brl => match fetcher.yahoo(&format!("{t}-USD"), from, today) {
                    Ok(usd) => {
                        let fx = with(vault, |v| v.fx_table().map_err(|e| e.to_string()))?;
                        Ok(usd
                            .into_iter()
                            .filter_map(|(d, c)| {
                                fx.usd_brl_on_or_before(d).map(|r| (d, c * r.rate))
                            })
                            .collect())
                    }
                    Err(e) => Err(match brl {
                        Err(e1) => describe(&e1),
                        Ok(_) => describe(&e),
                    }),
                },
            };
        match closes {
            Ok(closes) => {
                let quotes: Vec<NewQuote> = closes
                    .into_iter()
                    .map(|(date, close)| NewQuote {
                        ticker: t.clone(),
                        date,
                        close,
                        source: yahoo::SOURCE.into(),
                    })
                    .collect();
                saved += quotes.len();
                with(vault, |v| {
                    v.save_quotes(&quotes)
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                })?;
            }
            Err(e) => failures.push(format!("{t}: {e}")),
        }
    }
    let mut message = format!(
        "{saved} cotações de {} cripto(s)",
        first.len() - failures.len()
    );
    if !failures.is_empty() {
        message.push_str(&format!("; falhas: {}", failures.join("; ")));
    }
    Ok(UpdateLine {
        source: "Criptomoedas (Yahoo)".into(),
        ok: failures.is_empty(),
        message,
    })
}

/// Baixa o que falta de cada índice. Um índice que falha não impede os outros.
fn update_indexes(
    vault: Shared,
    fetcher: &dyn Fetcher,
    first: NaiveDate,
    today: NaiveDate,
) -> Result<UpdateLine, String> {
    // Um mês antes da primeira operação: cobre a véspera e o IPCA do mês.
    let wanted = (first - Duration::days(31)).with_day(1).unwrap_or(first);
    let mut done = Vec::new();
    let mut failed = Vec::new();
    for def in INDEXES {
        let range = with(vault, |v| v.index_range(def.id).map_err(|e| e.to_string()))?;
        let start = match range {
            Some((lo, hi)) if lo <= wanted + Duration::days(10) => hi + Duration::days(1),
            _ => wanted,
        };
        if start > today {
            done.push(format!("{} em dia", def.name));
            continue;
        }
        let fetched = match def.origin {
            IndexOrigin::Sgs(code) => fetcher.sgs(code, start, today),
            IndexOrigin::Yahoo(symbol) => fetcher.yahoo(symbol, start, today),
        };
        match fetched {
            Ok(values) => {
                let last = values.last().map(|(d, _)| *d);
                with(vault, |v| {
                    v.save_index_values(def.id, def.origin.source(), &values)
                        .map_err(|e| e.to_string())
                })?;
                done.push(match last {
                    Some(d) => format!("{} até {}", def.name, d.format("%d/%m/%Y")),
                    None => format!("{} em dia", def.name),
                });
            }
            // O Banco Central responde "não encontrado" quando o dia ainda não
            // foi publicado: nada novo, não é falha.
            Err(ProviderError::NotFound(_)) if start + Duration::days(10) >= today => {
                done.push(format!("{} em dia", def.name))
            }
            Err(e) => failed.push(format!("{}: {}", def.name, describe(&e))),
        }
    }
    let ok = failed.is_empty();
    let mut message = done.join("; ");
    if !ok {
        if !message.is_empty() {
            message.push_str(". ");
        }
        message.push_str(&format!("Falhou: {}", failed.join("; ")));
    }
    with(vault, |v| {
        v.record_quote_update("INDICES", None, ok, done.len() as i64, &message)
            .map_err(|e| e.to_string())
    })?;
    Ok(UpdateLine {
        source: "Índices de comparação".into(),
        ok,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use carteira_core::model::FxSource;
    use carteira_core::returns::PriceSource;
    use carteira_storage::KdfParams;
    use std::cell::RefCell;

    struct Fake {
        b3_up: bool,
        monthly_calls: RefCell<Vec<(i32, u32)>>,
        yahoo_calls: RefCell<Vec<(String, NaiveDate)>>,
        ptax_calls: RefCell<Vec<(NaiveDate, NaiveDate)>>,
    }

    fn fake(b3_up: bool) -> Fake {
        Fake {
            b3_up,
            monthly_calls: RefCell::default(),
            yahoo_calls: RefCell::default(),
            ptax_calls: RefCell::default(),
        }
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
        fn latest_daily(
            &self,
            today: NaiveDate,
        ) -> Result<(NaiveDate, Vec<cotahist::Record>), ProviderError> {
            if !self.b3_up {
                return Err(ProviderError::Network("bloqueado".into()));
            }
            Ok((
                today,
                (0..150)
                    .map(|i| rec(&format!("T{i:03}3"), today, Decimal::ONE))
                    .collect(),
            ))
        }
        fn monthly(&self, year: i32, month: u32) -> Result<Vec<cotahist::Record>, ProviderError> {
            self.monthly_calls.borrow_mut().push((year, month));
            if !self.b3_up {
                return Err(ProviderError::Network("bloqueado".into()));
            }
            Ok(vec![
                rec("TEST3", month_end(year, month), Decimal::from(month)),
                rec("OUTRO3", month_end(year, month), Decimal::ONE),
            ])
        }
        fn yahoo(
            &self,
            symbol: &str,
            from: NaiveDate,
            to: NaiveDate,
        ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError> {
            self.yahoo_calls
                .borrow_mut()
                .push((symbol.to_string(), from));
            Ok(vec![
                (from + Duration::days(10), Decimal::from(99)),
                (to, Decimal::from(99)),
            ])
        }
        fn us_catalog(&self) -> Result<Vec<nasdaq::Listing>, ProviderError> {
            Ok((0..120)
                .map(|i| nasdaq::Listing {
                    ticker: format!("U{i:03}"),
                    name: "TESTE".into(),
                    etf: i == 0,
                })
                .collect())
        }
        fn sgs(
            &self,
            _code: u32,
            _from: NaiveDate,
            to: NaiveDate,
        ) -> Result<Vec<(NaiveDate, Decimal)>, ProviderError> {
            // Como o SGS: dia ainda não publicado responde 404.
            if _from + Duration::days(2) > to {
                return Err(ProviderError::NotFound("sgs".into()));
            }
            Ok(vec![(_from, Decimal::ONE), (to, Decimal::ONE)])
        }
        fn ptax(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<ptax::Ptax>, ProviderError> {
            self.ptax_calls.borrow_mut().push((from, to));
            Ok(from
                .iter_days()
                .take_while(|d| *d <= to)
                .filter(|d| d.weekday().num_days_from_monday() < 5)
                .map(|date| ptax::Ptax {
                    date,
                    buy: Decimal::from(5),
                    sell: Decimal::from(5),
                })
                .collect())
        }
    }

    fn setup(dir: &std::path::Path) -> Mutex<Option<Vault>> {
        let mut v = Vault::create_with_params(
            dir,
            "senha-de-teste-123",
            "0.2.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
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
            currency: "BRL".into(),
            from_account: None,
            residue: None,
        })
        .unwrap();
        Mutex::new(Some(v))
    }

    #[test]
    fn cripto_busca_par_em_reais_e_entra_no_catalogo() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = setup(tmp.path());
        let first = NaiveDate::from_ymd_opt(2026, 2, 3).unwrap();
        {
            let mut g = vault.lock().unwrap();
            let v = g.as_mut().unwrap();
            let btc = |account: &str, currency: &str| Operation {
                id: String::new(),
                account: account.into(),
                asset: "btc".into(),
                date: first,
                side: Side::Compra,
                quantity: Decimal::new(123, 4),
                price: Decimal::from(350000),
                manual_fees: None,
                note: None,
                currency: currency.into(),
                from_account: None,
                residue: None,
            };
            assert!(
                v.create_operation(btc("Nomad", "USD")).is_err(),
                "cripto só em reais"
            );
            v.create_operation(btc("Carteira cripto", "BRL")).unwrap();
        }
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let f = fake(true);
        let report = update_all(&vault, &f, today).unwrap();
        assert!(
            report
                .iter()
                .any(|l| l.source.starts_with("Cripto") && l.ok),
            "{report:?}"
        );
        let calls = f.yahoo_calls.borrow().clone();
        assert!(
            calls.contains(&("BTC-BRL".into(), first - Duration::days(10))),
            "{calls:?}"
        );
        assert!(
            !calls.iter().any(|c| c.0 == "BTC.SA"),
            "não é ativo da B3: {calls:?}"
        );
        let g = vault.lock().unwrap();
        let v = g.as_ref().unwrap();
        let hit = &v.search_assets("ETH", 5).unwrap()[0];
        assert_eq!(
            (hit.kind.as_str(), hit.currency.as_str()),
            ("CRIPTO", "BRL")
        );
        assert!(v
            .price_table(&["BTC".into()])
            .unwrap()
            .latest("BTC")
            .is_some());
    }

    #[test]
    fn baixa_meses_encerrados_uma_vez_e_o_mes_corrente() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = setup(tmp.path());
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fake = fake(true);
        let report = update_all(&vault, &fake, today).unwrap();
        assert!(report.iter().all(|l| l.ok), "{report:?}");
        assert_eq!(*fake.monthly_calls.borrow(), vec![(2026, 1), (2026, 2)]);

        let again = update_all(&vault, &fake, today).unwrap();
        assert_eq!(
            fake.monthly_calls.borrow().len(),
            2,
            "mês já baixado não é baixado de novo"
        );
        assert!(
            again
                .iter()
                .any(|l| l.source.starts_with("Catálogo B3") && l.message.starts_with("em dia")),
            "{again:?}"
        );
        assert!(
            again
                .iter()
                .any(|l| l.source.contains("oficiais") && l.message.starts_with("em dia")),
            "{again:?}"
        );

        let g = vault.lock().unwrap();
        let v = g.as_ref().unwrap();
        let table = v.price_table(&["TEST3".into(), "OUTRO3".into()]).unwrap();
        assert_eq!(
            table
                .close_on_or_before("TEST3", NaiveDate::from_ymd_opt(2026, 2, 28).unwrap())
                .unwrap()
                .close,
            Decimal::from(2)
        );
        assert_eq!(
            table.close_on_or_before("TEST3", today).unwrap().close,
            Decimal::from(99)
        );
        assert!(
            table.latest("OUTRO3").is_none(),
            "só grava ativos da carteira"
        );
        assert!(v.asset_count().unwrap() >= 150);
    }

    #[test]
    fn indice_sem_dia_novo_publicado_nao_e_falha() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = setup(tmp.path());
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let f = fake(true);
        update_all(&vault, &f, today).unwrap();
        let next = update_all(&vault, &f, today + Duration::days(1)).unwrap();
        let line = next
            .iter()
            .find(|l| l.source.starts_with("Índices"))
            .unwrap();
        assert!(line.ok, "{line:?}");
        assert!(line.message.contains("CDI em dia"), "{line:?}");
    }

    #[test]
    fn sem_b3_usa_yahoo_desde_o_primeiro_mes() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = setup(tmp.path());
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let fake = fake(false);
        let report = update_all(&vault, &fake, today).unwrap();
        assert_eq!(
            fake.monthly_calls.borrow().len(),
            1,
            "para no primeiro erro de rede"
        );
        assert!(!report[0].ok);
        assert!(report.iter().any(|l| l.source.contains("Yahoo") && l.ok));
    }

    #[test]
    fn exterior_busca_historico_no_yahoo_e_ptax_uma_vez() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = setup(tmp.path());
        vault
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .create_operation(Operation {
                id: String::new(),
                account: "Exterior".into(),
                asset: "brk.b".into(),
                date: NaiveDate::from_ymd_opt(2026, 1, 20).unwrap(),
                side: Side::Compra,
                quantity: Decimal::from(2),
                price: Decimal::from(400),
                manual_fees: None,
                note: None,
                currency: "USD".into(),
                from_account: None,
                residue: None,
            })
            .unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let f = fake(true);
        let report = update_all(&vault, &f, today).unwrap();
        assert!(report.iter().all(|l| l.ok), "{report:?}");
        assert!(report.iter().any(|l| l.source.starts_with("Catálogo EUA")));
        assert!(
            report
                .iter()
                .any(|l| l.source.starts_with("PTAX") && l.message.contains("boletim")),
            "{report:?}"
        );
        let calls = f.yahoo_calls.borrow().clone();
        assert!(
            calls.contains(&(
                "BRK-B".into(),
                NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()
            )),
            "{calls:?}"
        );
        assert_eq!(
            *f.monthly_calls.borrow(),
            vec![(2026, 1), (2026, 2)],
            "COTAHIST só para ativos da B3"
        );

        let again = update_all(&vault, &f, today).unwrap();
        assert_eq!(f.ptax_calls.borrow().len(), 1, "PTAX já baixada até hoje");
        assert!(
            again
                .iter()
                .any(|l| l.source.starts_with("PTAX") && l.message == "em dia"),
            "{again:?}"
        );
        let last_us = f
            .yahoo_calls
            .borrow()
            .iter()
            .rev()
            .find(|c| c.0 == "BRK-B")
            .cloned()
            .unwrap();
        assert_eq!(
            last_us.1,
            today - Duration::days(5),
            "depois só busca a partir da última cotação"
        );

        // Nota antiga importada depois: busca o histórico desde ela.
        let old = NaiveDate::from_ymd_opt(2025, 6, 2).unwrap();
        {
            let mut g = vault.lock().unwrap();
            let v = g.as_mut().unwrap();
            for (asset, currency) in [("BRK.B", "USD"), ("OUTRO3", "BRL")] {
                v.create_operation(Operation {
                    id: String::new(),
                    account: format!("Antiga {currency}"),
                    asset: asset.into(),
                    date: if currency == "USD" {
                        old
                    } else {
                        NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()
                    },
                    side: Side::Compra,
                    quantity: Decimal::from(1),
                    price: Decimal::from(10),
                    manual_fees: None,
                    note: None,
                    currency: currency.into(),
                    from_account: None,
                    residue: None,
                })
                .unwrap();
            }
        }
        let before = f.monthly_calls.borrow().len();
        update_all(&vault, &f, today).unwrap();
        let calls = f.yahoo_calls.borrow().clone();
        assert!(
            calls.contains(&("BRK-B".into(), old - Duration::days(10))),
            "{calls:?}"
        );
        assert!(
            f.monthly_calls.borrow()[before..].contains(&(2026, 1)),
            "mês baixado de novo para o ativo que faltava: {:?}",
            f.monthly_calls.borrow()
        );
        let after = f.monthly_calls.borrow().len();
        update_all(&vault, &f, today).unwrap();
        assert_eq!(f.monthly_calls.borrow().len(), after, "e só uma vez");

        let g = vault.lock().unwrap();
        let v = g.as_ref().unwrap();
        assert!(v.fx_table().unwrap().usd_brl_on_or_before(old).is_some());
        assert!(v.fx_table().unwrap().usd_brl_on_or_before(today).is_some());
        assert_eq!(v.search_assets("U000", 5).unwrap()[0].kind, "ETF");
    }
}
