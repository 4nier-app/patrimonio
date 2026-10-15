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

// ---------- painel (etapa 3) ----------

/// Um mês da carteira no painel, em reais.
#[derive(Serialize)]
pub struct DashMonth {
    pub year: i32,
    pub month: u32,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    /// Patrimônio no fim do período.
    pub value: Option<Decimal>,
    /// Valor aplicado: soma das compras menos as vendas até o fim do mês.
    pub invested: Decimal,
    pub profit: Option<Decimal>,
    pub rate: Option<Decimal>,
    /// Queda do acumulado desde o maior valor (0 ou negativo).
    pub drawdown: Option<Decimal>,
    pub explanation: Vec<String>,
}

/// Cartão de rentabilidade com o CDI do mesmo período.
#[derive(Serialize)]
pub struct Card {
    pub label: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub rate: Option<Decimal>,
    pub cdi: Option<Decimal>,
    pub explanation: Vec<String>,
}

#[derive(Serialize)]
pub struct SliceAsset {
    pub asset: String,
    pub value: Option<Decimal>,
}

/// Fatia da carteira por tipo de ativo.
#[derive(Serialize)]
pub struct Slice {
    pub label: String,
    pub value: Option<Decimal>,
    pub assets: Vec<SliceAsset>,
}

/// Lucro de um ativo em cada mês do painel (mesma ordem de `months`).
#[derive(Serialize)]
pub struct AssetMonths {
    pub asset: String,
    pub profits: Vec<Option<Decimal>>,
}

/// Período em que um ativo esteve na carteira.
#[derive(Serialize)]
pub struct Span {
    pub asset: String,
    pub accounts: Vec<String>,
    pub start: NaiveDate,
    /// Data da venda que zerou a posição; `None` se ainda está na carteira.
    pub end: Option<NaiveDate>,
}

#[derive(Serialize)]
pub struct DashboardView {
    pub as_of: NaiveDate,
    pub months: Vec<DashMonth>,
    pub cards: Vec<Card>,
    pub benchmarks: Vec<carteira_core::benchmark::BenchmarkView>,
    pub allocation: Vec<Slice>,
    pub contributions: Vec<AssetMonths>,
    pub positions: Vec<Position>,
    pub timeline: Vec<Span>,
}

fn kind_label(kind: Option<&str>, currency: &str) -> &'static str {
    match (currency == BRL, kind) {
        (true, Some("ACAO")) => "Ações",
        (true, Some("FII")) => "FIIs",
        (true, Some("ETF")) => "ETFs",
        (true, Some("BDR")) => "BDRs",
        (true, _) => "Outros (B3)",
        (false, Some("ETF")) => "ETFs no exterior",
        (false, _) => "Ações no exterior",
    }
}

