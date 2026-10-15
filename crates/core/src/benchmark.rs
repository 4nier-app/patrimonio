//! Índices de comparação (CDI, IPCA, Ibovespa, IVVB11, dólar) e a simulação
//! "mesmos aportes no índice".
//!
//! Cada índice é medido nos mesmos períodos da carteira (mês a mês, do início
//! do período ao fim), para que a comparação use o mesmo tempo de exposição.
//!
//! - Taxa diária (CDI, % ao dia): (1 + t₁)·(1 + t₂)·… − 1 nos dias do período.
//! - Taxa mensal (IPCA, % no mês): proporcional aos dias do período no mês,
//!   (1 + t)^(dias/dias do mês) − 1.
//! - Nível (Ibovespa, IVVB11, dólar): valor no fim ÷ valor na véspera do
//!   início − 1.
//!
//! Mesmos aportes: cada compra ou venda da carteira entra (ou sai) do índice
//! na mesma data e pelo mesmo valor em reais:
//! V₁ = V₀·(1 + R do período) + Σ Fᵢ·(1 + R da data de Fᵢ até o fim).

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use rust_decimal::{Decimal, MathematicalOps};
use serde::Serialize;

use crate::format::{brl, number, pct};
use crate::returns::MonthResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IndexKind {
    /// Percentual ao dia (CDI).
    DailyRate,
    /// Percentual no mês, datado no dia 1 (IPCA).
    MonthlyRate,
    /// Pontos ou preço (Ibovespa, IVVB11, dólar).
    Level,
}

#[derive(Debug, Clone)]
pub struct IndexSeries {
    pub id: String,
    pub name: String,
    pub kind: IndexKind,
    pub source: String,
    pub points: BTreeMap<NaiveDate, Decimal>,
}

/// Rentabilidade do índice num intervalo, com a explicação.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IndexReturn {
    pub rate: Decimal,
    pub explanation: String,
}

fn days_in_month(d: NaiveDate) -> i64 {
    let first = d.with_day(1).expect("dia 1");
    let next = if d.month() == 12 {
        NaiveDate::from_ymd_opt(d.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1)
    }
    .expect("data válida");
    (next - first).num_days()
}

fn level_on_or_before(
    points: &BTreeMap<NaiveDate, Decimal>,
    d: NaiveDate,
) -> Option<(NaiveDate, Decimal)> {
    points.range(..=d).next_back().map(|(k, v)| (*k, *v))
}

impl IndexSeries {
    pub fn last_date(&self) -> Option<NaiveDate> {
        self.points.keys().next_back().copied()
    }

    /// Rentabilidade de `start` a `end`, inclusive. `None` quando faltam dados.
    pub fn period_return(&self, start: NaiveDate, end: NaiveDate) -> Option<IndexReturn> {
        if end < start {
            return Some(IndexReturn {
                rate: Decimal::ZERO,
                explanation: format!("{}: intervalo vazio, 0%", self.name),
            });
        }
        let last = self.last_date()?;
        match self.kind {
            IndexKind::DailyRate => {
                // A série precisa cobrir o período (a taxa do dia sai no dia útil seguinte).
                if last + Duration::days(5) < end {
                    return None;
                }
                let days: Vec<Decimal> = self.points.range(start..=end).map(|(_, v)| *v).collect();
                let factor: Decimal = days
                    .iter()
                    .map(|t| Decimal::ONE + t / Decimal::ONE_HUNDRED)
                    .product();
                let rate = factor - Decimal::ONE;
                Some(IndexReturn {
                    rate,
                    explanation: format!(
                        "{} de {} a {}: produto de {} taxas diárias (1 + t) − 1 = {}",
                        self.name,
                        start.format("%d/%m/%Y"),
                        end.format("%d/%m/%Y"),
                        days.len(),
                        pct(rate, 4)
                    ),
                })
            }
            IndexKind::MonthlyRate => {
                let mut factor = Decimal::ONE;
                let mut parts = Vec::new();
                let mut cursor = start;
                while cursor <= end {
                    let month_first = cursor.with_day(1).expect("dia 1");
                    let dim = days_in_month(cursor);
                    let month_last = month_first + Duration::days(dim - 1);
                    let to = month_last.min(end);
                    let covered = (to - cursor).num_days() + 1;
                    let t = *self.points.get(&month_first)?;
                    let f = Decimal::ONE + t / Decimal::ONE_HUNDRED;
                    let f = if covered == dim {
                        f
                    } else {
                        f.powd(Decimal::from(covered) / Decimal::from(dim))
                    };
                    factor *= f;
                    parts.push(format!(
                        "{} {}%{}",
                        month_first.format("%m/%Y"),
                        number(t, 2),
                        if covered == dim {
                            String::new()
                        } else {
                            format!(" × {covered}/{dim} dias")
                        }
                    ));
                    cursor = month_last + Duration::days(1);
                }
                let rate = factor - Decimal::ONE;
                Some(IndexReturn {
                    rate,
                    explanation: format!("{}: {} = {}", self.name, parts.join("; "), pct(rate, 4)),
                })
            }
            IndexKind::Level => {
                let (d0, v0) = level_on_or_before(&self.points, start - Duration::days(1))?;
                let (d1, v1) = level_on_or_before(&self.points, end)?;
                // Sem valor recente, o fim do período não está coberto.
                if d1 + Duration::days(7) < end || v0.is_zero() {
                    return None;
                }
                let rate = v1 / v0 - Decimal::ONE;
                Some(IndexReturn {
                    rate,
                    explanation: format!(
                        "{}: {} ({}) ÷ {} ({}) − 1 = {}",
                        self.name,
                        number(v1, 4),
                        d1.format("%d/%m/%Y"),
                        number(v0, 4),
                        d0.format("%d/%m/%Y"),
                        pct(rate, 4)
                    ),
                })
            }
        }
    }
}

