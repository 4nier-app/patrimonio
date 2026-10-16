//! Imposto de renda sobre a venda de ativos (pessoa física).
//!
//! **B3, apuração mensal** (IN RFB 1.585/2015):
//! - Operações comuns (ações, ETFs, BDRs e outros): 15% sobre o lucro do mês.
//!   Vendas de ações até R$ 20.000,00 no mês ficam isentas (o lucro das
//!   ações; o prejuízo continua compensável).
//! - Day trade (compra e venda do mesmo ativo, na mesma corretora e no mesmo
//!   dia, na quantidade casada): 20%, IRRF de 1% sobre o resultado do dia.
//! - FIIs: 20%, sem isenção.
//! - Prejuízo só compensa lucro do mesmo grupo (comum, day trade, FII), sem
//!   prazo.
//! - IRRF "dedo-duro" de 0,005% sobre as vendas comuns; retenção até
//!   R$ 1,00 no mês é dispensada. O IRRF é abatido do imposto do mês e o
//!   que sobra passa para os meses seguintes.
//! - DARF (código 6015) vence no último dia útil do mês seguinte. Abaixo de
//!   R$ 10,00 não se paga: o valor soma ao do mês seguinte.
//! - Custo de cada venda: preço médio de todas as compras do ativo (em todas
//!   as corretoras), com as taxas.
//!
//! **Exterior e criptomoedas, apuração anual** (Lei 14.754/2023): 15% sobre
//! o ganho do ano, em reais. Cada compra e cada venda são convertidas pela
//! PTAX de venda da própria data; prejuízo compensa ganho dos anos seguintes.
//! Entra na declaração anual, sem DARF mensal.
//!
//! É uma ferramenta de apoio: o resultado deve ser conferido com um contador.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::Serialize;

use crate::model::{FxSource, Side, BRL};
use crate::operations::PricedOperation;

/// Grupo de apuração da B3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Group {
    Comum,
    DayTrade,
    Fii,
}

impl Group {
    pub const ALL: [Group; 3] = [Group::Comum, Group::DayTrade, Group::Fii];

    pub fn label(&self) -> &'static str {
        match self {
            Group::Comum => "Operações comuns",
            Group::DayTrade => "Day trade",
            Group::Fii => "Fundos imobiliários",
        }
    }

    pub fn rate(&self) -> Decimal {
        match self {
            Group::Comum => Decimal::new(15, 2),
            Group::DayTrade | Group::Fii => Decimal::new(20, 2),
        }
    }
}

/// Limite de vendas de ações no mês para a isenção.
pub const EXEMPTION_LIMIT: Decimal = Decimal::from_parts(20_000, 0, 0, false, 0);
/// DARF abaixo disto não é pago; soma ao mês seguinte.
pub const DARF_MINIMUM: Decimal = Decimal::from_parts(10, 0, 0, false, 0);
/// Código da receita no DARF de renda variável (pessoa física).
pub const DARF_CODE: &str = "6015";

fn irrf_common_rate() -> Decimal {
    Decimal::new(5, 5)
}
fn irrf_day_trade_rate() -> Decimal {
    Decimal::new(1, 2)
}
fn exterior_rate() -> Decimal {
    Decimal::new(15, 2)
}

fn cents(v: Decimal) -> Decimal {
    v.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

fn month_of(d: NaiveDate) -> String {
    format!("{:04}-{:02}", d.year(), d.month())
}

// ---------- dias úteis ----------

fn easter(year: i32) -> NaiveDate {
    // Algoritmo de Meeus/Jones/Butcher (calendário gregoriano).
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).expect("data da Páscoa")
}

/// Feriados nacionais em que os bancos não abrem (inclui Carnaval, Sexta-feira
/// Santa e Corpus Christi; Consciência Negra a partir de 2024).
pub fn is_bank_holiday(d: NaiveDate) -> bool {
    let fixed = [
        (1, 1),
        (4, 21),
        (5, 1),
        (9, 7),
        (10, 12),
        (11, 2),
        (11, 15),
        (12, 25),
    ];
    if fixed.contains(&(d.month(), d.day())) {
        return true;
    }
    if d.year() >= 2024 && d.month() == 11 && d.day() == 20 {
        return true;
    }
    let e = easter(d.year());
    [-48, -47, -2, 60]
        .iter()
        .any(|off| e + Duration::days(*off) == d)
}

pub fn is_business_day(d: NaiveDate) -> bool {
    !matches!(d.weekday(), Weekday::Sat | Weekday::Sun) && !is_bank_holiday(d)
}

