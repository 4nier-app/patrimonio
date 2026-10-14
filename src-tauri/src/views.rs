//! Montagem das telas a partir do cofre e das regras do núcleo.

use std::collections::BTreeSet;

use carteira_core::fees::{default_rules, NoteFee};
use carteira_core::operations::{price_operations, validate, Operation, PricedOperation};
use carteira_core::portfolio::{positions, Position};
use carteira_core::returns::{monthly_returns, yearly_returns, MonthResult, Scope, YearResult};
use carteira_storage::{normalize_ticker, PriceTable, Vault};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

fn s<E: ToString>(e: E) -> String {
    e.to_string()
}

fn priced(v: &Vault) -> Result<Vec<PricedOperation>, String> {
    let ops = v.list_operations().map_err(s)?;
    Ok(price_operations(&default_rules(), &ops).map_err(s)?.0)
}

fn prices_for(v: &Vault, ops: &[PricedOperation]) -> Result<PriceTable, String> {
    let tickers: Vec<String> = ops.iter().map(|p| p.operation.asset.clone()).collect::<BTreeSet<_>>().into_iter().collect();
    v.price_table(&tickers).map_err(s)
}

/// Lançamentos do mais recente para o mais antigo.
pub fn list_operations(v: &Vault) -> Result<Vec<PricedOperation>, String> {
    let mut list = priced(v)?;
    list.reverse();
    Ok(list)
}

#[derive(Serialize)]
pub struct Preview {
    pub valid: Option<String>,
    pub gross: Decimal,
    pub fees: Decimal,
    pub fee_origin: &'static str,
    /// Taxas da nota inteira (conta + pregão) com a fórmula de cada uma.
    pub note_fees: Vec<NoteFee>,
    pub note_operations: usize,
}

/// Taxas que o lançamento terá, considerando os outros da mesma nota.
pub fn preview(v: &Vault, mut draft: Operation) -> Result<Preview, String> {
    draft.asset = normalize_ticker(&draft.asset);
    draft.account = draft.account.trim().to_string();
    if draft.id.is_empty() {
        draft.id = "rascunho".into();
    }
    if let Err(msg) = validate(&draft) {
        return Ok(Preview {
            valid: Some(msg),
            gross: Decimal::ZERO,
            fees: Decimal::ZERO,
            fee_origin: "automática",
            note_fees: vec![],
            note_operations: 0,
        });
    }
    let mut ops: Vec<Operation> = v.list_operations().map_err(s)?.into_iter().filter(|o| o.id != draft.id).collect();
    ops.push(draft.clone());
    let (list, notes) = price_operations(&default_rules(), &ops).map_err(s)?;
    let me = list.iter().find(|p| p.operation.id == draft.id).expect("rascunho incluído");
    let note = notes.iter().find(|n| n.operation_ids.contains(&draft.id));
    Ok(Preview {
        valid: None,
        gross: me.gross,
        fees: me.fees,
        fee_origin: me.fee_origin,
        note_fees: note.and_then(|n| n.fees.as_ref()).map(|f| f.fees.clone()).unwrap_or_default(),
        note_operations: note.map(|n| n.operation_ids.len()).unwrap_or(1),
    })
}

#[derive(Serialize)]
pub struct PortfolioView {
    pub as_of: NaiveDate,
    pub positions: Vec<Position>,
    pub cost: Decimal,
    pub market_value: Option<Decimal>,
    pub unrealized: Option<Decimal>,
    pub realized: Decimal,
    pub last_update: Option<String>,
}

pub fn portfolio(v: &Vault, as_of: NaiveDate) -> Result<PortfolioView, String> {
    let ops = priced(v)?;
    let table = prices_for(v, &ops)?;
    let trades: Vec<_> = ops.iter().map(|p| p.trade()).collect();
    let mut list = positions(&trades, &table, as_of);
    // Posições abertas primeiro; depois as encerradas (só resultado realizado).
    list.sort_by(|a, b| a.quantity.is_zero().cmp(&b.quantity.is_zero()).then(a.asset.cmp(&b.asset)));
    let open: Vec<&Position> = list.iter().filter(|p| !p.quantity.is_zero()).collect();
    let cost = open.iter().map(|p| p.cost).sum();
    let market_value = open.iter().map(|p| p.market_value).sum::<Option<Decimal>>();
    let unrealized = market_value.map(|m: Decimal| m - cost);
    let realized = list.iter().map(|p| p.realized).sum();
    let last_update = [carteira_providers::yahoo::SOURCE, carteira_providers::cotahist::SOURCE]
        .iter()
        .filter_map(|src| v.last_successful_update(src, None).ok().flatten())
        .max();
    Ok(PortfolioView { as_of, positions: list, cost, market_value, unrealized, realized, last_update })
}

#[derive(Serialize)]
pub struct ScopeReturns {
    pub asset: Option<String>,
    pub months: Vec<MonthResult>,
    pub years: Vec<YearResult>,
}

#[derive(Serialize)]
pub struct ReturnsView {
    pub as_of: NaiveDate,
    pub portfolio: ScopeReturns,
    pub assets: Vec<ScopeReturns>,
}

pub fn returns(v: &Vault, as_of: NaiveDate) -> Result<ReturnsView, String> {
    let ops = priced(v)?;
    let table = prices_for(v, &ops)?;
    let trades: Vec<_> = ops.iter().map(|p| p.trade()).collect();
    let scope = |scope: Scope, asset: Option<String>| {
        let months = monthly_returns(&trades, &table, as_of, &scope);
        let years = yearly_returns(&months);
        ScopeReturns { asset, months, years }
    };
    let tickers: BTreeSet<String> = trades.iter().map(|t| t.asset.clone()).collect();
    Ok(ReturnsView {
        as_of,
        portfolio: scope(Scope::Carteira, None),
        assets: tickers.into_iter().map(|t| scope(Scope::Ativo(t.clone()), Some(t))).collect(),
    })
}