impl IndexSeries {
    /// Rentabilidade de um aporte feito em `date` até `end`. No índice de
    /// nível, a base é o valor do próprio dia (a operação é a preço de
    /// fechamento); nas taxas, o dia do aporte já rende.
    pub fn flow_return(&self, date: NaiveDate, end: NaiveDate) -> Option<IndexReturn> {
        match self.kind {
            IndexKind::Level => self.period_return(date + Duration::days(1), end),
            _ => self.period_return(date, end),
        }
    }
}

/// O índice mês a mês, nos mesmos períodos da carteira.
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkView {
    pub id: String,
    pub name: String,
    pub kind: IndexKind,
    pub source: String,
    /// Uma posição para cada mês da carteira.
    pub rates: Vec<Option<Decimal>>,
    /// Valor que teria a carteira se os mesmos aportes fossem para o índice.
    pub same_flows: Vec<Option<Decimal>>,
    pub explanations: Vec<Vec<String>>,
}

pub fn compare(series: &IndexSeries, months: &[MonthResult]) -> BenchmarkView {
    let mut rates = Vec::with_capacity(months.len());
    let mut same_flows = Vec::with_capacity(months.len());
    let mut explanations = Vec::with_capacity(months.len());
    let mut value = Some(Decimal::ZERO);
    for m in months {
        // Período que começa numa compra (carteira vazia antes) conta como aporte.
        let period = if m.start.holdings.is_empty() {
            series.flow_return(m.period_start, m.period_end)
        } else {
            series.period_return(m.period_start, m.period_end)
        };
        let mut lines = vec![match &period {
            Some(r) => r.explanation.clone(),
            None => format!(
                "{}: sem dados de {} a {} (atualize as cotações)",
                series.name,
                m.period_start.format("%d/%m/%Y"),
                m.period_end.format("%d/%m/%Y")
            ),
        }];
        rates.push(period.as_ref().map(|r| r.rate));

        value = match (value, &period) {
            (Some(v0), Some(r)) => {
                let mut v1 = v0 * (Decimal::ONE + r.rate);
                let mut parts = vec![format!("{} × (1 + {})", brl(v0), pct(r.rate, 4))];
                let mut ok = true;
                for f in &m.flows {
                    if f.amount.is_zero() {
                        continue;
                    }
                    // A compra rende a partir do próprio dia; a venda deixa de render nele.
                    match series.flow_return(f.date, m.period_end) {
                        Some(fr) => {
                            v1 += f.amount * (Decimal::ONE + fr.rate);
                            parts.push(format!(
                                "{} × (1 + {} desde {})",
                                brl(f.amount),
                                pct(fr.rate, 4),
                                f.date.format("%d/%m/%Y")
                            ));
                        }
                        None => ok = false,
                    }
                }
                if ok {
                    lines.push(format!(
                        "Mesmos aportes no {}: {} = {}",
                        series.name,
                        parts.join(" + "),
                        brl(v1)
                    ));
                    Some(v1)
                } else {
                    None
                }
            }
            _ => None,
        };
        same_flows.push(value);
        explanations.push(lines);
    }
    BenchmarkView {
        id: series.id.clone(),
        name: series.name.clone(),
        kind: series.kind,
        source: series.source.clone(),
        rates,
        same_flows,
        explanations,
    }
}

/// Encadeia taxas: (1 + r₁)·(1 + r₂)·… − 1. `None` se faltar alguma.
pub fn chain(rates: &[Option<Decimal>]) -> Option<Decimal> {
    rates
        .iter()
        .map(|r| r.map(|r| Decimal::ONE + r))
        .product::<Option<Decimal>>()
        .map(|p| p - Decimal::ONE)
}

