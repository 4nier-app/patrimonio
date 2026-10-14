//! Rentabilidade pelo tempo real de exposição do capital.
//!
//! Para cada mês usa o método Modified Dietz com pesos diários:
//!
//! ```text
//! Lucro       = V1 − V0 − ΣFᵢ
//! Rentab.     = Lucro / (V0 + Σ wᵢ·Fᵢ)
//! wᵢ          = dias em que o fluxo i esteve na carteira / dias do período
//! ```
//!
//! O período começa no 1º dia do mês, ou na data da primeira compra se a
//! posição não existia no início do mês. Termina no último dia do mês (ou na
//! data de referência, no mês corrente), ou na véspera da venda que zerou a
//! posição. Assim o capital só conta a partir da data exata da compra e deixa
//! de contar na data exata da venda. Ver docs/ARQUITETURA.md, seção 5.
//!
//! O ano é o encadeamento dos meses: (1 + R₁)·(1 + R₂)·… − 1.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::format::{brl, number, pct, qty};
use crate::model::{Quote, Side, Trade};

/// Identificador e versão da fórmula, gravados em cada resultado.
pub const FORMULA_ID: &str = "modified-dietz-diario";
pub const FORMULA_VERSION: u32 = 1;

/// Fonte de cotações: fechamento na data ou no último pregão anterior.
pub trait PriceSource {
    fn close_on_or_before(&self, asset: &str, date: NaiveDate) -> Option<Quote>;
}

/// Escopo do cálculo: um ativo ou a carteira inteira.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "tipo", content = "ativo", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Scope {
    Ativo(String),
    Carteira,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Holding {
    pub asset: String,
    pub quantity: Decimal,
    pub quote: Option<Quote>,
    pub value: Option<Decimal>,
}

/// Valor de mercado numa data, com a composição usada.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Valuation {
    pub date: NaiveDate,
    pub holdings: Vec<Holding>,
    /// `None` quando falta alguma cotação.
    pub total: Option<Decimal>,
}

impl Valuation {
    fn zero(date: NaiveDate) -> Self {
        Valuation { date, holdings: Vec::new(), total: Some(Decimal::ZERO) }
    }
}

/// Um fluxo de caixa (compra ou venda) dentro do período.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Flow {
    pub trade_id: String,
    pub asset: String,
    pub date: NaiveDate,
    pub side: Side,
    pub quantity: Decimal,
    pub price: Decimal,
    pub gross: Decimal,
    pub fees: Decimal,
    /// Positivo em compras, negativo em vendas.
    pub amount: Decimal,
    /// Dias em que o fluxo ficou na carteira dentro do período.
    pub days_exposed: i64,
    pub weight: Decimal,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MonthResult {
    pub year: i32,
    pub month: u32,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub days: i64,
    /// A posição foi zerada dentro do mês.
    pub closed: bool,
    pub start: Valuation,
    pub end: Valuation,
    pub flows: Vec<Flow>,
    pub net_flows: Decimal,
    pub weighted_flows: Decimal,
    /// `None` quando falta cotação.
    pub profit: Option<Decimal>,
    pub average_capital: Option<Decimal>,
    /// Fração (0,05 = 5%). `None` sem cotação ou sem capital médio positivo.
    pub rate: Option<Decimal>,
    pub formula_id: &'static str,
    pub formula_version: u32,
    /// Fórmula com os números substituídos, para a memória de cálculo.
    pub explanation: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YearResult {
    pub year: i32,
    pub profit: Option<Decimal>,
    pub rate: Option<Decimal>,
    pub months: usize,
    pub explanation: String,
}

fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let (y, m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(y, m, 1).expect("data válida") - Duration::days(1)
}

fn first_day_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).expect("dia 1 existe")
}

fn positions_before(trades: &[&Trade], date: NaiveDate) -> BTreeMap<String, Decimal> {
    let mut map: BTreeMap<String, Decimal> = BTreeMap::new();
    for t in trades.iter().filter(|t| t.date < date) {
        *map.entry(t.asset.clone()).or_default() += t.signed_quantity();
    }
    map.retain(|_, q| !q.is_zero());
    map
}