pub fn dashboard(v: &Vault, as_of: NaiveDate) -> Result<DashboardView, String> {
    use carteira_core::benchmark::{chain, compare, drawdowns, IndexSeries};

    let ops = priced(v)?;
    let table = prices_for(v, &ops)?;
    let fx = v.fx_table().map_err(s)?;
    let trades = trades_of(&ops);

    let months = monthly_returns_in(
        &trades,
        &table,
        as_of,
        &Scope::Carteira,
        Conversion::ToBrl(&fx),
    );
    let rates: Vec<Option<Decimal>> = months.iter().map(|m| m.rate).collect();
    let dd = drawdowns(&rates);
    let mut invested = Decimal::ZERO;
    let dash_months: Vec<DashMonth> = months
        .iter()
        .zip(dd)
        .map(|(m, drawdown)| {
            invested += m.net_flows;
            DashMonth {
                year: m.year,
                month: m.month,
                period_start: m.period_start,
                period_end: m.period_end,
                value: m.end.total,
                invested,
                profit: m.profit,
                rate: m.rate,
                drawdown,
                explanation: m.explanation.clone(),
            }
        })
        .collect();

    // Índices de comparação: os baixados e o dólar (PTAX).
    let mut series: Vec<IndexSeries> = Vec::new();
    for def in crate::market::INDEXES {
        let (points, source) = v.index_points(def.id).map_err(s)?;
        series.push(IndexSeries {
            id: def.id.into(),
            name: def.name.into(),
            kind: def.kind,
            source: source.unwrap_or_else(|| def.origin.source().into()),
            points,
        });
    }
    series.push(IndexSeries {
        id: "DOLAR".into(),
        name: "Dólar (PTAX)".into(),
        kind: carteira_core::benchmark::IndexKind::Level,
        source: carteira_providers::ptax::SOURCE.into(),
        points: fx.points(),
    });
    let benchmarks: Vec<_> = series.iter().map(|s| compare(s, &months)).collect();

    // Cartões: desde o início, 12 meses e último mês.
    let cdi = benchmarks.iter().find(|b| b.id == "CDI");
    let n = months.len();
    let card = |label: &str, from_idx: usize| -> Option<Card> {
        let slice = months.get(from_idx..)?;
        let first = slice.first()?;
        let last = slice.last()?;
        let rate = chain(&rates[from_idx..]);
        let cdi_rate = cdi.and_then(|c| chain(&c.rates[from_idx..]));
        Some(Card {
            label: label.into(),
            from: first.period_start,
            to: last.period_end,
            rate,
            cdi: cdi_rate,
            explanation: vec![
                format!(
                    "Carteira: encadeamento de {} mês(es), (1 + r₁)·(1 + r₂)·… − 1 = {}",
                    slice.len(),
                    rate.map(|r| carteira_core::format::pct(r, 2))
                        .unwrap_or_else(|| "indisponível".into())
                ),
                format!(
                    "CDI nos mesmos períodos = {}",
                    cdi_rate
                        .map(|r| carteira_core::format::pct(r, 2))
                        .unwrap_or_else(|| "indisponível".into())
                ),
            ],
        })
    };
    let cards: Vec<Card> = [
        card("Desde o início", 0),
        card("Últimos 12 meses", n.saturating_sub(12)),
        card("Último mês", n.saturating_sub(1)),
    ]
    .into_iter()
    .flatten()
    .collect();

    // Alocação por tipo, só posições abertas.
    let mut positions = positions_with_fx(&trades, &table, Some(&fx), as_of);
    positions.sort_by(|a, b| a.asset.cmp(&b.asset));
    let mut slices: Vec<Slice> = Vec::new();
    for p in positions.iter().filter(|p| !p.quantity.is_zero()) {
        let asset = v.get_asset(&p.asset).map_err(s)?;
        let label = kind_label(asset.as_ref().map(|a| a.kind.as_str()), &p.currency);
        let item = SliceAsset {
            asset: p.asset.clone(),
            value: p.market_value_brl,
        };
        match slices.iter_mut().find(|s| s.label == label) {
            Some(sl) => {
                sl.value = sl.value.zip(item.value).map(|(a, b)| a + b);
                sl.assets.push(item);
            }
            None => slices.push(Slice {
                label: label.into(),
                value: item.value,
                assets: vec![item],
            }),
        }
    }
    slices.sort_by(|a, b| {
        b.value
            .unwrap_or_default()
            .cmp(&a.value.unwrap_or_default())
    });

    // Lucro de cada ativo por mês, para "o que puxou o resultado".
    let tickers: BTreeSet<String> = trades.iter().map(|t| t.asset.clone()).collect();
    let contributions: Vec<AssetMonths> = tickers
        .into_iter()
        .map(|t| {
            let am = monthly_returns_in(
                &trades,
                &table,
                as_of,
                &Scope::Ativo(t.clone()),
                Conversion::ToBrl(&fx),
            );
            let profits = months
                .iter()
                .map(|m| {
                    am.iter()
                        .find(|a| a.year == m.year && a.month == m.month)
                        .and_then(|a| a.profit)
                })
                .collect();
            AssetMonths { asset: t, profits }
        })
        .collect();

    Ok(DashboardView {
        as_of,
        months: dash_months,
        cards,
        benchmarks,
        allocation: slices,
        contributions,
        positions,
        timeline: timeline(&ops),
    })
}