/// Queda desde o pico do acumulado, mês a mês (0 ou negativo).
pub fn drawdowns(rates: &[Option<Decimal>]) -> Vec<Option<Decimal>> {
    let mut level = Decimal::ONE;
    let mut peak = Decimal::ONE;
    let mut broken = false;
    rates
        .iter()
        .map(|r| {
            match r {
                Some(r) if !broken => {
                    level *= Decimal::ONE + r;
                    peak = peak.max(level);
                }
                _ => broken = true,
            }
            (!broken).then(|| level / peak - Decimal::ONE)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Quote;
    use crate::model::{Side, Trade, BRL};
    use crate::returns::{monthly_returns, PriceSource, Scope};
    use rust_decimal_macros::dec;

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, day).unwrap()
    }

    fn series(kind: IndexKind, points: &[(NaiveDate, Decimal)]) -> IndexSeries {
        IndexSeries {
            id: "X".into(),
            name: "Índice".into(),
            kind,
            source: "TESTE".into(),
            points: points.iter().cloned().collect(),
        }
    }

    #[test]
    fn taxa_diaria_encadeada() {
        let s = series(
            IndexKind::DailyRate,
            &[
                (d(3, 2), dec!(0.05)),
                (d(3, 3), dec!(0.05)),
                (d(3, 4), dec!(0.05)),
            ],
        );
        let r = s.period_return(d(3, 3), d(3, 4)).unwrap();
        assert_eq!(r.rate, dec!(1.0005) * dec!(1.0005) - Decimal::ONE);
        assert!(r.explanation.contains("2 taxas"));
        // Período bem depois do último dado: sem dados.
        assert!(s.period_return(d(3, 3), d(3, 31)).is_none());
    }

    #[test]
    fn taxa_mensal_proporcional() {
        let s = series(
            IndexKind::MonthlyRate,
            &[(d(4, 1), dec!(1.0)), (d(5, 1), dec!(0.5))],
        );
        assert_eq!(
            s.period_return(d(4, 1), d(5, 31)).unwrap().rate,
            dec!(1.01) * dec!(1.005) - Decimal::ONE
        );
        let half = s.period_return(d(4, 16), d(4, 30)).unwrap().rate;
        assert!((half - dec!(0.00498)).abs() < dec!(0.00001), "{half}");
        assert!(s.period_return(d(6, 1), d(6, 30)).is_none());
    }

    #[test]
    fn nivel_pela_vespera() {
        let s = series(
            IndexKind::Level,
            &[(d(2, 27), dec!(100)), (d(3, 31), dec!(110))],
        );
        let r = s.period_return(d(3, 1), d(3, 31)).unwrap();
        assert_eq!(r.rate, dec!(0.1));
        assert!(r.explanation.contains("27/02/2026"));
    }

    struct Prices(Vec<Quote>);
    impl PriceSource for Prices {
        fn close_on_or_before(&self, asset: &str, date: NaiveDate) -> Option<Quote> {
            self.0
                .iter()
                .filter(|q| q.asset == asset && q.date <= date)
                .max_by_key(|q| q.date)
                .cloned()
        }
    }

    #[test]
    fn mesmos_aportes_no_indice() {
        // Compra de R$ 1.000 em 02/03 e mais R$ 1.000 em 01/04; índice sobe 10% em março e 0% em abril.
        let t = |id: &str, date, price: Decimal| Trade {
            id: id.into(),
            asset: "TEST3".into(),
            date,
            side: Side::Compra,
            quantity: dec!(100),
            price,
            gross: dec!(100) * price,
            fees: Decimal::ZERO,
            currency: BRL.into(),
        };
        let trades = vec![t("a", d(3, 2), dec!(10)), t("b", d(4, 1), dec!(10))];
        let q = |date, close| Quote {
            id: 0,
            asset: "TEST3".into(),
            date,
            close,
            source: "T".into(),
        };
        let prices = Prices(vec![
            q(d(3, 2), dec!(10)),
            q(d(3, 31), dec!(10)),
            q(d(4, 30), dec!(10)),
        ]);
        let months = monthly_returns(&trades, &prices, d(4, 30), &Scope::Carteira);
        let s = series(
            IndexKind::Level,
            &[
                (d(3, 2), dec!(100)),
                (d(3, 31), dec!(110)),
                (d(4, 30), dec!(110)),
            ],
        );
        let view = compare(&s, &months);
        // Março: a compra de 02/03 rende de 02/03 (100) a 31/03 (110) = 10%.
        assert_eq!(view.same_flows[0], Some(dec!(1100)));
        // Abril: 1.100 parados + 1.000 novos, índice estável.
        assert_eq!(view.same_flows[1], Some(dec!(2100)));
        assert_eq!(view.rates[1], Some(Decimal::ZERO));
    }

    #[test]
    fn encadeia_e_mede_a_queda() {
        let r = [Some(dec!(0.1)), Some(dec!(-0.2)), Some(dec!(0.1))];
        assert_eq!(
            chain(&r),
            Some(dec!(1.1) * dec!(0.8) * dec!(1.1) - Decimal::ONE)
        );
        let dd = drawdowns(&r);
        assert_eq!(dd[0], Some(Decimal::ZERO));
        assert_eq!(dd[1], Some(dec!(-0.2)));
        assert_eq!(dd[2], Some(dec!(-0.12)));
        assert_eq!(drawdowns(&[None, Some(dec!(0.1))]), vec![None, None]);
    }
}
