//! Renda fixa calculada pela taxa (CDB, LCI, LCA, LC, RDB...).
//!
//! A taxa vem do texto que o usuário digitou no cadastro ("102% do CDI",
//! "CDI + 1%", "12,5% ao ano", "IPCA + 6%"). Com ela, o valor de R$ 1
//! aplicado cresce dia a dia:
//!
//! - % do CDI: em cada dia útil, `1 + CDI do dia × percentual`;
//! - CDI + taxa: `(1 + CDI do dia) × (1 + taxa)^(1/252)` por dia útil;
//! - prefixado: `(1 + taxa)^(1/252)` por dia útil;
//! - IPCA + taxa: o IPCA do mês distribuído pelos dias corridos, vezes
//!   `(1 + taxa)^(1/252)` por dia útil.
//!
//! Dia útil é o dia com CDI publicado pelo Banco Central (feriados ficam de
//! fora sozinhos). Depois do último CDI baixado, contam os dias de semana e
//! repete-se o último CDI; sem IPCA do mês, repete-se o último IPCA. O dia
//! da aplicação rende para o dia seguinte, como nos bancos: o valor no fim de
//! um dia já tem a taxa dos dias úteis anteriores a ele.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use rust_decimal::{Decimal, MathematicalOps};
use serde::Serialize;

/// Como a aplicação rende.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RateSpec {
    /// Percentual do CDI (102 = 102% do CDI).
    CdiPct(Decimal),
    /// CDI mais uma taxa ao ano, em % (1 = CDI + 1% ao ano).
    CdiPlus(Decimal),
    /// Prefixado, % ao ano.
    Pre(Decimal),
    /// IPCA mais uma taxa ao ano, em %.
    IpcaPlus(Decimal),
}

impl RateSpec {
    /// Como o app entendeu a taxa, para mostrar no cadastro.
    pub fn describe(&self) -> String {
        let n = |v: &Decimal| v.normalize().to_string().replace('.', ",");
        match self {
            RateSpec::CdiPct(p) => format!("{}% do CDI", n(p)),
            RateSpec::CdiPlus(s) => format!("CDI + {}% ao ano", n(s)),
            RateSpec::Pre(r) => format!("prefixado de {}% ao ano", n(r)),
            RateSpec::IpcaPlus(s) => format!("IPCA + {}% ao ano", n(s)),
        }
    }
}

/// Primeiro número do texto ("102,5" ou "102.5").
fn number(s: &str) -> Option<Decimal> {
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let tail = &s[start..];
    let end = tail
        .find(|c: char| !(c.is_ascii_digit() || c == ',' || c == '.'))
        .unwrap_or(tail.len());
    let raw = tail[..end].trim_end_matches(['.', ',']).replace(',', ".");
    raw.parse().ok()
}

/// Lê a taxa digitada. `None` quando não dá para entender: o saldo fica só
/// pelo que o usuário informar.
pub fn parse_rate(text: &str) -> Option<RateSpec> {
    let t = text.to_lowercase().replace(' ', "");
    if t.is_empty() {
        return None;
    }
    let value = number(&t)?;
    if value <= Decimal::ZERO {
        return None;
    }
    let after_plus = |key: &str| -> Option<Decimal> {
        let i = t.find(key)?;
        let rest = &t[i + key.len()..];
        rest.strip_prefix('+').and_then(number)
    };
    if t.contains("ipca") {
        let s = after_plus("ipca").or_else(|| t.contains('+').then_some(value))?;
        return (s < Decimal::from(30)).then_some(RateSpec::IpcaPlus(s));
    }
    if t.contains("cdi") || t.contains("di") {
        if t.contains('+') {
            let s = after_plus("cdi").or_else(|| after_plus("di"))?;
            return (s < Decimal::from(30)).then_some(RateSpec::CdiPlus(s));
        }
        return (value <= Decimal::from(300)).then_some(RateSpec::CdiPct(value));
    }
    // Só um número: taxa prefixada ao ano ("12,5%", "12,5% a.a.", "pré 12,5").
    (value < Decimal::from(100)).then_some(RateSpec::Pre(value))
}

/// Séries do Banco Central: CDI (% ao dia, por dia útil) e IPCA (% no mês,
/// datado no dia 1º).
pub struct Indexes<'a> {
    pub cdi: &'a BTreeMap<NaiveDate, Decimal>,
    pub ipca: &'a BTreeMap<NaiveDate, Decimal>,
}