fn value_positions(positions: &BTreeMap<String, Decimal>, date: NaiveDate, prices: &dyn PriceSource) -> Valuation {
    let mut total = Some(Decimal::ZERO);
    let holdings = positions
        .iter()
        .map(|(asset, quantity)| {
            let quote = prices.close_on_or_before(asset, date);
            let value = quote.as_ref().map(|q| q.close * quantity);
            total = match (total, value) {
                (Some(t), Some(v)) => Some(t + v),
                _ => None,
            };
            Holding { asset: asset.clone(), quantity: *quantity, quote, value }
        })
        .collect();
    Valuation { date, holdings, total }
}

fn describe_valuation(label: &str, v: &Valuation) -> String {
    if v.holdings.is_empty() {
        return format!("{label} = R$ 0,00 (sem posição)");
    }
    let parts: Vec<String> = v
        .holdings
        .iter()
        .map(|h| match &h.quote {
            Some(q) => format!(
                "{} {} × {} (fechamento de {}, {})",
                qty(h.quantity),
                h.asset,
                brl(q.close),
                q.date.format("%d/%m/%Y"),
                q.source
            ),
            None => format!("{} {} × cotação indisponível", qty(h.quantity), h.asset),
        })
        .collect();
    match v.total {
        Some(t) => format!("{label} = {} = {}", parts.join(" + "), brl(t)),
        None => format!("{label} = {} = indisponível", parts.join(" + ")),
    }
}

/// Calcula a rentabilidade mês a mês até `as_of` (inclusive).
pub fn monthly_returns(trades: &[Trade], prices: &dyn PriceSource, as_of: NaiveDate, scope: &Scope) -> Vec<MonthResult> {
    let mut selected: Vec<&Trade> = trades
        .iter()
        .filter(|t| t.date <= as_of)
        .filter(|t| match scope {
            Scope::Ativo(a) => &t.asset == a,
            Scope::Carteira => true,
        })
        .collect();
    selected.sort_by_key(|t| t.date);
    let Some(first) = selected.first() else { return Vec::new() };

    let mut results = Vec::new();
    let mut month_start = first_day_of_month(first.date);
    while month_start <= as_of {
        let month_end = last_day_of_month(month_start.year(), month_start.month()).min(as_of);
        if let Some(r) = month_result(&selected, prices, month_start, month_end) {
            results.push(r);
        }
        month_start = last_day_of_month(month_start.year(), month_start.month()) + Duration::days(1);
    }
    results
}

