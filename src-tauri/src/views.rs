//! Montagem das telas a partir do cofre e das regras do núcleo.

use std::collections::BTreeSet;

use carteira_core::fees::{default_rules, NoteFee};
use carteira_core::model::{FxRate, FxSource, BRL};
use carteira_core::operations::{
    price_operations, trades_of, validate, Operation, PricedOperation,
};
use carteira_core::portfolio::{positions_with_fx, Position};
use carteira_core::returns::{
    monthly_returns_in, yearly_returns, Conversion, MonthResult, Scope, YearResult,
};
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
    let tickers: Vec<String> = ops
        .iter()
        .map(|p| p.operation.asset.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
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
    // A moeda de uma conta existente vem dela.
    if let Some(acc) = v.get_account(&draft.account).map_err(s)? {
        draft.currency = acc.currency;
    }
    if draft.id.is_empty() {
        draft.id = "rascunho".into();
    }
    draft.apply_total();
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
    let mut ops: Vec<Operation> = v
        .list_operations()
        .map_err(s)?
        .into_iter()
        .filter(|o| o.id != draft.id)
        .collect();
    ops.push(draft.clone());
    let (list, notes) = price_operations(&default_rules(), &ops).map_err(s)?;
    let me = list
        .iter()
        .find(|p| p.operation.id == draft.id)
        .expect("rascunho incluído");
    let note = notes.iter().find(|n| n.operation_ids.contains(&draft.id));
    Ok(Preview {
        valid: None,
        gross: me.gross,
        fees: me.fees,
        fee_origin: me.fee_origin,
        note_fees: note
            .and_then(|n| n.fees.as_ref())
            .map(|f| f.fees.clone())
            .unwrap_or_default(),
        note_operations: note.map(|n| n.operation_ids.len()).unwrap_or(1),
    })
}

/// Totais em reais. Ativos no exterior entram convertidos pela PTAX; os
/// totais ficam vazios se faltar a PTAX de alguma data.
#[derive(Serialize)]
pub struct PortfolioView {
    pub as_of: NaiveDate,
    pub positions: Vec<Position>,
    pub cost: Option<Decimal>,
    pub market_value: Option<Decimal>,
    pub unrealized: Option<Decimal>,
    pub realized: Option<Decimal>,
    /// PTAX usada para o valor de hoje, quando há ativos em dólar.
    pub fx: Option<FxRate>,
    pub last_update: Option<String>,
}

pub fn portfolio(v: &Vault, as_of: NaiveDate) -> Result<PortfolioView, String> {
    let ops = priced(v)?;
    let table = prices_for(v, &ops)?;
    let fx = v.fx_table().map_err(s)?;
    let trades = trades_of(&ops);
    let mut list = positions_with_fx(&trades, &table, Some(&fx), as_of);
    // Posições abertas primeiro (reais antes de dólar); depois as encerradas.
    list.sort_by(|a, b| {
        a.quantity
            .is_zero()
            .cmp(&b.quantity.is_zero())
            .then((a.currency != BRL).cmp(&(b.currency != BRL)))
            .then(a.asset.cmp(&b.asset))
    });
    let open: Vec<&Position> = list.iter().filter(|p| !p.quantity.is_zero()).collect();
    let cost = open.iter().map(|p| p.cost_brl).sum::<Option<Decimal>>();
    let market_value = open
        .iter()
        .map(|p| p.market_value_brl)
        .sum::<Option<Decimal>>();
    let unrealized = market_value.zip(cost).map(|(m, c)| m - c);
    let realized = list.iter().map(|p| p.realized_brl).sum::<Option<Decimal>>();
    let has_usd = list.iter().any(|p| p.currency != BRL);
    let last_update = [
        carteira_providers::yahoo::SOURCE,
        carteira_providers::cotahist::SOURCE,
    ]
    .iter()
    .filter_map(|src| v.last_successful_update(src, None).ok().flatten())
    .max();
    Ok(PortfolioView {
        as_of,
        positions: list,
        cost,
        market_value,
        unrealized,
        realized,
        fx: if has_usd {
            fx.usd_brl_on_or_before(as_of)
        } else {
            None
        },
        last_update,
    })
}

/// Rentabilidade em reais. Para ativos no exterior, também em dólar e o
/// efeito do câmbio: (1 + em reais) = (1 + em dólar) × (1 + câmbio).
#[derive(Serialize)]
pub struct ScopeReturns {
    pub asset: Option<String>,
    pub currency: String,
    pub months: Vec<MonthResult>,
    pub years: Vec<YearResult>,
    pub native: Option<NativeReturns>,
}

#[derive(Serialize)]
pub struct NativeReturns {
    pub months: Vec<MonthResult>,
    pub years: Vec<YearResult>,
    /// Efeito do câmbio de cada mês, na ordem de `months` em reais.
    pub fx_months: Vec<Option<Decimal>>,
    /// Efeito do câmbio de cada ano, na ordem de `years` em reais.
    pub fx_years: Vec<Option<Decimal>>,
}

/// Câmbio implícito: (1 + em reais) ÷ (1 + em dólar) − 1.
fn fx_effect(brl: Option<Decimal>, usd: Option<Decimal>) -> Option<Decimal> {
    let (b, u) = (brl?, usd?);
    let base = Decimal::ONE + u;
    if base.is_zero() {
        return None;
    }
    Some(((Decimal::ONE + b) / base - Decimal::ONE).round_dp(10))
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
    let fx = v.fx_table().map_err(s)?;
    let trades = trades_of(&ops);
    let scope = |scope: Scope, asset: Option<String>, currency: &str| {
        let months = monthly_returns_in(&trades, &table, as_of, &scope, Conversion::ToBrl(&fx));
        let years = yearly_returns(&months);
        let native = (currency != BRL).then(|| {
            let n_months = monthly_returns_in(&trades, &table, as_of, &scope, Conversion::Native);
            let n_years = yearly_returns(&n_months);
            let fx_months = months
                .iter()
                .map(|m| {
                    let u = n_months
                        .iter()
                        .find(|n| n.year == m.year && n.month == m.month)
                        .and_then(|n| n.rate);
                    fx_effect(m.rate, u)
                })
                .collect();
            let fx_years = years
                .iter()
                .map(|y| {
                    fx_effect(
                        y.rate,
                        n_years
                            .iter()
                            .find(|n| n.year == y.year)
                            .and_then(|n| n.rate),
                    )
                })
                .collect();
            NativeReturns {
                months: n_months,
                years: n_years,
                fx_months,
                fx_years,
            }
        });
        ScopeReturns {
            asset,
            currency: currency.to_string(),
            months,
            years,
            native,
        }
    };
    let tickers: BTreeSet<(String, String)> = trades
        .iter()
        .map(|t| (t.asset.clone(), t.currency.clone()))
        .collect();
    Ok(ReturnsView {
        as_of,
        portfolio: scope(Scope::Carteira, None, BRL),
        assets: tickers
            .into_iter()
            .map(|(t, c)| scope(Scope::Ativo(t.clone()), Some(t), &c))
            .collect(),
    })
}