fn weekday(d: NaiveDate) -> bool {
    !matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}

fn days_in_month(d: NaiveDate) -> u32 {
    let (y, m) = if d.month() == 12 {
        (d.year() + 1, 1)
    } else {
        (d.year(), d.month() + 1)
    };
    (NaiveDate::from_ymd_opt(y, m, 1).expect("dia 1") - Duration::days(1)).day()
}

/// Valor de R$ 1 aplicado em `from`, no fim de cada dia útil até `to`
/// (inclusive), mais `from` com 1. `None` sem os índices de que a taxa
/// precisa.
pub fn unit_curve(
    spec: RateSpec,
    from: NaiveDate,
    to: NaiveDate,
    idx: &Indexes,
) -> Option<Vec<(NaiveDate, Decimal)>> {
    let needs_cdi = matches!(spec, RateSpec::CdiPct(_) | RateSpec::CdiPlus(_));
    let needs_ipca = matches!(spec, RateSpec::IpcaPlus(_));
    let last_cdi = idx.cdi.keys().next_back().copied();
    // Sem CDI do período (nem para saber os dias úteis), não há o que calcular.
    if needs_cdi
        && idx.cdi.range(..from).next_back().is_none()
        && idx.cdi.range(from..).next().is_none()
    {
        return None;
    }
    if needs_ipca && idx.ipca.is_empty() {
        return None;
    }
    let hundred = Decimal::ONE_HUNDRED;
    let per_day = |annual: Decimal| -> Decimal {
        (Decimal::ONE + annual / hundred).powd(Decimal::ONE / Decimal::from(252))
    };
    let spread = match spec {
        RateSpec::CdiPlus(s) | RateSpec::IpcaPlus(s) => per_day(s),
        RateSpec::Pre(r) => per_day(r),
        RateSpec::CdiPct(_) => Decimal::ONE,
    };
    let mut ipca_month: Option<(u32, i32, Decimal)> = None;
    let mut ipca_day = |d: NaiveDate| -> Decimal {
        let key = (d.month(), d.year());
        if let Some((m, y, f)) = ipca_month {
            if (m, y) == key {
                return f;
            }
        }
        let first = d.with_day(1).expect("dia 1");
        let rate = idx
            .ipca
            .get(&first)
            .or_else(|| idx.ipca.range(..first).next_back().map(|(_, v)| v))
            .copied()
            .unwrap_or_default();
        let f =
            (Decimal::ONE + rate / hundred).powd(Decimal::ONE / Decimal::from(days_in_month(d)));
        ipca_month = Some((key.0, key.1, f));
        f
    };
    let mut last_rate = idx.cdi.range(..from).next_back().map(|(_, v)| *v);
    let mut value = Decimal::ONE;
    let mut out = vec![(from, value)];
    let mut d = from;
    while d < to {
        // A taxa do dia `d` aparece no fim do dia seguinte.
        let cdi_today = idx.cdi.get(&d).copied();
        if let Some(r) = cdi_today {
            last_rate = Some(r);
        }
        let business = cdi_today.is_some() || (last_cdi.is_none_or(|l| d > l) && weekday(d));
        let mut factor = Decimal::ONE;
        if needs_ipca {
            factor *= ipca_day(d);
        }
        if business {
            let cdi = cdi_today.or(last_rate).unwrap_or_default() / hundred;
            factor *= match spec {
                RateSpec::CdiPct(p) => Decimal::ONE + cdi * p / hundred,
                RateSpec::CdiPlus(_) => (Decimal::ONE + cdi) * spread,
                RateSpec::Pre(_) | RateSpec::IpcaPlus(_) => spread,
            };
        }
        value *= factor;
        d += Duration::days(1);
        if factor != Decimal::ONE || d == to {
            out.push((d, value.round_dp(12)));
        }
    }
    Some(out)
}