fn month_result(trades: &[&Trade], prices: &dyn PriceSource, month_start: NaiveDate, month_end: NaiveDate) -> Option<MonthResult> {
    let at_start = positions_before(trades, month_start);
    let in_month: Vec<&Trade> =
        trades.iter().copied().filter(|t| t.date >= month_start && t.date <= month_end).collect();
    if at_start.is_empty() && in_month.is_empty() {
        return None;
    }

    let period_start = if at_start.is_empty() { in_month[0].date } else { month_start };
    let at_end = positions_before(trades, month_end + Duration::days(1));
    let closed = at_end.is_empty();
    // Na venda que zera a posição o capital deixa de contar: o período vai até a véspera.
    let close_date = if closed { in_month.last().map(|t| t.date) } else { None };
    let period_end = match close_date {
        Some(d) => (d - Duration::days(1)).max(period_start),
        None => month_end,
    };
    let days = (period_end - period_start).num_days() + 1;
    let days_dec = Decimal::from(days);

    let start = if at_start.is_empty() {
        Valuation::zero(period_start)
    } else {
        value_positions(&at_start, month_start - Duration::days(1), prices)
    };
    let end = if closed { Valuation::zero(period_end) } else { value_positions(&at_end, period_end, prices) };

    let flows: Vec<Flow> = in_month
        .iter()
        .map(|t| {
            let days_exposed = match close_date {
                // Compra e venda no mesmo dia em que a posição abriu e fechou.
                Some(d) if t.date == d && d == period_start => match t.side {
                    Side::Compra => 1,
                    Side::Venda => 0,
                },
                Some(d) if t.date >= d => 0,
                _ => ((period_end - t.date).num_days() + 1).clamp(0, days),
            };
            Flow {
                trade_id: t.id.clone(),
                asset: t.asset.clone(),
                date: t.date,
                side: t.side,
                quantity: t.quantity,
                price: t.price,
                gross: t.gross,
                fees: t.fees,
                amount: t.cash_flow(),
                days_exposed,
                weight: Decimal::from(days_exposed) / days_dec,
            }
        })
        .collect();

    let net_flows: Decimal = flows.iter().map(|f| f.amount).sum();
    let weighted_flows: Decimal = flows.iter().map(|f| f.amount * f.weight).sum();
    let (profit, average_capital) = match (start.total, end.total) {
        (Some(v0), Some(v1)) => (Some(v1 - v0 - net_flows), Some(v0 + weighted_flows)),
        _ => (None, None),
    };
    let rate = match (profit, average_capital) {
        (Some(p), Some(c)) if c > Decimal::ZERO => Some(p / c),
        _ => None,
    };

    let mut explanation = vec![
        format!(
            "Período: {} a {} ({} dias){}",
            period_start.format("%d/%m/%Y"),
            period_end.format("%d/%m/%Y"),
            days,
            if closed { ", encerrado na véspera da venda que zerou a posição" } else { "" }
        ),
        describe_valuation("V0 (início)", &start),
        describe_valuation("V1 (fim)", &end),
    ];
    for f in &flows {
        explanation.push(format!(
            "F {} {}: {} {} × {} = {} {} taxas {} = {}; peso {}/{} = {}",
            f.date.format("%d/%m/%Y"),
            if f.side == Side::Compra { "compra" } else { "venda" },
            qty(f.quantity),
            f.asset,
            brl(f.price),
            brl(f.gross),
            if f.side == Side::Compra { "+" } else { "−" },
            brl(f.fees),
            brl(f.amount),
            f.days_exposed,
            days,
            number(f.weight, 4)
        ));
    }
    match (profit, average_capital) {
        (Some(p), Some(c)) => {
            explanation.push(format!(
                "Lucro = V1 − V0 − ΣF = {} − {} − {} = {}",
                brl(end.total.unwrap_or_default()),
                brl(start.total.unwrap_or_default()),
                brl(net_flows),
                brl(p)
            ));
            explanation.push(format!(
                "Capital médio = V0 + Σ(w·F) = {} + {} = {}",
                brl(start.total.unwrap_or_default()),
                brl(weighted_flows),
                brl(c)
            ));
            explanation.push(match rate {
                Some(r) => format!("Rentabilidade = Lucro / Capital médio = {} / {} = {}", brl(p), brl(c), pct(r, 2)),
                None => "Rentabilidade indisponível: capital médio não é positivo".to_string(),
            });
        }
        _ => explanation.push("Resultado indisponível: falta cotação de fechamento (atualize as cotações)".to_string()),
    }

    Some(MonthResult {
        year: month_start.year(),
        month: month_start.month(),
        period_start,
        period_end,
        days,
        closed,
        start,
        end,
        flows,
        net_flows,
        weighted_flows,
        profit,
        average_capital,
        rate,
        formula_id: FORMULA_ID,
        formula_version: FORMULA_VERSION,
        explanation,
    })
}

