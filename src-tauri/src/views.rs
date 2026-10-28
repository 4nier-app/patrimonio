//! Montagem das telas a partir do cofre e das regras do núcleo.

use std::collections::{BTreeMap, BTreeSet};

use carteira_core::fees::{default_rules, NoteFee};
use carteira_core::model::{FxRate, FxSource, BRL};
use carteira_core::operations::{
    price_operations, trades_of, validate, Operation, PricedOperation,
};
use carteira_core::portfolio::{positions_with_fx, Position};
use carteira_core::returns::{
    monthly_returns_in, window_result, yearly_returns, Conversion, MonthResult, Scope, YearResult,
};
use carteira_core::savings::{SavingsMove, SavingsReport};
use carteira_storage::{normalize_ticker, DarfPayment, PriceTable, Vault};
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::Serialize;

fn s<E: ToString>(e: E) -> String {
    e.to_string()
}

/// Lançamentos mais os ajustes dos eventos corporativos: é a base de todos
/// os cálculos (carteira, rentabilidade, impostos).
/// Taxas de cada operação: B3 automática, mais a corretagem da conta nas
/// ordens sem taxa informada.
pub fn price_all(
    v: &Vault,
    ops: &[Operation],
) -> Result<
    (
        Vec<PricedOperation>,
        Vec<carteira_core::operations::NoteGroup>,
    ),
    String,
> {
    let brokerage = v.brokerage_table().map_err(s)?;
    carteira_core::operations::price_operations_with(&default_rules(), ops, &brokerage).map_err(s)
}

pub fn engine_operations(v: &Vault) -> Result<Vec<Operation>, String> {
    let mut ops = v.list_operations().map_err(s)?;
    let events = v.list_corporate_events().map_err(s)?;
    let (_, adjustments) = carteira_core::corporate::apply(&ops, &events);
    ops.extend(adjustments);
    // Cofrinhos entram como cotas; FGTS, conta e benefício ficam de fora.
    for r in savings_reports(v)? {
        if r.savings.kind.is_investment() {
            ops.extend(r.operations);
        }
    }
    Ok(ops)
}

fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// Cada saldo cadastrado, pelos movimentos e saldos informados.
pub fn savings_reports(v: &Vault) -> Result<Vec<SavingsReport>, String> {
    let list = v.list_savings().map_err(s)?;
    if list.is_empty() {
        return Ok(Vec::new());
    }
    let moves = v.list_savings_moves().map_err(s)?;
    let today = today();
    // CDI e IPCA para a renda fixa calculada pela taxa.
    let cdi = v.index_points("CDI").map_err(s)?.0;
    let ipca = v.index_points("IPCA").map_err(s)?.0;
    let idx = carteira_core::fixed_curve::Indexes {
        cdi: &cdi,
        ipca: &ipca,
    };
    Ok(list
        .iter()
        .map(|sv| {
            let mine: Vec<SavingsMove> = moves
                .iter()
                .filter(|m| m.savings_id == sv.id)
                .cloned()
                .collect();
            carteira_core::savings::build_with(sv, &mine, today, Some(&idx))
        })
        .collect())
}

fn priced(v: &Vault) -> Result<Vec<PricedOperation>, String> {
    let ops = engine_operations(v)?;
    Ok(price_all(v, &ops)?.0)
}