/// Vencimento do DARF de um mês: último dia útil do mês seguinte.
pub fn darf_due(year: i32, month: u32) -> NaiveDate {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let mut d = NaiveDate::from_ymd_opt(ny, nm, 1).expect("data válida") - Duration::days(1);
    while !is_business_day(d) {
        d -= Duration::days(1);
    }
    d
}

// ---------- B3 ----------

/// Uma venda (ou a parte casada de um day trade) com a conta do resultado.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SaleLine {
    pub date: NaiveDate,
    pub account: String,
    pub asset: String,
    pub group: Group,
    /// Ação: entra no limite de isenção de R$ 20 mil.
    pub stock: bool,
    /// Parte casada de compra e venda no mesmo dia e corretora.
    pub day_trade: bool,
    pub quantity: Decimal,
    /// Valor da venda (quantidade × preço).
    pub gross: Decimal,
    /// Valor da venda menos as taxas.
    pub net: Decimal,
    /// Custo: preço médio × quantidade (no day trade, o das compras do dia).
    pub cost: Decimal,
    pub average_price: Decimal,
    pub result: Decimal,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupMonth {
    pub group: Group,
    /// Resultado das vendas do mês (já sem o lucro isento de ações).
    pub result: Decimal,
    /// Prejuízo de meses anteriores disponível no início do mês.
    pub loss_before: Decimal,
    /// Prejuízo usado para abater o lucro.
    pub loss_used: Decimal,
    /// Prejuízo que passa para o mês seguinte.
    pub loss_after: Decimal,
    pub base: Decimal,
    pub rate: Decimal,
    pub tax: Decimal,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MonthTax {
    /// "AAAA-MM".
    pub month: String,
    /// Vendas de ações em operações comuns (para o limite de R$ 20 mil).
    pub stock_sales: Decimal,
    pub exempt: bool,
    /// Lucro de ações que ficou isento.
    pub exempt_profit: Decimal,
    pub groups: Vec<GroupMonth>,
    /// Soma do imposto dos grupos.
    pub gross_tax: Decimal,
    /// IRRF estimado retido no mês (dedo-duro + day trade).
    pub irrf_month: Decimal,
    /// IRRF de meses anteriores ainda não usado.
    pub irrf_before: Decimal,
    pub irrf_used: Decimal,
    pub irrf_after: Decimal,
    /// Imposto de meses anteriores que ficou abaixo de R$ 10.
    pub small_before: Decimal,
    /// Imposto a pagar neste mês, já com o acumulado.
    pub due: Decimal,
    /// Valor do DARF (zero se ficou abaixo de R$ 10).
    pub darf: Decimal,
    /// Valor que fica para o próximo mês por ser menor que R$ 10.
    pub small_after: Decimal,
    pub due_date: NaiveDate,
    pub sales: Vec<SaleLine>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct B3Report {
    pub months: Vec<MonthTax>,
    pub warnings: Vec<String>,
}

/// Classe do ativo no catálogo ("ACAO", "FII", "ETF", "BDR", "OUTRO"...).
pub type KindOf<'a> = &'a dyn Fn(&str) -> Option<String>;

#[derive(Default, Clone)]
struct Holding {
    quantity: Decimal,
    cost: Decimal,
}

impl Holding {
    fn average(&self) -> Decimal {
        if self.quantity.is_zero() {
            Decimal::ZERO
        } else {
            self.cost / self.quantity
        }
    }

    fn buy(&mut self, quantity: Decimal, cost: Decimal) {
        self.quantity += quantity;
        self.cost += cost;
    }

    /// Tira uma quantidade ao preço médio; devolve (quantidade, custo).
    fn take(&mut self, quantity: Decimal) -> (Decimal, Decimal) {
        let out = quantity.min(self.quantity);
        let out = if crate::model::is_dust(self.quantity - out) {
            self.quantity
        } else {
            out
        };
        let cost = if self.quantity.is_zero() {
            Decimal::ZERO
        } else {
            self.cost * out / self.quantity
        };
        self.quantity -= out;
        self.cost -= cost;
        if self.quantity.is_zero() {
            self.cost = Decimal::ZERO;
        }
        (out, cost)
    }
}

/// Bolsa brasileira. Fundos de investimento ficam de fora: o banco retém o
/// IR no resgate e no come-cotas, sem DARF.
fn is_b3(p: &PricedOperation) -> bool {
    p.operation.currency == BRL
        && !crate::crypto::is_crypto(&p.operation.asset)
        && !crate::fund::is_fund(&p.operation.asset)
}

/// Posição de cada ativo da B3 no fim de uma data, ao preço médio fiscal.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YearEndPosition {
    pub asset: String,
    pub quantity: Decimal,
    /// Custo de aquisição em reais (o que vai em Bens e Direitos).
    pub cost_brl: Decimal,
    /// Custo em dólar, para ativos no exterior.
    pub cost_usd: Option<Decimal>,
}

/// Compras e vendas de um ativo numa corretora no mesmo dia.
type DaySides<'a> = (Vec<&'a PricedOperation>, Vec<&'a PricedOperation>);

struct B3Run {
    sales: Vec<SaleLine>,
    irrf_dt: BTreeMap<String, Decimal>,
    warnings: Vec<String>,
    year_end: BTreeMap<i32, Vec<YearEndPosition>>,
}

fn run_b3(priced: &[PricedOperation], kind_of: KindOf) -> B3Run {
    let ops: Vec<&PricedOperation> = priced.iter().filter(|p| is_b3(p)).collect();
    let dates: BTreeSet<NaiveDate> = ops.iter().map(|p| p.operation.date).collect();
    let mut holdings: BTreeMap<String, Holding> = BTreeMap::new();
    let mut sales = Vec::new();
    let mut irrf_dt: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut year_end = BTreeMap::new();
    let kind = |asset: &str| kind_of(asset).unwrap_or_else(|| "OUTRO".into());
    let mut last_year: Option<i32> = None;

    for date in dates {
        if let Some(y) = last_year {
            for year in y..date.year() {
                year_end.insert(year, snapshot(&holdings));
            }
        }
        last_year = Some(date.year());
        let today: Vec<&PricedOperation> = ops
            .iter()
            .copied()
            .filter(|p| p.operation.date == date)
            .collect();
        // Eventos corporativos valem desde a abertura do pregão: entram antes
        // do day trade e não contam como compra.
        for p in today.iter().filter(|p| p.operation.side == Side::Ajuste) {
            holdings
                .entry(p.operation.asset.clone())
                .or_default()
                .buy(p.operation.quantity, p.gross);
        }
        // Por (ativo, conta): soma de compras e de vendas do dia.
        let mut by_key: BTreeMap<(String, String), DaySides> = BTreeMap::new();
        for p in &today {
            let o = &p.operation;
            let e = by_key
                .entry((o.asset.clone(), o.account.clone()))
                .or_default();
            match o.side {
                Side::Compra => e.0.push(p),
                Side::Venda => e.1.push(p),
                Side::Transferencia | Side::Ajuste => {}
            }
        }
        let mut leftover_buys: Vec<(String, Decimal, Decimal)> = Vec::new();
        let mut leftover_sells: Vec<(String, String, Decimal, Decimal, Decimal)> = Vec::new();
        let mut dt_result_by_account: BTreeMap<String, Decimal> = BTreeMap::new();
        for ((asset, account), (buys, sells)) in &by_key {
            let qb: Decimal = buys.iter().map(|p| p.operation.quantity).sum();
            let qs: Decimal = sells.iter().map(|p| p.operation.quantity).sum();
            let cost_b: Decimal = buys.iter().map(|p| p.gross + p.fees).sum();
            let gross_s: Decimal = sells.iter().map(|p| p.gross).sum();
            let net_s: Decimal = sells.iter().map(|p| p.gross - p.fees).sum();
            let matched = qb.min(qs);
            let fii = kind(asset) == "FII";
            if matched > Decimal::ZERO {
                let cost = cost_b * matched / qb;
                let net = net_s * matched / qs;
                let gross = gross_s * matched / qs;
                let result = net - cost;
                sales.push(SaleLine {
                    date,
                    account: account.clone(),
                    asset: asset.clone(),
                    group: if fii { Group::Fii } else { Group::DayTrade },
                    stock: false,
                    day_trade: true,
                    quantity: matched,
                    gross: cents(gross),
                    net: cents(net),
                    cost: cents(cost),
                    average_price: cost / matched,
                    result: cents(result),
                });
                *dt_result_by_account.entry(account.clone()).or_default() += result;
            }
            if qb > matched {
                leftover_buys.push((asset.clone(), qb - matched, cost_b * (qb - matched) / qb));
            }
            if qs > matched {
                let q = qs - matched;
                leftover_sells.push((
                    asset.clone(),
                    account.clone(),
                    q,
                    gross_s * q / qs,
                    net_s * q / qs,
                ));
            }
        }
        // IRRF de day trade: 1% do resultado positivo do dia em cada corretora.
        for r in dt_result_by_account.values() {
            if *r > Decimal::ZERO {
                *irrf_dt.entry(month_of(date)).or_default() += *r * irrf_day_trade_rate();
            }
        }
        // Sem a hora de cada ordem, as compras do dia entram antes das vendas.
        for (asset, q, cost) in leftover_buys {
            holdings.entry(asset).or_default().buy(q, cost);
        }
        for (asset, account, q, gross, net) in leftover_sells {
            let h = holdings.entry(asset.clone()).or_default();
            let average = h.average();
            let (out, cost) = h.take(q);
            if out < q && !crate::model::is_dust(q - out) {
                warnings.push(format!(
                    "{asset}: venda de {q} em {} maior que a posição ({out}); o custo do que faltou ficou zero.",
                    date.format("%d/%m/%Y")
                ));
            }
            let k = kind(&asset);
            let group = if k == "FII" { Group::Fii } else { Group::Comum };
            sales.push(SaleLine {
                date,
                account,
                asset,
                group,
                stock: k == "ACAO",
                day_trade: false,
                quantity: q,
                gross: cents(gross),
                net: cents(net),
                cost: cents(cost),
                average_price: average,
                result: cents(net - cost),
            });
        }
        // Resíduo de transferência de custódia: sai sem venda.
        for p in today
            .iter()
            .filter(|p| p.operation.side == Side::Transferencia)
        {
            if let Some(r) = p.operation.residue.filter(|r| *r > Decimal::ZERO) {
                holdings
                    .entry(p.operation.asset.clone())
                    .or_default()
                    .take(r);
            }
        }
    }
    if let Some(y) = last_year {
        year_end.insert(y, snapshot(&holdings));
    }
    B3Run {
        sales,
        irrf_dt,
        warnings,
        year_end,
    }
}

fn snapshot(holdings: &BTreeMap<String, Holding>) -> Vec<YearEndPosition> {
    holdings
        .iter()
        .filter(|(_, h)| !crate::model::is_dust(h.quantity))
        .map(|(a, h)| YearEndPosition {
            asset: a.clone(),
            quantity: h.quantity,
            cost_brl: cents(h.cost),
            cost_usd: None,
        })
        .collect()
}

/// Apuração mensal da B3 até `until` (inclusive), com a memória de cálculo.
pub fn b3_months(priced: &[PricedOperation], kind_of: KindOf, until: NaiveDate) -> B3Report {
    let run = run_b3(priced, kind_of);
    let mut by_month: BTreeMap<String, Vec<SaleLine>> = BTreeMap::new();
    for s in run.sales.into_iter().filter(|s| s.date <= until) {
        by_month.entry(month_of(s.date)).or_default().push(s);
    }
    let mut losses: BTreeMap<Group, Decimal> = BTreeMap::new();
    let mut irrf_carry = Decimal::ZERO;
    let mut small = Decimal::ZERO;
    let mut months = Vec::new();
    let first = by_month.keys().next().cloned();
    let Some(first) = first else {
        return B3Report {
            months,
            warnings: run.warnings,
        };
    };
    // Percorre todos os meses do primeiro com venda até `until`, para levar
    // adiante o acumulado abaixo de R$ 10 e o IRRF.
    let (mut y, mut m) = (
        first[..4].parse::<i32>().unwrap_or(until.year()),
        first[5..].parse::<u32>().unwrap_or(1),
    );
    while (y, m) <= (until.year(), until.month()) {
        let key = format!("{y:04}-{m:02}");
        let sales = by_month.remove(&key).unwrap_or_default();
        let stock_sales: Decimal = sales
            .iter()
            .filter(|s| s.group == Group::Comum && s.stock)
            .map(|s| s.gross)
            .sum();
        let exempt = stock_sales <= EXEMPTION_LIMIT;
        let stock_result: Decimal = sales
            .iter()
            .filter(|s| s.group == Group::Comum && s.stock)
            .map(|s| s.result)
            .sum();
        let exempt_profit = if exempt && stock_result > Decimal::ZERO {
            stock_result
        } else {
            Decimal::ZERO
        };
        let mut groups = Vec::new();
        let mut gross_tax = Decimal::ZERO;
        for g in Group::ALL {
            let mut result: Decimal = sales
                .iter()
                .filter(|s| s.group == g)
                .map(|s| s.result)
                .sum();
            if g == Group::Comum {
                result -= exempt_profit;
            }
            let before = losses.get(&g).copied().unwrap_or_default();
            let (used, after, base) = if result > Decimal::ZERO {
                let used = before.min(result);
                (used, before - used, result - used)
            } else {
                (Decimal::ZERO, before - result, Decimal::ZERO)
            };
            losses.insert(g, after);
            let tax = cents(base * g.rate());
            gross_tax += tax;
            if !result.is_zero() || !before.is_zero() {
                groups.push(GroupMonth {
                    group: g,
                    result,
                    loss_before: before,
                    loss_used: used,
                    loss_after: after,
                    base,
                    rate: g.rate(),
                    tax,
                });
            }
        }
        let common_sales: Decimal = sales.iter().filter(|s| !s.day_trade).map(|s| s.gross).sum();
        let mut irrf_month = cents(common_sales * irrf_common_rate());
        if irrf_month <= Decimal::ONE {
            irrf_month = Decimal::ZERO;
        }
        let dt = cents(run.irrf_dt.get(&key).copied().unwrap_or_default());
        irrf_month += if dt <= Decimal::ONE {
            Decimal::ZERO
        } else {
            dt
        };
        let irrf_before = irrf_carry;
        let available = irrf_before + irrf_month;
        let irrf_used = available.min(gross_tax);
        let irrf_after = available - irrf_used;
        irrf_carry = irrf_after;
        let small_before = small;
        let due = gross_tax - irrf_used + small_before;
        let (darf, small_after) = if due >= DARF_MINIMUM {
            (due, Decimal::ZERO)
        } else {
            (Decimal::ZERO, due)
        };
        small = small_after;
        let any = !sales.is_empty() || !due.is_zero() || !irrf_after.is_zero();
        if any {
            months.push(MonthTax {
                month: key,
                stock_sales,
                exempt,
                exempt_profit,
                groups,
                gross_tax,
                irrf_month,
                irrf_before,
                irrf_used,
                irrf_after,
                small_before,
                due,
                darf,
                small_after,
                due_date: darf_due(y, m),
                sales,
            });
        }
        (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    }
    B3Report {
        months,
        warnings: run.warnings,
    }
}

// ---------- exterior e cripto (anual) ----------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForeignSale {
    pub date: NaiveDate,
    pub account: String,
    pub asset: String,
    pub crypto: bool,
    pub quantity: Decimal,
    pub net_usd: Decimal,
    pub ptax: Decimal,
    pub net_brl: Decimal,
    pub cost_brl: Decimal,
    pub result_brl: Decimal,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForeignYear {
    pub year: i32,
    pub result: Decimal,
    pub loss_before: Decimal,
    pub loss_used: Decimal,
    pub loss_after: Decimal,
    pub base: Decimal,
    pub rate: Decimal,
    pub tax: Decimal,
    pub sales: Vec<ForeignSale>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct AnnualReport {
    pub foreign: Vec<ForeignYear>,
    /// Posição em 31/12 de cada ano (B3, exterior e cripto), ao custo.
    pub positions: BTreeMap<i32, Vec<YearEndPosition>>,
    pub warnings: Vec<String>,
}

#[derive(Default, Clone)]
struct ForeignHolding {
    quantity: Decimal,
    cost_brl: Decimal,
    cost_usd: Decimal,
}

/// Apuração anual do exterior e das criptomoedas, e posição de fim de ano.
pub fn annual(
    priced: &[PricedOperation],
    kind_of: KindOf,
    fx: &dyn FxSource,
    until: NaiveDate,
) -> AnnualReport {
    let mut warnings = Vec::new();
    let mut ops: Vec<&PricedOperation> = priced
        .iter()
        .filter(|p| {
            !is_b3(p) && !crate::fund::is_fund(&p.operation.asset) && p.operation.date <= until
        })
        .collect();
    ops.sort_by_key(|p| (p.operation.date, p.operation.side.day_order()));
    let mut holdings: BTreeMap<String, ForeignHolding> = BTreeMap::new();
    let mut sales: Vec<ForeignSale> = Vec::new();
    let mut year_end: BTreeMap<i32, Vec<YearEndPosition>> = BTreeMap::new();
    let mut last_year: Option<i32> = None;
    let snap = |h: &BTreeMap<String, ForeignHolding>| -> Vec<YearEndPosition> {
        h.iter()
            .filter(|(_, h)| !crate::model::is_dust(h.quantity))
            .map(|(a, h)| YearEndPosition {
                asset: a.clone(),
                quantity: h.quantity,
                cost_brl: cents(h.cost_brl),
                cost_usd: Some(cents(h.cost_usd)),
            })
            .collect()
    };
    for p in ops {
        let o = &p.operation;
        if let Some(y) = last_year {
            for year in y..o.date.year() {
                year_end.insert(year, snap(&holdings));
            }
        }
        last_year = Some(o.date.year());
        let rate = match fx.usd_brl_on_or_before(o.date) {
            Some(r) => r.rate,
            None => {
                if o.side != Side::Transferencia {
                    warnings.push(format!(
                        "{}: sem PTAX para {}; operação fora do relatório anual.",
                        o.asset,
                        o.date.format("%d/%m/%Y")
                    ));
                }
                continue;
            }
        };
        let h = holdings.entry(o.asset.clone()).or_default();
        let out_of = |h: &mut ForeignHolding, q: Decimal| -> (Decimal, Decimal) {
            let out = q.min(h.quantity);
            let out = if crate::model::is_dust(h.quantity - out) {
                h.quantity
            } else {
                out
            };
            if h.quantity.is_zero() {
                return (Decimal::ZERO, Decimal::ZERO);
            }
            let brl = h.cost_brl * out / h.quantity;
            let usd = h.cost_usd * out / h.quantity;
            h.quantity -= out;
            h.cost_brl -= brl;
            h.cost_usd -= usd;
            if h.quantity.is_zero() {
                h.cost_brl = Decimal::ZERO;
                h.cost_usd = Decimal::ZERO;
            }
            (out, brl)
        };
        match o.side {
            Side::Compra | Side::Ajuste => {
                let usd = p.gross + p.fees;
                h.quantity += o.quantity;
                h.cost_usd += usd;
                h.cost_brl += usd * rate;
            }
            Side::Venda => {
                let net_usd = p.gross - p.fees;
                let (_, cost_brl) = out_of(h, o.quantity);
                let net_brl = net_usd * rate;
                sales.push(ForeignSale {
                    date: o.date,
                    account: o.account.clone(),
                    asset: o.asset.clone(),
                    crypto: crate::crypto::is_crypto(&o.asset),
                    quantity: o.quantity,
                    net_usd: cents(net_usd),
                    ptax: rate,
                    net_brl: cents(net_brl),
                    cost_brl: cents(cost_brl),
                    result_brl: cents(net_brl - cost_brl),
                });
            }
            Side::Transferencia => {
                if let Some(r) = o.residue.filter(|r| *r > Decimal::ZERO) {
                    out_of(h, r);
                }
            }
        }
    }
    if let Some(y) = last_year {
        year_end.insert(y, snap(&holdings));
    }
    let mut by_year: BTreeMap<i32, Vec<ForeignSale>> = BTreeMap::new();
    for s in sales {
        by_year.entry(s.date.year()).or_default().push(s);
    }
    let mut loss = Decimal::ZERO;
    let mut foreign = Vec::new();
    for (year, sales) in by_year {
        let result: Decimal = sales.iter().map(|s| s.result_brl).sum();
        let before = loss;
        let (used, after, base) = if result > Decimal::ZERO {
            let used = before.min(result);
            (used, before - used, result - used)
        } else {
            (Decimal::ZERO, before - result, Decimal::ZERO)
        };
        loss = after;
        foreign.push(ForeignYear {
            year,
            result,
            loss_before: before,
            loss_used: used,
            loss_after: after,
            base,
            rate: exterior_rate(),
            tax: cents(base * exterior_rate()),
            sales,
        });
    }
    // Posições de fim de ano: B3 + exterior.
    let b3 = run_b3(
        &priced
            .iter()
            .filter(|p| p.operation.date <= until)
            .cloned()
            .collect::<Vec<_>>(),
        kind_of,
    );
    let mut positions = b3.year_end;
    for (year, list) in year_end {
        positions.entry(year).or_default().extend(list);
    }
    // Anos sem operação herdam a posição do ano anterior.
    if let (Some(&first), Some(&last)) = (positions.keys().next(), positions.keys().last()) {
        let end = last.max(until.year());
        let mut prev: Option<Vec<YearEndPosition>> = None;
        for year in first..=end {
            match positions.get(&year) {
                Some(list) => prev = Some(list.clone()),
                None => {
                    if let Some(p) = &prev {
                        positions.insert(year, p.clone());
                    }
                }
            }
        }
    }
    for list in positions.values_mut() {
        list.sort_by(|a, b| a.asset.cmp(&b.asset));
    }
    AnnualReport {
        foreign,
        positions,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FxRate, USD};
    use crate::operations::Operation;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn op(
        account: &str,
        asset: &str,
        date: NaiveDate,
        side: Side,
        q: Decimal,
        price: Decimal,
        fees: Decimal,
    ) -> PricedOperation {
        let operation = Operation {
            id: format!("{asset}-{date}-{side:?}-{q}"),
            account: account.into(),
            asset: asset.into(),
            date,
            side,
            quantity: q,
            price,
            manual_fees: Some(fees),
            note: None,
            currency: BRL.into(),
            from_account: None,
            residue: None,
            ir: None,
            iof: None,
        };
        PricedOperation {
            gross: operation.gross(),
            fees,
            fee_origin: "informada",
            operation,
        }
    }

    fn kinds(t: &str) -> Option<String> {
        Some(
            match t {
                "FUND11" => "FII",
                "INDX11" => "ETF",
                _ => "ACAO",
            }
            .into(),
        )
    }

    #[test]
    fn vencimento_no_ultimo_dia_util_do_mes_seguinte() {
        // Março/2026 → 30/04/2026 (quinta).
        assert_eq!(darf_due(2026, 3), d(2026, 4, 30));
        // Janeiro/2026 → 27/02/2026 (sexta; 28 é sábado).
        assert_eq!(darf_due(2026, 1), d(2026, 2, 27));
        // Dezembro/2025 → 30/01/2026.
        assert_eq!(darf_due(2025, 12), d(2026, 1, 30));
        // Páscoa de 2026 em 05/04: Sexta-feira Santa 03/04 e Carnaval 16-17/02.
        assert!(is_bank_holiday(d(2026, 4, 3)));
        assert!(is_bank_holiday(d(2026, 2, 17)));
        assert!(is_bank_holiday(d(2026, 6, 4)), "Corpus Christi");
    }

    #[test]
    fn vendas_de_acoes_ate_20_mil_isentas() {
        let ops = vec![
            op(
                "X",
                "ABCD3",
                d(2026, 1, 5),
                Side::Compra,
                dec!(100),
                dec!(10),
                dec!(0),
            ),
            op(
                "X",
                "ABCD3",
                d(2026, 2, 5),
                Side::Venda,
                dec!(100),
                dec!(15),
                dec!(0),
            ),
        ];
        let r = b3_months(&ops, &kinds, d(2026, 2, 28));
        let feb = r.months.iter().find(|m| m.month == "2026-02").unwrap();
        assert!(feb.exempt);
        assert_eq!(feb.exempt_profit, dec!(500));
        assert_eq!(feb.darf, dec!(0));
    }

    #[test]
    fn acima_de_20_mil_paga_15_por_cento_e_abate_prejuizo() {
        let ops = vec![
            op(
                "X",
                "ABCD3",
                d(2026, 1, 5),
                Side::Compra,
                dec!(2000),
                dec!(10),
                dec!(0),
            ),
            // Janeiro: prejuízo de R$ 1.000 (vende 1.000 a 9 = 9.000, isento mas compensável).
            op(
                "X",
                "ABCD3",
                d(2026, 1, 20),
                Side::Venda,
                dec!(1000),
                dec!(9),
                dec!(0),
            ),
            // Março: vende 1.000 a 25 = 25.000 (> 20 mil): lucro 15.000.
            op(
                "X",
                "ABCD3",
                d(2026, 3, 10),
                Side::Venda,
                dec!(1000),
                dec!(25),
                dec!(0),
            ),
        ];
        let r = b3_months(&ops, &kinds, d(2026, 3, 31));
        let jan = r.months.iter().find(|m| m.month == "2026-01").unwrap();
        assert_eq!(jan.groups[0].loss_after, dec!(1000));
        let mar = r.months.iter().find(|m| m.month == "2026-03").unwrap();
        assert!(!mar.exempt);
        let g = &mar.groups[0];
        assert_eq!(
            (g.result, g.loss_used, g.base),
            (dec!(15000), dec!(1000), dec!(14000))
        );
        assert_eq!(g.tax, dec!(2100));
        // Dedo-duro: 0,005% de 25.000 = 1,25.
        assert_eq!(mar.irrf_month, dec!(1.25));
        assert_eq!(mar.darf, dec!(2098.75));
        assert_eq!(mar.due_date, d(2026, 4, 30));
    }

    #[test]
    fn day_trade_separado_com_20_por_cento() {
        let ops = vec![
            op(
                "X",
                "ABCD3",
                d(2026, 1, 5),
                Side::Compra,
                dec!(100),
                dec!(10),
                dec!(0),
            ),
            // Mesmo dia, mesma corretora: compra 200 a 20 e vende 300 a 21.
            op(
                "X",
                "ABCD3",
                d(2026, 2, 3),
                Side::Compra,
                dec!(200),
                dec!(20),
                dec!(0),
            ),
            op(
                "X",
                "ABCD3",
                d(2026, 2, 3),
                Side::Venda,
                dec!(300),
                dec!(21),
                dec!(0),
            ),
        ];
        let r = b3_months(&ops, &kinds, d(2026, 2, 28));
        let feb = r.months.iter().find(|m| m.month == "2026-02").unwrap();
        let dt = feb
            .groups
            .iter()
            .find(|g| g.group == Group::DayTrade)
            .unwrap();
        // 200 casadas: 200 × (21 − 20) = 200.
        assert_eq!(dt.result, dec!(200));
        assert_eq!(dt.tax, dec!(40));
        // As outras 100 vendidas saem do preço médio (10): lucro 1.100, isento.
        assert!(feb.exempt);
        assert_eq!(feb.exempt_profit, dec!(1100));
        // IRRF de day trade: 1% de 200 = 2,00.
        assert_eq!(feb.irrf_month, dec!(2));
        assert_eq!(feb.darf, dec!(38));
    }

    #[test]
    fn corretoras_diferentes_nao_sao_day_trade_e_fii_paga_20() {
        let ops = vec![
            op(
                "X",
                "FUND11",
                d(2026, 1, 5),
                Side::Compra,
                dec!(10),
                dec!(100),
                dec!(0),
            ),
            op(
                "Y",
                "FUND11",
                d(2026, 1, 5),
                Side::Venda,
                dec!(10),
                dec!(110),
                dec!(0),
            ),
            op(
                "X",
                "INDX11",
                d(2026, 1, 6),
                Side::Compra,
                dec!(10),
                dec!(100),
                dec!(0),
            ),
            op(
                "X",
                "INDX11",
                d(2026, 1, 7),
                Side::Venda,
                dec!(10),
                dec!(120),
                dec!(0),
            ),
        ];
        let r = b3_months(&ops, &kinds, d(2026, 1, 31));
        let jan = &r.months[0];
        assert!(jan.groups.iter().all(|g| g.group != Group::DayTrade));
        let fii = jan.groups.iter().find(|g| g.group == Group::Fii).unwrap();
        assert_eq!(fii.tax, dec!(20));
        // ETF não tem isenção: 15% de 200 = 30.
        let comum = jan.groups.iter().find(|g| g.group == Group::Comum).unwrap();
        assert_eq!(comum.tax, dec!(30));
        assert_eq!(jan.darf, dec!(50));
    }

    #[test]
    fn darf_abaixo_de_10_reais_passa_para_o_mes_seguinte() {
        let ops = vec![
            op(
                "X",
                "INDX11",
                d(2026, 1, 5),
                Side::Compra,
                dec!(20),
                dec!(100),
                dec!(0),
            ),
            op(
                "X",
                "INDX11",
                d(2026, 1, 20),
                Side::Venda,
                dec!(10),
                dec!(105),
                dec!(0),
            ),
            op(
                "X",
                "INDX11",
                d(2026, 2, 20),
                Side::Venda,
                dec!(10),
                dec!(105),
                dec!(0),
            ),
        ];
        let r = b3_months(&ops, &kinds, d(2026, 2, 28));
        assert_eq!(
            (r.months[0].darf, r.months[0].small_after),
            (dec!(0), dec!(7.5))
        );
        assert_eq!(
            (r.months[1].small_before, r.months[1].darf),
            (dec!(7.5), dec!(15))
        );
    }

    struct Fx;
    impl FxSource for Fx {
        fn usd_brl_on_or_before(&self, date: NaiveDate) -> Option<FxRate> {
            Some(FxRate {
                date,
                rate: if date.year() == 2025 {
                    dec!(5)
                } else {
                    dec!(6)
                },
                source: "teste".into(),
            })
        }
    }

    #[test]
    fn exterior_em_reais_pela_ptax_de_cada_data() {
        let usd = |side, date, q, p| {
            let mut o = op("Nomad", "VOO", date, side, q, p, dec!(0));
            o.operation.currency = USD.into();
            o
        };
        let ops = vec![
            usd(Side::Compra, d(2025, 3, 3), dec!(2), dec!(100)),
            // Venda de 1 a US$ 100: em dólar empata, em reais ganha (6 − 5) × 100.
            usd(Side::Venda, d(2026, 3, 3), dec!(1), dec!(100)),
        ];
        let r = annual(&ops, &kinds, &Fx, d(2026, 12, 31));
        assert_eq!(r.foreign.len(), 1);
        let y = &r.foreign[0];
        assert_eq!((y.year, y.result, y.tax), (2026, dec!(100), dec!(15)));
        let end25 = &r.positions[&2025][0];
        assert_eq!(
            (end25.quantity, end25.cost_brl, end25.cost_usd),
            (dec!(2), dec!(1000), Some(dec!(200)))
        );
        assert_eq!(r.positions[&2026][0].quantity, dec!(1));
    }
}