/// Resultado anual: lucro somado e rentabilidade encadeada dos meses.
pub fn yearly_returns(months: &[MonthResult]) -> Vec<YearResult> {
    let mut by_year: BTreeMap<i32, Vec<&MonthResult>> = BTreeMap::new();
    for m in months {
        by_year.entry(m.year).or_default().push(m);
    }
    by_year
        .into_iter()
        .map(|(year, ms)| {
            let profit = ms.iter().map(|m| m.profit).sum::<Option<Decimal>>();
            let rate = ms
                .iter()
                .map(|m| m.rate.map(|r| Decimal::ONE + r))
                .product::<Option<Decimal>>()
                .map(|p| p - Decimal::ONE);
            let factors: Vec<String> = ms
                .iter()
                .map(|m| match m.rate {
                    Some(r) => format!("(1 + {})", pct(r, 2)),
                    None => "(indisponível)".into(),
                })
                .collect();
            let explanation = match rate {
                Some(r) => format!("{} − 1 = {}", factors.join(" × "), pct(r, 2)),
                None => format!("{} − 1: indisponível", factors.join(" × ")),
            };
            YearResult { year, profit, rate, months: ms.len(), explanation }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;

    struct Prices(HashMap<(String, NaiveDate), Decimal>);

    impl Prices {
        fn new(list: &[(&str, (i32, u32, u32), Decimal)]) -> Self {
            Prices(list.iter().map(|(a, (y, m, d), p)| ((a.to_string(), day(*y, *m, *d)), *p)).collect())
        }
    }

    impl PriceSource for Prices {
        fn close_on_or_before(&self, asset: &str, date: NaiveDate) -> Option<Quote> {
            (0..10).find_map(|back| {
                let d = date - Duration::days(back);
                self.0.get(&(asset.to_string(), d)).map(|p| Quote {
                    id: 0,
                    asset: asset.into(),
                    date: d,
                    close: *p,
                    source: "teste".into(),
                })
            })
        }
    }

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn trade(id: &str, asset: &str, date: NaiveDate, side: Side, q: Decimal, price: Decimal, fees: Decimal) -> Trade {
        Trade { id: id.into(), asset: asset.into(), date, side, quantity: q, price, gross: q * price, fees }
    }

    fn round2(v: Decimal) -> Decimal {
        (v * dec!(100)).round_dp(2)
    }

    #[test]
    fn exemplo_da_documentacao_petr4_janeiro() {
        // Compra em 15/01, fechamento de 30/01 (último pregão) a R$ 40,00.
        let trades = vec![trade("t1", "PETR4", day(2026, 1, 15), Side::Compra, dec!(100), dec!(38), dec!(1.13))];
        let prices = Prices::new(&[("PETR4", (2026, 1, 30), dec!(40))]);
        let r = monthly_returns(&trades, &prices, day(2026, 1, 31), &Scope::Ativo("PETR4".into()));
        assert_eq!(r.len(), 1);
        let m = &r[0];
        assert_eq!(m.period_start, day(2026, 1, 15));
        assert_eq!(m.days, 17);
        assert_eq!(m.flows[0].weight, Decimal::ONE);
        assert_eq!(m.profit, Some(dec!(198.87)));
        assert_eq!(round2(m.rate.unwrap()), dec!(5.23));
        assert!(m.explanation.iter().any(|l| l.contains("R$ 198,87")));
    }

    #[test]
    fn segundo_aporte_pesa_so_pelos_dias_investido() {
        let trades = vec![
            trade("t1", "PETR4", day(2026, 1, 15), Side::Compra, dec!(100), dec!(38), dec!(1.13)),
            trade("t2", "PETR4", day(2026, 1, 22), Side::Compra, dec!(50), dec!(39), dec!(0.57)),
        ];
        let prices = Prices::new(&[("PETR4", (2026, 1, 30), dec!(40))]);
        let m = &monthly_returns(&trades, &prices, day(2026, 1, 31), &Scope::Carteira)[0];
        assert_eq!(m.days, 17);
        assert_eq!(m.flows[1].days_exposed, 10);
        // Lucro = 6.000 − 0 − (3.801,13 + 1.950,57) = 248,30
        assert_eq!(m.profit, Some(dec!(248.30)));
        let capital = dec!(3801.13) + dec!(1950.57) * dec!(10) / dec!(17);
        assert_eq!(m.average_capital.unwrap().round_dp(10), capital.round_dp(10));
        assert_eq!(m.rate.unwrap().round_dp(10), (dec!(248.30) / capital).round_dp(10));
    }

    #[test]
    fn mes_seguinte_comeca_com_o_valor_do_fim_do_mes_anterior() {
        let trades = vec![trade("t1", "VALE3", day(2026, 1, 15), Side::Compra, dec!(10), dec!(60), dec!(0))];
        let prices = Prices::new(&[("VALE3", (2026, 1, 30), dec!(62)), ("VALE3", (2026, 2, 27), dec!(65.10))]);
        let r = monthly_returns(&trades, &prices, day(2026, 2, 28), &Scope::Carteira);
        assert_eq!(r.len(), 2);
        let feb = &r[1];
        assert_eq!(feb.period_start, day(2026, 2, 1));
        assert_eq!(feb.start.total, Some(dec!(620)));
        assert_eq!(feb.end.total, Some(dec!(651.0)));
        assert_eq!(feb.rate, Some(dec!(0.05)));
        let years = yearly_returns(&r);
        let expected = (Decimal::ONE + r[0].rate.unwrap()) * dec!(1.05) - Decimal::ONE;
        assert_eq!(years[0].rate, Some(expected));
        assert_eq!(years[0].profit, Some(dec!(51.0)));
    }

    #[test]
    fn venda_total_encerra_o_periodo_na_vespera() {
        let trades = vec![
            trade("t1", "ITSA4", day(2026, 3, 2), Side::Compra, dec!(100), dec!(10), dec!(0)),
            trade("t2", "ITSA4", day(2026, 3, 12), Side::Venda, dec!(100), dec!(11), dec!(0)),
        ];
        let prices = Prices::new(&[]);
        let r = monthly_returns(&trades, &prices, day(2026, 4, 30), &Scope::Carteira);
        assert_eq!(r.len(), 1, "abril não tem exposição");
        let m = &r[0];
        assert!(m.closed);
        assert_eq!(m.period_end, day(2026, 3, 11));
        assert_eq!(m.days, 10);
        assert_eq!(m.flows[1].weight, Decimal::ZERO);
        assert_eq!(m.profit, Some(dec!(100)));
        assert_eq!(m.rate, Some(dec!(0.1)));
    }

    #[test]
    fn compra_e_venda_no_mesmo_dia() {
        let trades = vec![
            trade("t1", "BBAS3", day(2026, 5, 6), Side::Compra, dec!(100), dec!(20), dec!(0)),
            trade("t2", "BBAS3", day(2026, 5, 6), Side::Venda, dec!(100), dec!(20.5), dec!(0)),
        ];
        let m = &monthly_returns(&trades, &Prices::new(&[]), day(2026, 5, 31), &Scope::Carteira)[0];
        assert_eq!(m.days, 1);
        assert_eq!(m.profit, Some(dec!(50)));
        assert_eq!(m.rate, Some(dec!(0.025)));
    }

    #[test]
    fn venda_parcial_reduz_o_capital_a_partir_da_data_da_venda() {
        let trades = vec![
            trade("t1", "WEGE3", day(2026, 6, 1), Side::Compra, dec!(100), dec!(50), dec!(0)),
            trade("t2", "WEGE3", day(2026, 6, 16), Side::Venda, dec!(50), dec!(52), dec!(0)),
        ];
        let prices = Prices::new(&[("WEGE3", (2026, 6, 30), dec!(53))]);
        let m = &monthly_returns(&trades, &prices, day(2026, 6, 30), &Scope::Carteira)[0];
        assert!(!m.closed);
        assert_eq!(m.days, 30);
        assert_eq!(m.flows[1].days_exposed, 15);
        // Lucro = 2.650 − 0 − (5.000 − 2.600) = 250; capital = 5.000 − 2.600 × 15/30 = 3.700
        assert_eq!(m.profit, Some(dec!(250)));
        assert_eq!(m.average_capital, Some(dec!(3700)));
    }

    #[test]
    fn sem_cotacao_o_resultado_fica_indisponivel() {
        let trades = vec![trade("t1", "ABEV3", day(2026, 1, 10), Side::Compra, dec!(10), dec!(12), dec!(0))];
        let r = monthly_returns(&trades, &Prices::new(&[]), day(2026, 1, 31), &Scope::Carteira);
        assert_eq!(r[0].profit, None);
        assert_eq!(r[0].rate, None);
        assert!(r[0].explanation.last().unwrap().contains("falta cotação"));
        assert_eq!(yearly_returns(&r)[0].rate, None);
    }

    #[test]
    fn mes_corrente_vai_ate_a_data_de_referencia() {
        let trades = vec![trade("t1", "PETR4", day(2026, 10, 1), Side::Compra, dec!(10), dec!(40), dec!(0))];
        let prices = Prices::new(&[("PETR4", (2026, 10, 13), dec!(42))]);
        let m = &monthly_returns(&trades, &prices, day(2026, 10, 14), &Scope::Carteira)[0];
        assert_eq!(m.period_end, day(2026, 10, 14));
        assert_eq!(m.end.holdings[0].quote.as_ref().unwrap().date, day(2026, 10, 13));
        assert_eq!(m.profit, Some(dec!(20)));
    }
}