fn prices_for(v: &Vault, ops: &[PricedOperation]) -> Result<PriceTable, String> {
    let tickers: Vec<String> = ops
        .iter()
        .map(|p| p.operation.asset.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut table = v.price_table(&tickers).map_err(s)?;
    if tickers
        .iter()
        .any(|t| carteira_core::savings::is_savings(t))
    {
        add_savings_quotes(v, &mut table)?;
    }
    Ok(table)
}

/// Valor da cota de cada cofrinho, calculado (não vem de nenhuma fonte).
pub fn add_savings_quotes(v: &Vault, table: &mut PriceTable) -> Result<(), String> {
    for r in savings_reports(v)? {
        for (date, close) in &r.unit_values {
            table.insert(carteira_core::model::Quote {
                id: 0,
                asset: r.ticker.clone(),
                date: *date,
                close: *close,
                source: carteira_core::savings::SOURCE.into(),
            });
        }
    }
    Ok(())
}

/// Lançamentos do mais recente para o mais antigo.
pub fn list_operations(v: &Vault) -> Result<Vec<PricedOperation>, String> {
    let ops = v.list_operations().map_err(s)?;
    let mut list = price_all(v, &ops)?.0;
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
    carteira_core::operations::fill_come_cotas(&mut draft);
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
    let (list, notes) = price_all(v, &ops)?;
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
    /// Nome de cada ativo no catálogo (fundos são listados pelo CNPJ).
    pub names: BTreeMap<String, String>,
    /// Tipo de cada ativo no catálogo (ACAO, FII, FUNDO, CRIPTO...).
    pub kinds: BTreeMap<String, String>,
    /// Corretoras em dólar (sem criptomoedas): quanto chegaria no Brasil.
    pub dollar_accounts: Vec<DollarAccount>,
}

/// Saldo de uma corretora em dólar e a conta do valor em reais: pelo valor
/// de resgate (com o custo para trazer ao Brasil informado) ou pela PTAX.
#[derive(Serialize)]
pub struct DollarAccount {
    pub account: String,
    /// Saldo em dólar; vazio se faltar cotação de algum ativo.
    pub usd: Option<Decimal>,
    /// Taxa de conversão da corretora, em %; vazia = pela PTAX.
    pub fee_pct: Option<Decimal>,
    pub iof_pct: Decimal,
    pub total_pct: Option<Decimal>,
    /// Dólar comercial usado no valor de resgate.
    pub commercial: Option<FxRate>,
    /// Quando o dólar comercial foi atualizado.
    pub commercial_updated: Option<String>,
    /// Câmbio usado (valor de resgate ou PTAX) e o valor em reais.
    pub value_fx: Option<FxRate>,
    pub value_brl: Option<Decimal>,
    /// PTAX e o valor em reais para o imposto de renda.
    pub ptax: Option<FxRate>,
    pub ptax_brl: Option<Decimal>,
}

fn dollar_accounts(
    v: &Vault,
    open: &[&Position],
    fx: &carteira_storage::ValueFx,
    as_of: NaiveDate,
) -> Result<Vec<DollarAccount>, String> {
    let mut out = Vec::new();
    let updated = v.last_successful_update("YAHOO_DOLAR", None).map_err(s)?;
    for a in v.list_accounts().map_err(s)? {
        if a.currency == BRL {
            continue;
        }
        let held: Vec<(&Position, Decimal)> = open
            .iter()
            .filter(|p| p.currency != BRL && !carteira_core::crypto::is_crypto(&p.asset))
            .filter_map(|p| {
                p.by_account
                    .get(&a.name)
                    .filter(|q| **q > Decimal::ZERO)
                    .map(|q| (*p, *q))
            })
            .collect();
        if held.is_empty() {
            continue;
        }
        let usd = held
            .iter()
            .map(|(p, q)| p.quote.as_ref().map(|c| c.close * q))
            .sum::<Option<Decimal>>()
            .map(|u| u.round_dp(2));
        let fee_pct = fx.fee(&a.name);
        let value_fx = fx.value_rate(&a.name, as_of);
        let ptax = fx.usd_brl_on_or_before(as_of);
        out.push(DollarAccount {
            usd,
            fee_pct,
            iof_pct: carteira_storage::IOF_PCT,
            total_pct: fee_pct.map(carteira_storage::total_pct),
            commercial: fee_pct.and(fx.commercial_on_or_before(as_of)),
            commercial_updated: fee_pct.and(updated.clone()),
            value_brl: usd
                .zip(value_fx.as_ref())
                .map(|(u, f)| (u * f.rate).round_dp(2)),
            value_fx,
            ptax_brl: usd
                .zip(ptax.as_ref())
                .map(|(u, f)| (u * f.rate).round_dp(2)),
            ptax,
            account: a.name,
        });
    }
    Ok(out)
}

/// Código do ativo → nome ou tipo no catálogo.
type ByAsset = BTreeMap<String, String>;

/// Nome e tipo dos ativos lançados, pelo catálogo.
fn catalog_info(v: &Vault, ops: &[PricedOperation]) -> Result<(ByAsset, ByAsset), String> {
    let mut names = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    for t in ops
        .iter()
        .map(|p| &p.operation.asset)
        .collect::<BTreeSet<_>>()
    {
        if let Some(a) = v.get_asset(t).map_err(s)? {
            kinds.insert(t.clone(), a.kind);
            if !a.name.is_empty() {
                names.insert(t.clone(), a.name);
            }
        }
    }
    Ok((names, kinds))
}

pub fn portfolio(v: &Vault, as_of: NaiveDate) -> Result<PortfolioView, String> {
    let ops = priced(v)?;
    let table = prices_for(v, &ops)?;
    let fx = v.value_fx().map_err(s)?;
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
    let (names, kinds) = catalog_info(v, &ops)?;
    let dollar_accounts = dollar_accounts(v, &open, &fx, as_of)?;
    let last_update = [
        carteira_providers::yahoo::SOURCE,
        carteira_providers::cotahist::SOURCE,
        carteira_providers::cvm::SOURCE,
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
        names,
        kinds,
        dollar_accounts,
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
    pub names: BTreeMap<String, String>,
}

pub fn returns(v: &Vault, as_of: NaiveDate) -> Result<ReturnsView, String> {
    let ops = priced(v)?;
    let table = prices_for(v, &ops)?;
    let fx = v.value_fx().map_err(s)?;
    let trades = trades_of(&ops);
    let incomes = v.list_incomes().map_err(s)?;
    let scope = |scope: Scope, asset: Option<String>, currency: &str| {
        let months = monthly_returns_in(
            &trades,
            &incomes,
            &table,
            as_of,
            &scope,
            Conversion::ToBrl(&fx),
        );
        let years = yearly_returns(&months);
        let native = (currency != BRL).then(|| {
            let n_months =
                monthly_returns_in(&trades, &incomes, &table, as_of, &scope, Conversion::Native);
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
    let (names, _) = catalog_info(v, &ops)?;
    Ok(ReturnsView {
        as_of,
        names,
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
    pub names: BTreeMap<String, String>,
    /// FGTS, conta corrente e cartões benefício: patrimônio que não é
    /// investimento, fora da rentabilidade.
    pub other_balances: Vec<OtherBalance>,
    /// Saldos sem nenhum lançamento nos últimos meses fechados.
    pub missing_balances: Vec<MissingBalance>,
}

/// Saldo que ficou sem lançamento (nem saldo, nem depósito, nem resgate) em
/// algum dos três últimos meses fechados.
#[derive(Serialize)]
pub struct MissingBalance {
    pub id: String,
    pub name: String,
    pub account: String,
    pub kind: &'static str,
    /// Dia 1 de cada mês sem lançamento, do mais antigo ao mais recente.
    pub months: Vec<NaiveDate>,
}

/// Meses fechados (até três para trás) em que o saldo existia e não teve
/// nenhum lançamento. A renda fixa calculada pela taxa não precisa.
fn missing_balances(reports: &[SavingsReport], today: NaiveDate) -> Vec<MissingBalance> {
    let month_start = today.with_day(1).expect("dia 1 existe");
    reports
        .iter()
        .filter(|r| r.automatic.is_none() && r.savings.open_on(today))
        .filter_map(|r| {
            let months: Vec<NaiveDate> = (1..=3)
                .rev()
                .filter_map(|k| month_start.checked_sub_months(chrono::Months::new(k)))
                .filter(|m| r.savings.since.is_some_and(|s| s <= *m))
                .filter(|m| {
                    let end = m
                        .checked_add_months(chrono::Months::new(1))
                        .expect("mês seguinte");
                    !r.units_history.iter().any(|(d, _)| d >= m && *d < end)
                })
                .collect();
            (!months.is_empty()).then(|| MissingBalance {
                id: r.savings.id.clone(),
                name: r.savings.title().to_string(),
                account: r.savings.bank().to_string(),
                kind: r.savings.kind.label(),
                months,
            })
        })
        .collect()
}

#[derive(Serialize)]
pub struct OtherBalance {
    pub id: String,
    pub name: String,
    pub account: String,
    pub kind: &'static str,
    pub balance: Decimal,
    pub last_informed: Option<NaiveDate>,
}

fn other_balances(v: &Vault) -> Result<Vec<OtherBalance>, String> {
    Ok(savings_reports(v)?
        .into_iter()
        // Conta encerrada não pede saldo novo nem soma no patrimônio.
        .filter(|r| !r.savings.kind.is_investment() && r.savings.open_on(today()))
        .map(|r| OtherBalance {
            id: r.savings.id.clone(),
            name: r.savings.title().to_string(),
            account: r.savings.bank().to_string(),
            kind: r.savings.kind.label(),
            balance: r.balance,
            last_informed: r.last_informed,
        })
        .collect())
}

fn kind_label(kind: Option<&str>, currency: &str) -> &'static str {
    match (currency == BRL, kind) {
        (_, Some("CRIPTO")) => "Criptomoedas",
        (_, Some("FUNDO")) => "Fundos de investimento",
        (_, Some("RENDA_FIXA")) => "Renda fixa",
        (_, Some("TESOURO")) => "Tesouro Direto",
        (true, Some("ACAO")) => "Ações",
        (true, Some("FII")) => "FIIs e Fiagros",
        (true, Some("ETF")) => "ETFs",
        (true, Some("FI_INFRA")) => "FI-Infra",
        (true, Some("ETF_RF")) => "ETFs de renda fixa",
        (true, Some("BDR")) => "BDRs",
        (true, _) => "Outros (B3)",
        (false, Some("ETF")) => "ETFs no exterior",
        (false, _) => "Ações no exterior",
    }
}

/// Índices de comparação: os baixados e o dólar (PTAX).
fn index_series(
    v: &Vault,
    fx: &carteira_storage::FxTable,
) -> Result<Vec<carteira_core::benchmark::IndexSeries>, String> {
    use carteira_core::benchmark::{IndexKind, IndexSeries};
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
        kind: IndexKind::Level,
        source: carteira_providers::ptax::SOURCE.into(),
        points: fx.points(),
    });
    Ok(series)
}

pub fn dashboard(v: &Vault, as_of: NaiveDate) -> Result<DashboardView, String> {
    use carteira_core::benchmark::{chain, compare, drawdowns};

    let ops = priced(v)?;
    let table = prices_for(v, &ops)?;
    let fx = v.value_fx().map_err(s)?;
    let trades = trades_of(&ops);
    let incomes = v.list_incomes().map_err(s)?;

    let months = monthly_returns_in(
        &trades,
        &incomes,
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

    let series = index_series(v, fx.ptax())?;
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
                &incomes,
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

    let (names, _) = catalog_info(v, &ops)?;
    Ok(DashboardView {
        as_of,
        names,
        months: dash_months,
        cards,
        benchmarks,
        allocation: slices,
        contributions,
        positions,
        timeline: timeline(&ops),
        other_balances: other_balances(v)?,
        missing_balances: missing_balances(&savings_reports(v)?, today()),
    })
}

/// Dias de um mês no painel: cada ponto é o resultado do dia, tirado da
/// diferença entre o acumulado do mês até ele e até o dia útil anterior.
#[derive(Serialize)]
pub struct DaysView {
    pub days: Vec<DashMonth>,
    pub benchmarks: Vec<carteira_core::benchmark::BenchmarkView>,
    pub contributions: Vec<AssetMonths>,
}

/// Taxa do dia a partir dos acumulados: (1 + até hoje) / (1 + até ontem) − 1.
fn step(now: Option<Decimal>, before: Option<Decimal>) -> Option<Decimal> {
    let (now, before) = (now?, before?);
    let base = Decimal::ONE + before;
    if base.is_zero() {
        return None;
    }
    Some((Decimal::ONE + now) / base - Decimal::ONE)
}

fn diff(now: Option<Decimal>, before: Option<Decimal>) -> Option<Decimal> {
    Some(now? - before?)
}

pub fn dashboard_days(
    v: &Vault,
    year: i32,
    month: u32,
    as_of: NaiveDate,
) -> Result<DaysView, String> {
    use carteira_core::benchmark::{compare, compare_from, drawdowns, BenchmarkView};
    use chrono::{Datelike, Duration, Weekday};

    let empty = DaysView {
        days: vec![],
        benchmarks: vec![],
        contributions: vec![],
    };
    let ops = priced(v)?;
    let table = prices_for(v, &ops)?;
    let fx = v.value_fx().map_err(s)?;
    let trades = trades_of(&ops);
    let incomes = v.list_incomes().map_err(s)?;
    let months = monthly_returns_in(
        &trades,
        &incomes,
        &table,
        as_of,
        &Scope::Carteira,
        Conversion::ToBrl(&fx),
    );
    let Some(idx) = months
        .iter()
        .position(|m| m.year == year && m.month == month)
    else {
        return Ok(empty);
    };
    let target = &months[idx];
    let start = target.period_start;
    let last = target.period_end.min(as_of);

    // Dias úteis do período, sempre com o último.
    let mut dates = Vec::new();
    let mut d = start;
    while d <= last {
        if !matches!(d.weekday(), Weekday::Sat | Weekday::Sun) || d == last {
            dates.push(d);
        }
        d += Duration::days(1);
    }

    let windows: Vec<(NaiveDate, MonthResult)> = dates
        .iter()
        .filter_map(|&d| {
            window_result(
                &trades,
                &incomes,
                &table,
                start,
                d,
                &Scope::Carteira,
                Conversion::ToBrl(&fx),
            )
            .map(|w| (d, w))
        })
        .collect();
    if windows.is_empty() {
        return Ok(empty);
    }

    let invested_before: Decimal = months[..idx].iter().map(|m| m.net_flows).sum();
    let mut rates = Vec::with_capacity(windows.len());
    let mut profits = Vec::with_capacity(windows.len());
    let (mut cum, mut acc) = (Some(Decimal::ZERO), Some(Decimal::ZERO));
    for (_, w) in &windows {
        rates.push(step(w.rate, cum));
        profits.push(diff(w.profit, acc));
        cum = w.rate;
        acc = w.profit;
    }
    // Queda desde o pico contando os meses anteriores.
    let mut all: Vec<Option<Decimal>> = months[..idx].iter().map(|m| m.rate).collect();
    all.extend(rates.iter().copied());
    let dd = drawdowns(&all).split_off(idx);

    let days: Vec<DashMonth> = windows
        .iter()
        .enumerate()
        .map(|(i, (d, w))| DashMonth {
            year: d.year(),
            month: d.month(),
            period_start: *d,
            period_end: *d,
            value: w.end.total,
            invested: invested_before + w.net_flows,
            profit: profits[i],
            rate: rates[i],
            drawdown: dd[i],
            explanation: w.explanation.clone(),
        })
        .collect();

    let benchmarks: Vec<BenchmarkView> = index_series(v, fx.ptax())?
        .iter()
        .map(|sr| {
            let initial = if idx == 0 {
                Some(Decimal::ZERO)
            } else {
                compare(sr, &months[..idx])
                    .same_flows
                    .last()
                    .copied()
                    .flatten()
            };
            let mut out = BenchmarkView {
                id: sr.id.clone(),
                name: sr.name.clone(),
                kind: sr.kind,
                source: sr.source.clone(),
                rates: vec![],
                same_flows: vec![],
                explanations: vec![],
            };
            let mut before = Some(Decimal::ZERO);
            for (_, w) in &windows {
                let one = compare_from(sr, std::slice::from_ref(w), initial);
                let now = one.rates[0];
                out.rates.push(step(now, before));
                out.same_flows.push(one.same_flows[0]);
                out.explanations.push(one.explanations[0].clone());
                before = now;
            }
            out
        })
        .collect();

    let tickers: BTreeSet<String> = trades.iter().map(|t| t.asset.clone()).collect();
    let contributions = tickers
        .into_iter()
        .map(|t| {
            let scope = Scope::Ativo(t.clone());
            let mut before: Option<Decimal> = None;
            let profits = windows
                .iter()
                .map(|(d, _)| {
                    let now = window_result(
                        &trades,
                        &incomes,
                        &table,
                        start,
                        *d,
                        &scope,
                        Conversion::ToBrl(&fx),
                    )
                    .and_then(|w| w.profit);
                    let p = match (now, before) {
                        (Some(n), Some(b)) => Some(n - b),
                        (Some(n), None) => Some(n),
                        _ => None,
                    };
                    if now.is_some() {
                        before = now;
                    }
                    p
                })
                .collect();
            AssetMonths { asset: t, profits }
        })
        .collect();

    Ok(DaysView {
        days,
        benchmarks,
        contributions,
    })
}

// ---------- impostos ----------

#[derive(Serialize)]
pub struct TaxMonthView {
    #[serde(flatten)]
    pub tax: carteira_core::tax::MonthTax,
    /// "PAGO", "PENDENTE", "ATRASADO", "EM_ANDAMENTO" (mês corrente) ou "SEM_DARF".
    pub status: &'static str,
    pub paid: Option<DarfPayment>,
}

#[derive(Serialize)]
pub struct TaxView {
    pub months: Vec<TaxMonthView>,
    pub annual: carteira_core::tax::AnnualReport,
    /// Nome de cada ativo, para a lista de bens.
    pub names: BTreeMap<String, String>,
    pub warnings: Vec<String>,
    pub darf_code: &'static str,
    /// Saldos em 31/12 (cofrinhos, FGTS, conta): Bens e Direitos.
    pub savings_year_end: Vec<SavingsYearEnd>,
}

#[derive(Serialize)]
pub struct SavingsYearEnd {
    pub year: i32,
    pub name: String,
    pub account: String,
    pub kind: &'static str,
    pub value: Decimal,
}

pub fn taxes(v: &Vault, as_of: NaiveDate) -> Result<TaxView, String> {
    use carteira_core::tax;
    let ops = priced(v)?;
    let (names, kinds) = catalog_info(v, &ops)?;
    let kind_of = |t: &str| kinds.get(t).cloned();
    let b3 = tax::b3_months(&ops, &kind_of, as_of);
    let fx = v.fx_table().map_err(s)?;
    let annual = tax::annual(&ops, &kind_of, &fx, as_of);
    let paid: BTreeMap<String, DarfPayment> = v
        .list_darf_payments()
        .map_err(s)?
        .into_iter()
        .map(|p| (p.month.clone(), p))
        .collect();
    let current = format!("{:04}-{:02}", as_of.year(), as_of.month());
    let months = b3
        .months
        .into_iter()
        .rev()
        .map(|m| {
            let paid = paid.get(&m.month).cloned();
            let status = if paid.is_some() {
                "PAGO"
            } else if m.month == current {
                "EM_ANDAMENTO"
            } else if m.darf.is_zero() {
                "SEM_DARF"
            } else if as_of > m.due_date {
                "ATRASADO"
            } else {
                "PENDENTE"
            };
            TaxMonthView {
                tax: m,
                status,
                paid,
            }
        })
        .collect();
    let mut warnings = b3.warnings;
    warnings.extend(annual.warnings.iter().cloned());
    let unknown: Vec<&String> = ops
        .iter()
        .filter(|p| {
            p.operation.currency == BRL
                && !carteira_core::crypto::is_crypto(&p.operation.asset)
                && !carteira_core::fund::bank_withholds(&p.operation.asset)
        })
        .map(|p| &p.operation.asset)
        .filter(|t| kinds.get(*t).map_or(true, |k| k == "OUTRO"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !unknown.is_empty() {
        warnings.push(format!(
            "Sem classificação no catálogo da B3 (tratados como operação comum, sem isenção): {}.",
            unknown
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let mut savings_year_end: Vec<SavingsYearEnd> = savings_reports(v)?
        .into_iter()
        .flat_map(|r| {
            r.year_end
                .iter()
                .map(|(year, value)| SavingsYearEnd {
                    year: *year,
                    name: r.savings.title().to_string(),
                    account: r.savings.bank().to_string(),
                    kind: r.savings.kind.label(),
                    value: *value,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    // Imóveis e veículos: custo em 31/12 (compra + obras até a data).
    let works = v.list_improvements().map_err(s)?;
    for item in v.list_owned().map_err(s)? {
        for year in item.bought_on.year()..=as_of.year() {
            let end = NaiveDate::from_ymd_opt(year, 12, 31).expect("31/12");
            if let Some(value) = item.cost_on(&works, end) {
                savings_year_end.push(SavingsYearEnd {
                    year,
                    name: item.name.clone(),
                    account: String::new(),
                    kind: item.kind.label(),
                    value,
                });
            }
        }
    }
    Ok(TaxView {
        savings_year_end,
        months,
        annual,
        names,
        warnings,
        darf_code: tax::DARF_CODE,
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
            carteira_core::model::Side::Compra | carteira_core::model::Side::Ajuste => op.quantity,
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
        if entry.0 <= Decimal::ZERO || carteira_core::model::is_dust(entry.0) {
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
    use chrono::Datelike;
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
            ir: None,
            iof: None,
        }
    }

    #[test]
    fn impostos_com_darf_pendente_e_pago() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-1",
            "0.7.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        // TEST3 fora do catálogo: operação comum sem isenção, com aviso.
        v.create_operation(op("TEST3", d(1, 5), Side::Compra, dec!(100)))
            .unwrap();
        let mut sale = op("TEST3", d(2, 5), Side::Venda, dec!(100));
        sale.price = dec!(20);
        v.create_operation(sale).unwrap();
        let view = taxes(&v, d(3, 10)).unwrap();
        let feb = view
            .months
            .iter()
            .find(|m| m.tax.month == "2026-02")
            .unwrap();
        // Lucro 1.000 × 15% = 150; dedo-duro de R$ 0,10 fica dispensado.
        assert_eq!((feb.tax.darf, feb.status), (dec!(150), "PENDENTE"));
        assert!(view.warnings.iter().any(|w| w.contains("TEST3")));
        assert_eq!(taxes(&v, d(4, 1)).unwrap().months[0].status, "ATRASADO");
        v.mark_darf_paid(&DarfPayment {
            month: "2026-02".into(),
            paid_on: d(3, 20),
            amount: dec!(150),
        })
        .unwrap();
        let view = taxes(&v, d(4, 1)).unwrap();
        assert_eq!(view.months[0].status, "PAGO");
        assert_eq!(view.annual.positions[&2026].len(), 0);
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

        // Dias de abril: somados e encadeados, batem com o mês.
        let days = dashboard_days(&v, 2026, 4, d(4, 30)).unwrap();
        assert_eq!(days.days.first().unwrap().period_end, d(4, 1));
        assert_eq!(days.days.last().unwrap().period_end, d(4, 30));
        assert!(days
            .days
            .iter()
            .all(|x| x.period_end.weekday().number_from_monday() <= 5));
        let total: Decimal = days.days.iter().map(|x| x.profit.unwrap()).sum();
        assert_eq!(Some(total), view.months[1].profit);
        let rates: Vec<_> = days.days.iter().map(|x| x.rate).collect();
        let chained = carteira_core::benchmark::chain(&rates).unwrap();
        assert!((chained - view.months[1].rate.unwrap()).abs() < dec!(0.0000001));
        assert_eq!(days.days.last().unwrap().invested, dec!(1000));
        let cdi_days = days.benchmarks.iter().find(|b| b.id == "CDI").unwrap();
        let last = cdi_days.same_flows.last().unwrap().unwrap();
        assert!((last - cdi.same_flows[1].unwrap()).abs() < dec!(0.01));
        assert!(dashboard_days(&v, 2026, 7, d(4, 30))
            .unwrap()
            .days
            .is_empty());
    }
}

// ---------- proventos ----------

/// Provento com os valores em reais e a quantidade que havia na data com.
#[derive(Debug, Serialize)]
pub struct IncomeRow {
    #[serde(flatten)]
    pub priced: carteira_core::income::PricedIncome,
    /// Quantidade do ativo na data com (ou no pagamento, sem data com).
    pub quantity: Option<Decimal>,
    /// Valor bruto por cota, quando há quantidade.
    pub per_share: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct IncomesView {
    /// Do pagamento mais recente para o mais antigo.
    pub items: Vec<IncomeRow>,
    pub names: ByAsset,
    pub kinds: ByAsset,
}

/// Quantidade do ativo em todas as contas no fim do dia.
fn held_on(ops: &[PricedOperation], asset: &str, day: NaiveDate) -> Decimal {
    ops.iter()
        .map(|p| &p.operation)
        .filter(|o| o.asset == asset && o.date <= day)
        .map(|o| match o.side {
            carteira_core::model::Side::Compra | carteira_core::model::Side::Ajuste => o.quantity,
            carteira_core::model::Side::Venda => -o.quantity,
            _ => -o.residue.unwrap_or_default(),
        })
        .sum()
}

pub fn incomes(v: &Vault) -> Result<IncomesView, String> {
    let ops = priced(v)?;
    let fx = v.fx_table().map_err(s)?;
    let mut names = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    let mut items = Vec::new();
    for i in v.list_incomes().map_err(s)? {
        if !names.contains_key(&i.asset) && !kinds.contains_key(&i.asset) {
            if let Some(a) = v.get_asset(&i.asset).map_err(s)? {
                kinds.insert(i.asset.clone(), a.kind);
                if !a.name.is_empty() {
                    names.insert(i.asset.clone(), a.name);
                }
            }
        }
        let held = held_on(&ops, &i.asset, i.ex_date.unwrap_or(i.pay_date));
        let quantity = (held > Decimal::ZERO).then_some(held);
        let per_share = quantity.map(|q| (i.gross / q).round_dp(8).normalize());
        items.push(IncomeRow {
            priced: carteira_core::income::price_income(i, &fx),
            quantity,
            per_share,
        });
    }
    items.reverse();
    Ok(IncomesView {
        items,
        names,
        kinds,
    })
}

/// Evento corporativo com o efeito em cada conta.
#[derive(Debug, Serialize)]
pub struct EventRow {
    pub event: carteira_core::corporate::CorporateEvent,
    pub label: String,
    pub ex_date: NaiveDate,
    pub effects: Vec<EffectRow>,
    pub before: Decimal,
    pub after: Decimal,
    /// Bonificação: valor isento para a declaração, em reais (exterior pela
    /// PTAX da data ex). Vazio se faltar a PTAX.
    pub exempt_brl: Option<Decimal>,
    /// Preço médio do ativo antes e depois do evento (todas as contas).
    pub average_before: Option<Decimal>,
    pub average_after: Option<Decimal>,
    pub currency: String,
}

#[derive(Debug, Serialize)]
pub struct EffectRow {
    pub account: String,
    pub before: Decimal,
    pub after: Decimal,
    pub delta: Decimal,
    pub cost_added: Decimal,
}

#[derive(Debug, Serialize)]
pub struct EventsView {
    /// Do mais recente para o mais antigo.
    pub items: Vec<EventRow>,
    /// Ativos que já passaram pela carteira (fundos e cripto ficam de fora).
    pub assets: Vec<String>,
    pub names: ByAsset,
}

/// Preço médio do ativo, somando as contas, no fim do dia.
fn average_on(
    ops: &[Operation],
    asset: &str,
    day: NaiveDate,
    table: &PriceTable,
) -> Result<Option<Decimal>, String> {
    let mine: Vec<Operation> = ops.iter().filter(|o| o.asset == asset).cloned().collect();
    let priced = price_operations(&default_rules(), &mine).map_err(s)?.0;
    Ok(
        carteira_core::portfolio::positions(&trades_of(&priced), table, day)
            .into_iter()
            .find(|p| p.asset == asset && !carteira_core::model::is_dust(p.quantity))
            .map(|p| p.average_price),
    )
}

fn event_rows(
    v: &Vault,
    ops: &[Operation],
    events: &[carteira_core::corporate::CorporateEvent],
    only: Option<&str>,
) -> Result<Vec<EventRow>, String> {
    use carteira_core::corporate::{apply, label, EventKind};
    let (effects, adjustments) = apply(ops, events);
    let mut all = ops.to_vec();
    all.extend(adjustments);
    let fx = v.fx_table().map_err(s)?;
    let mut rows = Vec::new();
    for ev in events.iter().filter(|e| only.is_none_or(|id| e.id == id)) {
        let mine: Vec<_> = effects.iter().filter(|f| f.event_id == ev.id).collect();
        let currency = mine
            .first()
            .map(|f| f.currency.clone())
            .unwrap_or_else(|| BRL.into());
        let cost: Decimal = mine.iter().map(|f| f.cost_added).sum();
        let exempt_brl = if ev.kind != EventKind::Bonificacao {
            Some(Decimal::ZERO)
        } else if currency == BRL {
            Some(cost)
        } else {
            fx.usd_brl_on_or_before(ev.ex_date())
                .map(|r| (cost * r.rate).round_dp(2))
        };
        let table = v.price_table(std::slice::from_ref(&ev.asset)).map_err(s)?;
        // Antes: sem este evento; depois: com ele (e com os anteriores).
        let without: Vec<Operation> = all
            .iter()
            .filter(|o| !o.id.starts_with(&format!("{}:", ev.id)))
            .cloned()
            .collect();
        rows.push(EventRow {
            label: label(ev),
            ex_date: ev.ex_date(),
            before: mine.iter().map(|f| f.before).sum(),
            after: mine.iter().map(|f| f.after).sum(),
            effects: mine
                .iter()
                .map(|f| EffectRow {
                    account: f.account.clone(),
                    before: f.before,
                    after: f.after,
                    delta: f.delta(),
                    cost_added: f.cost_added,
                })
                .collect(),
            exempt_brl,
            average_before: average_on(&without, &ev.asset, ev.data_com, &table)?,
            average_after: average_on(&all, &ev.asset, ev.ex_date(), &table)?,
            currency,
            event: ev.clone(),
        });
    }
    Ok(rows)
}

pub fn corporate_events(v: &Vault) -> Result<EventsView, String> {
    let ops = v.list_operations().map_err(s)?;
    let events = v.list_corporate_events().map_err(s)?;
    let mut items = event_rows(v, &ops, &events, None)?;
    items.reverse();
    let assets: Vec<String> = ops
        .iter()
        .map(|o| o.asset.clone())
        .filter(|a| !carteira_core::fund::bank_withholds(a) && !carteira_core::crypto::is_crypto(a))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut names = BTreeMap::new();
    for a in &assets {
        if let Some(info) = v.get_asset(a).map_err(s)? {
            if !info.name.is_empty() {
                names.insert(a.clone(), info.name);
            }
        }
    }
    Ok(EventsView {
        items,
        assets,
        names,
    })
}

/// Efeito de um evento ainda não salvo (ou de uma alteração), sem gravar.
pub fn preview_event(
    v: &Vault,
    mut draft: carteira_core::corporate::CorporateEvent,
) -> Result<EventRow, String> {
    draft.asset = normalize_ticker(&draft.asset);
    if draft.id.is_empty() {
        draft.id = "ev-rascunho".into();
    }
    draft.validate()?;
    let ops = v.list_operations().map_err(s)?;
    let mut events: Vec<_> = v
        .list_corporate_events()
        .map_err(s)?
        .into_iter()
        .filter(|e| e.id != draft.id)
        .collect();
    events.push(draft.clone());
    event_rows(v, &ops, &events, Some(&draft.id))?
        .pop()
        .ok_or_else(|| "evento não encontrado".into())
}

#[cfg(test)]
mod event_tests {
    use super::*;
    use carteira_core::corporate::{CorporateEvent, EventKind};
    use carteira_core::model::Side;
    use carteira_storage::KdfParams;
    use rust_decimal_macros::dec;

    #[test]
    fn bonificacao_muda_quantidade_preco_medio_e_impostos() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.2.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
        let op = |date, side, q| Operation {
            id: String::new(),
            account: "Corretora X".into(),
            asset: "TEST3".into(),
            date,
            side,
            quantity: q,
            price: dec!(20),
            manual_fees: Some(dec!(0)),
            note: None,
            currency: "BRL".into(),
            from_account: None,
            residue: None,
            ir: None,
            iof: None,
        };
        v.create_operation(op(d(1, 5), Side::Compra, dec!(100)))
            .unwrap();
        // Venda no dia ex: não pode virar day trade com as ações bonificadas.
        v.create_operation(op(d(3, 2), Side::Venda, dec!(10)))
            .unwrap();
        let ev = CorporateEvent {
            id: String::new(),
            asset: "TEST3".into(),
            kind: EventKind::Bonificacao,
            data_com: d(3, 1),
            proportion: dec!(10),
            cost_per_share: dec!(9),
            note: None,
        };
        let preview = preview_event(&v, ev.clone()).unwrap();
        assert_eq!(preview.after, dec!(110));
        assert_eq!(preview.exempt_brl, Some(dec!(90)));
        assert_eq!(preview.average_before, Some(dec!(20)));
        // (2.000 + 90) / 110
        assert_eq!(preview.average_after.unwrap().round_dp(4), dec!(19.0000));
        v.create_event(ev).unwrap();

        let view = corporate_events(&v).unwrap();
        assert_eq!(view.items.len(), 1);
        assert_eq!(view.assets, vec!["TEST3".to_string()]);
        // Lançamentos não mostram o ajuste; os cálculos usam.
        assert_eq!(list_operations(&v).unwrap().len(), 2);
        let ops = priced(&v).unwrap();
        assert_eq!(held_on(&ops, "TEST3", d(3, 2)), dec!(100));
        let kind = |_: &str| Some("ACAO".to_string());
        let report = carteira_core::tax::b3_months(&ops, &kind, d(12, 31));
        let march = report.months.iter().find(|m| m.month == "2026-03").unwrap();
        // Venda comum (isenta) com o preço médio já ajustado: 10 × (20 − 19).
        assert_eq!(march.exempt_profit, dec!(10));
    }
}

/// Tela de saldos e reservas: cada saldo com o que o app estima hoje e os
/// movimentos lançados.
#[derive(Serialize)]
pub struct SavingsItem {
    pub savings: carteira_core::savings::Savings,
    pub kind_label: &'static str,
    pub ticker: String,
    pub balance: Decimal,
    pub balance_net: Decimal,
    pub invested: Decimal,
    /// Rendimento estimado ainda no saldo (saldo − dinheiro depositado).
    pub earned: Decimal,
    pub last_informed: Option<NaiveDate>,
    /// Renda fixa calculada pela taxa ("102% do CDI").
    pub automatic: Option<String>,
    pub checks: Vec<carteira_core::savings::BalanceCheck>,
    pub redemptions: Vec<carteira_core::savings::Redemption>,
    /// Do mais recente para o mais antigo.
    pub moves: Vec<SavingsMove>,
}

#[derive(Serialize)]
pub struct SavingsView {
    pub items: Vec<SavingsItem>,
    pub total_investments: Decimal,
    pub total_others: Decimal,
    /// Último dia com CDI baixado (a estimativa dos cofrinhos vai até ele).
    pub cdi_until: Option<NaiveDate>,
}

pub fn savings_view(v: &Vault) -> Result<SavingsView, String> {
    let mut moves = v.list_savings_moves().map_err(s)?;
    moves.reverse();
    let items: Vec<SavingsItem> = savings_reports(v)?
        .into_iter()
        .map(|r| SavingsItem {
            kind_label: r.savings.kind.label(),
            ticker: r.ticker.clone(),
            balance: r.balance,
            balance_net: r.balance_net,
            invested: r.invested,
            earned: r.balance - r.invested,
            last_informed: r.last_informed,
            automatic: r.automatic.clone(),
            moves: moves
                .iter()
                .filter(|m| m.savings_id == r.savings.id)
                .cloned()
                .collect(),
            checks: r.checks,
            redemptions: r.redemptions,
            savings: r.savings,
        })
        .collect();
    let total = |inv: bool| {
        items
            .iter()
            .filter(|i| i.savings.kind.is_investment() == inv)
            .map(|i| i.balance)
            .sum()
    };
    Ok(SavingsView {
        total_investments: total(true),
        total_others: total(false),
        cdi_until: v
            .index_points("CDI")
            .map_err(s)?
            .0
            .keys()
            .next_back()
            .copied(),
        items,
    })
}

#[cfg(test)]
mod savings_tests {
    use super::*;
    use carteira_core::savings::{MoveKind, Savings, SavingsKind};
    use carteira_storage::KdfParams;
    use rust_decimal_macros::dec;

    #[test]
    fn cofrinho_entra_na_carteira_e_fgts_so_no_patrimonio() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.2.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        let mk = |kind| Savings {
            id: String::new(),
            kind,
            account: "Banco X".into(),
            name: "Reserva".into(),
            rate_pct: dec!(100),
            tax_free: false,
            note: None,
            since: None,
            until: None,
            fixed: None,
        };
        let c = v.create_savings(mk(SavingsKind::Cofrinho)).unwrap();
        let f = v.create_savings(mk(SavingsKind::Fgts)).unwrap();
        let d = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        for (id, amount) in [(&c.id, dec!(1000)), (&f.id, dec!(5000))] {
            v.create_savings_move(SavingsMove {
                id: String::new(),
                savings_id: id.clone(),
                date: d,
                kind: MoveKind::Deposito,
                amount,
                ir: None,
                iof: None,
                note: None,
            })
            .unwrap();
        }
        v.create_savings_move(SavingsMove {
            id: String::new(),
            savings_id: c.id.clone(),
            date: NaiveDate::from_ymd_opt(2026, 2, 5).unwrap(),
            kind: MoveKind::Resgate,
            amount: dec!(100),
            ir: None,
            iof: None,
            note: None,
        })
        .unwrap();
        let p = portfolio(&v, today()).unwrap();
        assert_eq!(p.positions.len(), 1, "só o cofrinho é investimento");
        assert_eq!(p.positions[0].asset, carteira_core::savings::ticker(&c.id));
        assert_eq!(p.market_value, Some(dec!(900)), "cofrinho sem estimativa");
        assert_eq!(
            p.kinds.get(&p.positions[0].asset).map(String::as_str),
            Some("RENDA_FIXA")
        );
        let t = taxes(&v, today()).unwrap();
        assert!(
            t.months.iter().all(|m| m.tax.sales.is_empty()),
            "cofrinho não é venda na B3"
        );
        let dash = dashboard(&v, today()).unwrap();
        assert_eq!(dash.other_balances.len(), 1);
        assert_eq!(dash.allocation[0].label, "Renda fixa");
        let sv = savings_view(&v).unwrap();
        assert_eq!(sv.total_investments, dec!(900));
        assert_eq!(sv.total_others, dec!(5000), "FGTS sem estimativa");
    }
}

// ---------- proventos pelo histórico do mercado ----------

/// Ativo que já passou pela carteira e pode pagar proventos.
#[derive(Debug, Clone)]
pub struct MarketAsset {
    pub ticker: String,
    pub currency: String,
    pub kind: String,
    pub first: NaiveDate,
}

/// O que é preciso ler do cofre antes de ir à internet.
pub struct MarketInput {
    pub assets: Vec<MarketAsset>,
    pub ops: Vec<Operation>,
    pub incomes: Vec<carteira_core::income::Income>,
}

/// Tipos que pagam proventos por cota (fundos, cripto e renda fixa ficam de fora).
const PAYERS: &[&str] = &["ACAO", "FII", "ETF", "BDR", "FI_INFRA"];

/// Imposto retido nos EUA sobre dividendos de quem mora no Brasil.
pub const US_WITHHOLDING: Decimal = Decimal::from_parts(30, 0, 0, false, 2);

pub fn market_input(v: &Vault) -> Result<MarketInput, String> {
    let ops = engine_operations(v)?;
    let mut first: BTreeMap<String, (NaiveDate, String)> = BTreeMap::new();
    for o in &ops {
        let e = first
            .entry(o.asset.clone())
            .or_insert((o.date, o.currency.clone()));
        if o.date < e.0 {
            e.0 = o.date;
        }
    }
    let mut assets = Vec::new();
    for (ticker, (date, currency)) in first {
        let Some(a) = v.get_asset(&ticker).map_err(s)? else {
            continue;
        };
        if PAYERS.contains(&a.kind.as_str()) {
            assets.push(MarketAsset {
                ticker,
                currency,
                kind: a.kind,
                first: date,
            });
        }
    }
    Ok(MarketInput {
        assets,
        ops,
        incomes: v.list_incomes().map_err(s)?,
    })
}

/// Provento que o histórico do mercado indica e ainda não foi lançado.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SuggestedIncome {
    pub account: String,
    pub asset: String,
    pub kind: carteira_core::income::IncomeKind,
    /// Data ex informada pelo Yahoo (quem tinha a cota no dia anterior recebe).
    pub ex_date: NaiveDate,
    pub per_share: Decimal,
    pub quantity: Decimal,
    pub gross: Decimal,
    pub withheld: Decimal,
    pub currency: String,
}

#[derive(Debug, Default, Serialize)]
pub struct MarketIncomes {
    /// Por ativo: (data ex, valor por cota ajustado aos desdobramentos), dos
    /// últimos 12 meses. Vale para a quantidade de hoje.
    pub history: BTreeMap<String, Vec<(NaiveDate, Decimal)>>,
    /// Do mais recente para o mais antigo.
    pub suggestions: Vec<SuggestedIncome>,
    /// Ativos que o Yahoo não respondeu.
    pub unavailable: Vec<String>,
}

/// Quão perto um provento já lançado está desta data ex, em dias; `None`
/// quando está longe demais para ser o mesmo pagamento. A data com, quando
/// informada, é a mais confiável; senão vale a data do pagamento, que vem
/// depois da data ex (dias no exterior, até meses na B3).
fn distance_to(inc: &carteira_core::income::Income, ex: NaiveDate) -> Option<i64> {
    let by_ex = inc
        .ex_date
        .map(|e| (e - ex).num_days())
        .filter(|d| d.abs() <= 10)
        .map(|d| d.abs());
    let by_pay = Some((inc.pay_date - ex).num_days())
        .filter(|d| (-5..=120).contains(d))
        .map(|d| d.abs());
    match (by_ex, by_pay) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Cotas de cada conta no fim do dia `day`.
fn held_by_account(ops: &[Operation], asset: &str, day: NaiveDate) -> BTreeMap<String, Decimal> {
    use carteira_core::model::Side;
    let mut out: BTreeMap<String, Decimal> = BTreeMap::new();
    for o in ops.iter().filter(|o| o.asset == asset && o.date <= day) {
        let residue = o.residue.unwrap_or_default();
        match o.side {
            Side::Compra | Side::Ajuste => *out.entry(o.account.clone()).or_default() += o.quantity,
            Side::Venda => *out.entry(o.account.clone()).or_default() -= o.quantity,
            Side::Transferencia => match &o.from_account {
                Some(from) => {
                    *out.entry(from.clone()).or_default() -= o.quantity + residue;
                    *out.entry(o.account.clone()).or_default() += o.quantity;
                }
                None => *out.entry(o.account.clone()).or_default() -= residue,
            },
        }
    }
    out
}

/// Junta o histórico de cada ativo com as cotas de cada conta em cada data ex.
/// Um provento já lançado (mesmo ativo e conta, data com perto da data ex ou
/// pagamento até 100 dias depois) cobre uma sugestão.
pub fn market_incomes(
    input: &MarketInput,
    fetched: &BTreeMap<String, Option<carteira_providers::yahoo::Dividends>>,
    today: NaiveDate,
) -> MarketIncomes {
    use carteira_core::income::IncomeKind;
    let mut out = MarketIncomes::default();
    let year_ago = today - chrono::Duration::days(365);
    let mut suggestions = Vec::new();
    for a in &input.assets {
        let Some(found) = fetched.get(&a.ticker) else {
            continue;
        };
        let Some(d) = found else {
            out.unavailable.push(a.ticker.clone());
            continue;
        };
        out.history.insert(
            a.ticker.clone(),
            d.dividends
                .iter()
                .filter(|(day, _)| *day > year_ago && *day <= today)
                .cloned()
                .collect(),
        );
        // Primeiro, o que cada conta tinha a receber em cada data ex.
        let mut due = Vec::new();
        for (ex, per_share) in d.unadjusted() {
            if ex < a.first || ex > today {
                continue;
            }
            let Some(before) = ex.pred_opt() else {
                continue;
            };
            for (account, quantity) in held_by_account(&input.ops, &a.ticker, before) {
                if quantity <= Decimal::ZERO {
                    continue;
                }
                let gross = (per_share * quantity).round_dp(2);
                if gross < Decimal::new(1, 2) {
                    continue;
                }
                due.push((ex, per_share, account, quantity, gross));
            }
        }
        // Depois, casa cada provento já lançado com a data ex mais perto dele,
        // começando pelos pares mais próximos de todos. Assim um pagamento de
        // março não é tomado pela data ex de dezembro e não sobra um
        // provento reconhecido como se faltasse.
        let mut pairs = Vec::new();
        for (j, (ex, _, account, _, _)) in due.iter().enumerate() {
            for (i, inc) in input.incomes.iter().enumerate() {
                if inc.asset == a.ticker && inc.account == *account {
                    if let Some(dist) = distance_to(inc, *ex) {
                        pairs.push((dist, j, i));
                    }
                }
            }
        }
        pairs.sort();
        let mut covered = vec![false; due.len()];
        let mut used = vec![false; input.incomes.len()];
        for (_, j, i) in pairs {
            if !covered[j] && !used[i] {
                covered[j] = true;
                used[i] = true;
            }
        }
        for ((ex, per_share, account, quantity, gross), done) in due.into_iter().zip(covered) {
            if done {
                continue;
            }
            let usd = a.currency == "USD";
            suggestions.push(SuggestedIncome {
                account,
                asset: a.ticker.clone(),
                kind: if a.kind == "FII" || a.kind == "FI_INFRA" {
                    IncomeKind::Rendimento
                } else {
                    IncomeKind::Dividendo
                },
                ex_date: ex,
                per_share,
                quantity,
                gross,
                withheld: if usd {
                    (gross * US_WITHHOLDING).round_dp(2)
                } else {
                    Decimal::ZERO
                },
                currency: a.currency.clone(),
            });
        }
    }
    suggestions.sort_by(|a, b| b.ex_date.cmp(&a.ex_date).then(a.asset.cmp(&b.asset)));
    out.suggestions = suggestions;
    out
}

#[cfg(test)]
mod market_tests {
    use super::*;
    use carteira_core::income::{Income, IncomeKind};
    use carteira_core::model::Side;
    use carteira_providers::yahoo::Dividends;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn op(account: &str, date: NaiveDate, side: Side, q: Decimal, currency: &str) -> Operation {
        Operation {
            id: format!("{account}{date}"),
            account: account.into(),
            asset: if currency == "USD" {
                "TSTX".into()
            } else {
                "TEST3".into()
            },
            date,
            side,
            quantity: q,
            price: dec!(10),
            manual_fees: None,
            note: None,
            currency: currency.into(),
            from_account: None,
            residue: None,
            ir: None,
            iof: None,
        }
    }

    #[test]
    fn sugere_o_que_falta_lancar_com_as_cotas_de_cada_conta() {
        let ops = vec![
            op("A", d(2025, 1, 10), Side::Compra, dec!(100), "BRL"),
            op("B", d(2025, 5, 10), Side::Compra, dec!(50), "BRL"),
            op("A", d(2025, 8, 1), Side::Venda, dec!(100), "BRL"),
            op("N", d(2025, 2, 1), Side::Compra, dec!(10), "USD"),
        ];
        let input = MarketInput {
            assets: vec![
                MarketAsset {
                    ticker: "TEST3".into(),
                    currency: "BRL".into(),
                    kind: "ACAO".into(),
                    first: d(2025, 1, 10),
                },
                MarketAsset {
                    ticker: "TSTX".into(),
                    currency: "USD".into(),
                    kind: "ETF".into(),
                    first: d(2025, 2, 1),
                },
                MarketAsset {
                    ticker: "SEMNET3".into(),
                    currency: "BRL".into(),
                    kind: "ACAO".into(),
                    first: d(2025, 2, 1),
                },
            ],
            ops,
            incomes: vec![Income {
                id: "i1".into(),
                account: "A".into(),
                asset: "TEST3".into(),
                kind: IncomeKind::Dividendo,
                pay_date: d(2025, 3, 25),
                ex_date: None,
                gross: dec!(50),
                withheld: dec!(0),
                currency: "BRL".into(),
                note: None,
            }],
        };
        let mut fetched = BTreeMap::new();
        fetched.insert(
            "TEST3".to_string(),
            Some(Dividends {
                // Valores ajustados por um desdobramento 1:2 em 2025-09-01.
                dividends: vec![
                    (d(2024, 12, 1), dec!(1)),
                    (d(2025, 3, 3), dec!(0.25)),
                    (d(2025, 6, 2), dec!(0.5)),
                    (d(2025, 10, 1), dec!(0.1)),
                ],
                splits: vec![(d(2025, 9, 1), dec!(2))],
            }),
        );
        fetched.insert(
            "TSTX".to_string(),
            Some(Dividends {
                dividends: vec![(d(2025, 6, 20), dec!(1.5))],
                splits: vec![],
            }),
        );
        fetched.insert("SEMNET3".to_string(), None);
        let r = market_incomes(&input, &fetched, d(2025, 10, 20));
        let got: Vec<_> = r
            .suggestions
            .iter()
            .map(|s| {
                (
                    s.account.as_str(),
                    s.asset.as_str(),
                    s.ex_date,
                    s.per_share,
                    s.quantity,
                    s.gross,
                    s.withheld,
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                // Junho vem desfeito do desdobramento posterior: 0,5 × 2 = 1 por cota.
                (
                    "B",
                    "TEST3",
                    d(2025, 10, 1),
                    dec!(0.1),
                    dec!(50),
                    dec!(5.00),
                    dec!(0)
                ),
                (
                    "N",
                    "TSTX",
                    d(2025, 6, 20),
                    dec!(1.5),
                    dec!(10),
                    dec!(15.00),
                    dec!(4.50)
                ),
                (
                    "A",
                    "TEST3",
                    d(2025, 6, 2),
                    dec!(1),
                    dec!(100),
                    dec!(100.00),
                    dec!(0)
                ),
                (
                    "B",
                    "TEST3",
                    d(2025, 6, 2),
                    dec!(1),
                    dec!(50),
                    dec!(50.00),
                    dec!(0)
                ),
            ]
        );
        // Março já foi lançado (pagamento 22 dias depois da data ex); dez/24 é antes da 1ª compra.
        assert_eq!(r.unavailable, vec!["SEMNET3".to_string()]);
        assert_eq!(r.history["TEST3"].len(), 4);
    }
}

#[cfg(test)]
mod caso_etf_trimestral {
    use super::*;
    use carteira_core::income::{Income, IncomeKind};
    use carteira_core::model::Side;
    use carteira_providers::yahoo::Dividends;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn compra(asset: &str, q: Decimal) -> Operation {
        Operation {
            id: format!("c{asset}"),
            account: "Nomad".into(),
            asset: asset.into(),
            date: d(2024, 10, 28),
            side: Side::Compra,
            quantity: q,
            price: dec!(100),
            manual_fees: None,
            note: None,
            currency: "USD".into(),
            from_account: None,
            residue: None,
            ir: None,
            iof: None,
        }
    }

    fn lancado(asset: &str, pay: NaiveDate, ex: Option<NaiveDate>) -> Income {
        Income {
            id: format!("i{asset}{pay}"),
            account: "Nomad".into(),
            asset: asset.into(),
            kind: IncomeKind::Dividendo,
            pay_date: pay,
            ex_date: ex,
            gross: dec!(70.00),
            withheld: dec!(21.00),
            currency: "USD".into(),
            note: None,
        }
    }

    /// Mesmo fora de ordem, cada lançamento casa com a data ex mais próxima.
    #[test]
    fn nao_sugere_o_que_ja_foi_lancado() {
        let input = MarketInput {
            assets: vec![MarketAsset {
                ticker: "VOO".into(),
                currency: "USD".into(),
                kind: "ETF".into(),
                first: d(2024, 10, 28),
            }],
            ops: vec![compra("VOO", dec!(40))],
            // Fora de ordem de propósito, e um deles sem data com.
            incomes: vec![
                lancado("VOO", d(2026, 6, 30), Some(d(2026, 6, 25))),
                lancado("VOO", d(2026, 9, 30), None),
                lancado("VOO", d(2026, 3, 31), Some(d(2026, 3, 26))),
            ],
        };
        let mut fetched = BTreeMap::new();
        fetched.insert(
            "VOO".to_string(),
            Some(Dividends {
                dividends: vec![
                    (d(2025, 3, 27), dec!(1.812)),
                    (d(2025, 6, 30), dec!(1.745)),
                    (d(2025, 9, 29), dec!(1.74)),
                    (d(2025, 12, 22), dec!(1.771)),
                    (d(2026, 3, 27), dec!(1.872)),
                    (d(2026, 6, 26), dec!(1.962)),
                    (d(2026, 9, 28), dec!(1.823)),
                ],
                splits: vec![],
            }),
        );
        let r = market_incomes(&input, &fetched, d(2026, 10, 26));
        let datas: Vec<NaiveDate> = r.suggestions.iter().map(|s| s.ex_date).collect();
        println!("SUGERIDOS: {datas:?}");
        assert_eq!(datas.len(), 4, "só os de 2025 deveriam sobrar");
    }

    /// Sem data com em nenhum: o pagamento de março não pode ficar com a data
    /// ex de dezembro (o caso real que sobrava setembro na lista).
    #[test]
    fn sem_data_com_casa_pelo_pagamento_mais_perto() {
        let input = MarketInput {
            assets: vec![MarketAsset {
                ticker: "VOO".into(),
                currency: "USD".into(),
                kind: "ETF".into(),
                first: d(2024, 10, 28),
            }],
            ops: vec![compra("VOO", dec!(40))],
            incomes: vec![
                lancado("VOO", d(2026, 9, 30), None),
                lancado("VOO", d(2026, 6, 30), None),
                lancado("VOO", d(2026, 3, 31), None),
            ],
        };
        let mut fetched = BTreeMap::new();
        fetched.insert(
            "VOO".to_string(),
            Some(Dividends {
                dividends: vec![
                    (d(2025, 12, 22), dec!(1.771)),
                    (d(2026, 3, 27), dec!(1.872)),
                    (d(2026, 6, 26), dec!(1.962)),
                    (d(2026, 9, 28), dec!(1.823)),
                ],
                splits: vec![],
            }),
        );
        let r = market_incomes(&input, &fetched, d(2026, 10, 26));
        let datas: Vec<NaiveDate> = r.suggestions.iter().map(|s| s.ex_date).collect();
        assert_eq!(datas, vec![d(2025, 12, 22)]);
    }

    #[test]
    fn historico_completo_do_voo() {
        let input = MarketInput {
            assets: vec![MarketAsset {
                ticker: "VOO".into(),
                currency: "USD".into(),
                kind: "ETF".into(),
                first: d(2024, 10, 28),
            }],
            ops: vec![compra("VOO", dec!(40))],
            incomes: vec![
                lancado("VOO", d(2026, 9, 30), Some(d(2026, 9, 25))),
                lancado("VOO", d(2026, 6, 30), None),
                lancado("VOO", d(2026, 3, 31), None),
            ],
        };
        let mut fetched = BTreeMap::new();
        fetched.insert(
            "VOO".to_string(),
            Some(Dividends {
                dividends: vec![
                    (d(2024, 12, 23), dec!(1.7385)),
                    (d(2025, 3, 27), dec!(1.8121)),
                    (d(2025, 6, 30), dec!(1.7447)),
                    (d(2025, 9, 29), dec!(1.74)),
                    (d(2025, 12, 22), dec!(1.771)),
                    (d(2026, 3, 27), dec!(1.8724)),
                    (d(2026, 6, 26), dec!(1.9622)),
                    (d(2026, 9, 28), dec!(1.8226)),
                ],
                splits: vec![],
            }),
        );
        let r = market_incomes(&input, &fetched, d(2026, 10, 26));
        let datas: Vec<NaiveDate> = r.suggestions.iter().map(|s| s.ex_date).collect();
        println!("SUGERIDOS: {datas:?}");
        assert!(datas.contains(&d(2025, 12, 22)));
        assert_eq!(datas.len(), 5);
    }
}

#[cfg(test)]
mod missing_tests {
    use super::*;
    use carteira_core::savings::{build, MoveKind, Savings, SavingsKind};
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn avisa_os_meses_fechados_sem_lancamento() {
        let c = Savings {
            id: "c".into(),
            kind: SavingsKind::Cofrinho,
            account: "Banco X".into(),
            name: String::new(),
            rate_pct: dec!(100),
            tax_free: false,
            note: None,
            since: Some(d(2026, 7, 1)),
            until: None,
            fixed: None,
        };
        let mv = |id: &str, date| SavingsMove {
            id: id.into(),
            savings_id: "c".into(),
            date,
            kind: MoveKind::Saldo,
            amount: dec!(100),
            ir: None,
            iof: None,
            note: None,
        };
        let today = d(2026, 10, 20);
        // Saldos em julho e setembro: falta agosto (julho é o mais antigo dos três).
        let r = build(
            &c,
            &[mv("a", d(2026, 7, 31)), mv("b", d(2026, 9, 30))],
            today,
        );
        let m = missing_balances(&[r], today);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].months, vec![d(2026, 8, 1)]);
        // Tudo informado: nada a avisar.
        let r = build(
            &c,
            &[
                mv("a", d(2026, 7, 31)),
                mv("b", d(2026, 8, 31)),
                mv("c", d(2026, 9, 30)),
            ],
            today,
        );
        assert!(missing_balances(&[r], today).is_empty());
        // Antes do "desde" não pede nada.
        let novo = Savings {
            since: Some(d(2026, 9, 1)),
            ..c
        };
        let r = build(&novo, &[mv("b", d(2026, 9, 30))], today);
        assert!(missing_balances(&[r], today).is_empty());
    }
}