/// Produtos isentos de IR para pessoa física.
pub fn tax_free_product(product: &str) -> bool {
    let p = product.trim().to_uppercase();
    ["LCI", "LCA", "LCD", "CRI", "CRA", "LIG"]
        .iter()
        .any(|k| p.starts_with(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn entende_as_taxas_mais_comuns() {
        assert_eq!(parse_rate("102% do CDI"), Some(RateSpec::CdiPct(dec!(102))));
        assert_eq!(parse_rate("110 % CDI"), Some(RateSpec::CdiPct(dec!(110))));
        assert_eq!(parse_rate("CDI 98,5%"), Some(RateSpec::CdiPct(dec!(98.5))));
        assert_eq!(parse_rate("100% DI"), Some(RateSpec::CdiPct(dec!(100))));
        assert_eq!(parse_rate("CDI + 1,2%"), Some(RateSpec::CdiPlus(dec!(1.2))));
        assert_eq!(parse_rate("IPCA + 6%"), Some(RateSpec::IpcaPlus(dec!(6))));
        assert_eq!(
            parse_rate("ipca+5,75% a.a."),
            Some(RateSpec::IpcaPlus(dec!(5.75)))
        );
        assert_eq!(parse_rate("12,5% a.a."), Some(RateSpec::Pre(dec!(12.5))));
        assert_eq!(parse_rate("pré 13%"), Some(RateSpec::Pre(dec!(13))));
        assert_eq!(parse_rate(""), None);
        assert_eq!(parse_rate("não sei"), None);
        assert_eq!(parse_rate("IPCA"), None);
    }

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, day).unwrap()
    }

    #[test]
    fn cdi_rende_so_nos_dias_uteis_a_partir_do_dia_seguinte() {
        // CDI de 0,05% ao dia em 2, 3 e 5/3 (4/3 é feriado: sem CDI).
        let cdi: BTreeMap<_, _> = [
            (d(3, 2), dec!(0.05)),
            (d(3, 3), dec!(0.05)),
            (d(3, 5), dec!(0.05)),
        ]
        .into();
        let ipca = BTreeMap::new();
        let idx = Indexes {
            cdi: &cdi,
            ipca: &ipca,
        };
        let c = unit_curve(RateSpec::CdiPct(dec!(100)), d(3, 2), d(3, 5), &idx).unwrap();
        let at = |x: NaiveDate| c.iter().rev().find(|(k, _)| *k <= x).unwrap().1;
        assert_eq!(at(d(3, 2)), dec!(1));
        assert_eq!(at(d(3, 3)), dec!(1.0005));
        assert_eq!(at(d(3, 4)), dec!(1.00100025));
        // O feriado não rende.
        assert_eq!(at(d(3, 5)), dec!(1.00100025));
        // 200% do CDI rende o dobro por dia.
        let c = unit_curve(RateSpec::CdiPct(dec!(200)), d(3, 2), d(3, 3), &idx).unwrap();
        assert_eq!(c.last().unwrap().1, dec!(1.001));
    }

    #[test]
    fn prefixado_de_252_dias_uteis_rende_a_taxa_do_ano() {
        let cdi: BTreeMap<_, _> = BTreeMap::new();
        let ipca = BTreeMap::new();
        let idx = Indexes {
            cdi: &cdi,
            ipca: &ipca,
        };
        // Sem CDI baixado, contam os dias de semana: 252 dias úteis a partir
        // de uma segunda-feira são 50 semanas e 2 dias.
        let from = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        let to = from + Duration::days(50 * 7 + 2);
        let c = unit_curve(RateSpec::Pre(dec!(12)), from, to, &idx).unwrap();
        assert_eq!(c.last().unwrap().1.round_dp(6), dec!(1.12));
    }

    #[test]
    fn ipca_do_mes_se_espalha_pelos_dias_corridos() {
        let cdi: BTreeMap<_, _> = BTreeMap::new();
        let ipca: BTreeMap<_, _> = [(d(4, 1), dec!(0.5))].into();
        let idx = Indexes {
            cdi: &cdi,
            ipca: &ipca,
        };
        // Abril inteiro, sem taxa real: só o IPCA de 0,5%.
        let c = unit_curve(RateSpec::IpcaPlus(dec!(0.000001)), d(4, 1), d(5, 1), &idx).unwrap();
        assert_eq!(c.last().unwrap().1.round_dp(4), dec!(1.0050));
    }

    #[test]
    fn reconhece_os_isentos() {
        assert!(tax_free_product("LCI"));
        assert!(tax_free_product("lca banco x"));
        assert!(!tax_free_product("CDB"));
    }
}