/// Períodos de cada ativo na carteira, somando todas as contas.
fn timeline(ops: &[PricedOperation]) -> Vec<Span> {
    use std::collections::BTreeMap;
    let mut sorted: Vec<&Operation> = ops.iter().map(|p| &p.operation).collect();
    sorted.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
    let mut open: BTreeMap<String, (Decimal, Span)> = BTreeMap::new();
    let mut spans = Vec::new();
    for op in sorted {
        let delta = match op.side {
            carteira_core::model::Side::Compra => op.quantity,
            carteira_core::model::Side::Venda => -op.quantity,
            carteira_core::model::Side::Transferencia => -op.residue.unwrap_or_default(),
        };
        let entry = open.entry(op.asset.clone()).or_insert_with(|| {
            (
                Decimal::ZERO,
                Span {
                    asset: op.asset.clone(),
                    accounts: Vec::new(),
                    start: op.date,
                    end: None,
                },
            )
        });
        if entry.0.is_zero() {
            entry.1.start = op.date;
            entry.1.accounts.clear();
        }
        if !entry.1.accounts.contains(&op.account) {
            entry.1.accounts.push(op.account.clone());
        }
        entry.0 += delta;
        if entry.0 <= Decimal::ZERO {
            let (_, mut span) = open.remove(&op.asset).expect("existe");
            span.end = Some(op.date);
            spans.push(span);
        }
    }
    spans.extend(open.into_values().map(|(_, s)| s));
    spans.sort_by(|a, b| a.start.cmp(&b.start).then(a.asset.cmp(&b.asset)));
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use carteira_core::model::Side;
    use carteira_storage::{KdfParams, NewQuote};
    use rust_decimal_macros::dec;

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, day).unwrap()
    }

    fn op(asset: &str, date: NaiveDate, side: Side, q: Decimal) -> Operation {
        Operation {
            id: String::new(),
            account: "Corretora X".into(),
            asset: asset.into(),
            date,
            side,
            quantity: q,
            price: dec!(10),
            manual_fees: Some(Decimal::ZERO),
            note: None,
            currency: BRL.into(),
            from_account: None,
            residue: None,
        }
    }

    #[test]
    fn painel_com_indice_e_linha_do_tempo() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-1",
            "0.5.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        v.create_operation(op("TEST3", d(3, 2), Side::Compra, dec!(100)))
            .unwrap();
        v.create_operation(op("OUTR3", d(3, 10), Side::Compra, dec!(10)))
            .unwrap();
        v.create_operation(op("OUTR3", d(4, 10), Side::Venda, dec!(10)))
            .unwrap();
        let q = |t: &str, date, close| NewQuote {
            ticker: t.into(),
            date,
            close,
            source: "YAHOO".into(),
        };
        v.save_quotes(&[
            q("TEST3", d(3, 2), dec!(10)),
            q("TEST3", d(3, 31), dec!(11)),
            q("TEST3", d(4, 30), dec!(11)),
            q("OUTR3", d(3, 10), dec!(10)),
            q("OUTR3", d(3, 31), dec!(10)),
            q("OUTR3", d(4, 9), dec!(10)),
        ])
        .unwrap();
        // CDI de 0,05% ao dia em todos os dias úteis do período.
        let cdi: Vec<(NaiveDate, Decimal)> = d(2, 1)
            .iter_days()
            .take_while(|x| *x <= d(4, 30))
            .map(|x| (x, dec!(0.05)))
            .collect();
        v.save_index_values("CDI", "BCB-SGS", &cdi).unwrap();

        let view = dashboard(&v, d(4, 30)).unwrap();
        assert_eq!(view.months.len(), 2);
        assert_eq!(view.months[0].invested, dec!(1100));
        // A venda devolve R$ 100: aplicado cai para R$ 1.000.
        assert_eq!(view.months[1].invested, dec!(1000));
        assert_eq!(view.months[0].value, Some(dec!(1200)));
        let cdi = view.benchmarks.iter().find(|b| b.id == "CDI").unwrap();
        assert!(cdi.rates.iter().all(|r| r.is_some()));
        assert!(cdi.same_flows[1].unwrap() > dec!(1000));
        let ibov = view.benchmarks.iter().find(|b| b.id == "IBOV").unwrap();
        assert!(ibov.rates.iter().all(|r| r.is_none()));
        assert_eq!(view.cards.len(), 3);
        assert!(view.cards[0].cdi.is_some());
        assert_eq!(view.allocation.len(), 1);
        assert_eq!(view.allocation[0].assets[0].asset, "TEST3");
        assert_eq!(view.timeline.len(), 2);
        let outr = view.timeline.iter().find(|s| s.asset == "OUTR3").unwrap();
        assert_eq!((outr.start, outr.end), (d(3, 10), Some(d(4, 10))));
        assert_eq!(
            view.timeline
                .iter()
                .find(|s| s.asset == "TEST3")
                .unwrap()
                .end,
            None
        );
        let test3 = view
            .contributions
            .iter()
            .find(|c| c.asset == "TEST3")
            .unwrap();
        assert_eq!(test3.profits[0], Some(dec!(100)));
    }
}
