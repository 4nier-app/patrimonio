//! Evolução do patrimônio: quanto havia em cada ativo no fim de cada mês (ou
//! de cada ano), agrupado como a planilha do usuário. É um retrato do
//! patrimônio, não da rentabilidade: aporte aparece como alta e saque como
//! queda. Para explicar a variação, a tela separa o dinheiro novo (aportes −
//! resgates dos investimentos) do rendimento.

use std::collections::BTreeMap;

use carteira_core::model::{FxSource, BRL, USD};
use carteira_core::operations::{trades_of, PricedOperation};
use carteira_core::portfolio::positions_with_fx;
use carteira_core::savings::{self, SavingsKind, SavingsReport};
use carteira_storage::{FxTable, PriceTable, Vault};
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use rust_decimal::Decimal;
use serde::Serialize;

fn s<E: ToString>(e: E) -> String {
    e.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ColumnKind {
    /// 31/12 do ano anterior: o ponto de partida.
    Base,
    Month,
    Year,
    Yesterday,
    Today,
}

#[derive(Debug, Clone, Serialize)]
pub struct Column {
    pub date: NaiveDate,
    pub kind: ColumnKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// Código do ativo (vazio para saldos que não são investimento).
    pub asset: Option<String>,
    /// Saldo (conta, FGTS, benefício, cofrinho): dá para informar o saldo
    /// direto na tabela.
    pub savings_id: Option<String>,
    pub name: String,
    pub detail: Option<String>,
    /// Valor em reais em cada coluna; `None` quando não havia o ativo.
    pub values: Vec<Option<Decimal>>,
    /// Coluna em que havia o ativo, mas faltou cotação ou PTAX.
    pub missing: Vec<bool>,
    /// Resultado da primeira à última coluna.
    pub gain: Option<Gain>,
    /// Resultado desde a primeira compra até a última coluna (só
    /// investimentos).
    pub since: Option<Gain>,
}

/// Resultado no período, descontando compras e vendas: um ativo vendido no
/// meio do ano mostra o que rendeu até a venda, não uma queda de 100%.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Gain {
    /// Valor final + vendas − compras − valor inicial, em reais.
    pub value: Decimal,
    /// Valor inicial + compras: a base do percentual.
    pub base: Decimal,
}

impl std::ops::Add for Gain {
    type Output = Gain;
    fn add(self, o: Gain) -> Gain {
        Gain {
            value: self.value + o.value,
            base: self.base + o.base,
        }
    }
}

/// Soma dos resultados; sem resultado se faltou cotação em alguma linha.
fn sum_gains<'a>(rows: impl Iterator<Item = &'a Row>) -> Option<Gain> {
    sum_by(rows, |r| r.gain)
}

fn sum_since<'a>(rows: impl Iterator<Item = &'a Row>) -> Option<Gain> {
    sum_by(rows, |r| r.since)
}

fn sum_by<'a>(rows: impl Iterator<Item = &'a Row>, pick: fn(&Row) -> Option<Gain>) -> Option<Gain> {
    let mut acc = Gain::default();
    let mut any = false;
    for r in rows {
        if r.values.iter().all(Option::is_none) && pick(r).is_none() {
            continue;
        }
        acc = acc + pick(r)?;
        any = true;
    }
    any.then_some(acc)
}

#[derive(Debug, Clone, Serialize)]
pub struct Group {
    pub id: &'static str,
    pub label: String,
    /// Entra no "Total investimentos".
    pub investment: bool,
    pub rows: Vec<Row>,
    pub total: Vec<Option<Decimal>>,
    pub gain: Option<Gain>,
    pub since: Option<Gain>,
}

#[derive(Debug, Serialize)]
pub struct EvolutionView {
    /// Anos com patrimônio, do mais recente para o mais antigo.
    pub years: Vec<i32>,
    /// Ano escolhido; `None` = todos os anos.
    pub year: Option<i32>,
    pub columns: Vec<Column>,
    pub groups: Vec<Group>,
    pub total: Vec<Option<Decimal>>,
    pub investments: Vec<Option<Decimal>>,
    /// Aportes − resgates dos investimentos entre a coluna anterior e esta.
    pub new_money: Vec<Option<Decimal>>,
    /// Variação simples do patrimônio, da primeira à última coluna.
    pub total_gain: Option<Gain>,
    /// Resultado dos investimentos, descontando aportes e resgates.
    pub investments_gain: Option<Gain>,
    /// Resultado dos investimentos desde a primeira compra.
    pub investments_since: Option<Gain>,
    /// Alguma coluna ficou sem cotação ou PTAX de algum ativo.
    pub incomplete: bool,
}

fn month_end(y: i32, m: u32) -> NaiveDate {
    let next = if m == 12 {
        NaiveDate::from_ymd_opt(y + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(y, m + 1, 1)
    };
    next.expect("data válida") - Duration::days(1)
}

fn year_end(y: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, 12, 31).expect("31/12 existe")
}

fn previous_weekday(d: NaiveDate) -> NaiveDate {
    let mut p = d - Duration::days(1);
    while matches!(p.weekday(), Weekday::Sat | Weekday::Sun) {
        p -= Duration::days(1);
    }
    p
}

/// Colunas do período: um ano (31/12 anterior, fim de cada mês já fechado e,
/// no ano atual, ontem e hoje) ou todos os anos (31/12 de cada um e hoje).
pub fn columns(year: Option<i32>, first: NaiveDate, today: NaiveDate) -> Vec<Column> {
    let mut cols = Vec::new();
    match year {
        Some(y) => {
            if y > first.year() {
                cols.push(Column {
                    date: year_end(y - 1),
                    kind: ColumnKind::Base,
                });
            }
            let current = (y == today.year()).then(|| today.month());
            for m in 1..=12 {
                if current.is_some_and(|c| m >= c) {
                    break;
                }
                cols.push(Column {
                    date: month_end(y, m),
                    kind: ColumnKind::Month,
                });
            }
            if current.is_some() {
                // O mês corrente aparece como ontem e hoje.
                let yesterday = previous_weekday(today);
                if cols.last().is_none_or(|x| x.date < yesterday) {
                    cols.push(Column {
                        date: yesterday,
                        kind: ColumnKind::Yesterday,
                    });
                }
                cols.push(Column {
                    date: today,
                    kind: ColumnKind::Today,
                });
            }
        }
        None => {
            for y in first.year()..today.year() {
                cols.push(Column {
                    date: year_end(y),
                    kind: ColumnKind::Year,
                });
            }
            cols.push(Column {
                date: today,
                kind: ColumnKind::Today,
            });
        }
    }
    cols
}

/// Grupo de cada linha: fundos por banco (o FMP-FGTS também é fundo), B3
/// separada em ações/ETFs e FIIs, exterior, cripto, renda fixa, FGTS e
/// dinheiro.
fn group_of(kind: Option<&str>, currency: &str, account: &str) -> (&'static str, String) {
    match kind {
        Some("FUNDO") => ("fundos", format!("Fundos · {account}")),
        Some("CRIPTO") => ("cripto", "Criptomoedas".into()),
        _ if currency == USD => ("exterior", "Ações e ETFs no exterior".into()),
        Some("FII") => ("fii", "Fundos imobiliários (FIIs)".into()),
        _ => ("b3", "Ações e ETFs na B3".into()),
    }
}

const ORDER: [&str; 8] = [
    "fundos", "b3", "fii", "exterior", "cripto", "rf", "fgts", "dinheiro",
];

struct Inputs<'a> {
    ops: &'a [PricedOperation],
    table: &'a PriceTable,
    fx: &'a FxTable,
    reports: &'a [SavingsReport],
}

fn build(
    v: &Vault,
    inp: &Inputs,
    year: Option<i32>,
    years: Vec<i32>,
    first: NaiveDate,
    today: NaiveDate,
) -> Result<EvolutionView, String> {
    let cols = columns(year, first, today);
    let n = cols.len();
    let trades = trades_of(inp.ops);

    // Conta de cada ativo (a do último lançamento), para agrupar fundos por banco.
    let mut account: BTreeMap<&str, &str> = BTreeMap::new();
    for p in inp.ops {
        account.insert(&p.operation.asset, &p.operation.account);
    }

    let mut rows: BTreeMap<(&'static str, String), Vec<Row>> = BTreeMap::new();
    let mut by_asset: BTreeMap<String, Row> = BTreeMap::new();
    for (i, c) in cols.iter().enumerate() {
        for p in positions_with_fx(&trades, inp.table, Some(inp.fx), c.date) {
            if p.quantity.is_zero() || savings::is_savings(&p.asset) {
                continue;
            }
            let row = by_asset.entry(p.asset.clone()).or_insert_with(|| Row {
                asset: Some(p.asset.clone()),
                savings_id: None,
                name: p.asset.clone(),
                detail: None,
                values: vec![None; n],
                missing: vec![false; n],
                gain: None,
                since: None,
            });
            match p.market_value_brl {
                Some(val) => row.values[i] = Some(val),
                None => row.missing[i] = true,
            }
        }
    }
    // Ativo comprado e vendido entre duas colunas: não aparece em nenhuma,
    // mas o resultado dele entra no período.
    let (start, end) = (cols[0].date, cols[n - 1].date);
    for p in inp.ops {
        let a = &p.operation.asset;
        if p.operation.date > start && p.operation.date <= end && !savings::is_savings(a) {
            by_asset.entry(a.clone()).or_insert_with(|| Row {
                asset: Some(a.clone()),
                savings_id: None,
                name: a.clone(),
                detail: None,
                values: vec![None; n],
                missing: vec![false; n],
                gain: None,
                since: None,
            });
        }
    }
    for (asset, mut row) in by_asset {
        let info = v.get_asset(&asset).map_err(s)?;
        let kind = info.as_ref().map(|a| a.kind.clone());
        let currency = info
            .as_ref()
            .map(|a| a.currency.clone())
            .unwrap_or_else(|| BRL.into());
        let name = info.map(|a| a.name).unwrap_or_default();
        let acc = account.get(asset.as_str()).copied().unwrap_or("");
        if kind.as_deref() == Some("FUNDO") && !name.is_empty() {
            row.name = name.clone();
            row.detail = Some(asset.clone());
        } else if !name.is_empty() {
            row.detail = Some(name.clone());
        }
        let key = group_of(kind.as_deref(), &currency, acc);
        rows.entry(key).or_default().push(row);
    }

    // Saldos: cofrinhos em renda fixa; FGTS; conta e benefício em dinheiro.
    // Nenhum é estimado: vale o que o usuário informa na tabela.
    for r in inp.reports {
        let values: Vec<Option<Decimal>> = cols
            .iter()
            .map(|c| {
                // Só o saldo informado (ou movimentado) no próprio mês, sem
                // estimar nem repetir nos meses seguintes. Ontem e hoje
                // mostram o último saldo.
                let from = match c.kind {
                    ColumnKind::Yesterday | ColumnKind::Today => NaiveDate::MIN,
                    _ => c.date.with_day(1).expect("dia 1 existe"),
                };
                let last = r
                    .units_history
                    .iter()
                    .rev()
                    .find(|(d, _)| *d <= c.date && *d >= from)?;
                Some(r.balance_on(last.0))
            })
            .collect();
        // Saldo recém-cadastrado, ainda sem valor, aparece em branco para o
        // usuário informar os meses direto na tabela.
        let fresh = r.units_history.is_empty();
        if !fresh && values.iter().all(|x| x.is_none_or(|v| v.is_zero())) {
            continue;
        }
        let key = match r.savings.kind {
            SavingsKind::Cofrinho => ("rf", "Renda fixa (cofrinhos)".to_string()),
            SavingsKind::Fgts => ("fgts", "FGTS".to_string()),
            SavingsKind::Conta | SavingsKind::Beneficio => ("dinheiro", "Dinheiro".to_string()),
        };
        rows.entry(key).or_default().push(Row {
            asset: r.savings.kind.is_investment().then(|| r.ticker.clone()),
            savings_id: Some(r.savings.id.clone()),
            name: r.savings.name.clone(),
            // O nome já diz o tipo quando não foi preenchido ("Conta
            // corrente"); embaixo, só o banco.
            detail: Some(r.savings.account.clone()),
            values,
            missing: vec![false; n],
            gain: None,
            since: None,
        });
    }

    // Resultado de cada linha: valor final − inicial − (compras − vendas).
    let flow_brl = |p: &PricedOperation| -> Option<Decimal> {
        let t = p.trade()?;
        let cash = t.cash_flow();
        if t.currency == USD {
            inp.fx.usd_brl_on_or_before(t.date).map(|f| cash * f.rate)
        } else {
            Some(cash)
        }
    };
    let in_period = |p: &&PricedOperation| p.operation.date > start && p.operation.date <= end;
    for row in rows.values_mut().flatten() {
        if row.missing[0] || row.missing[n - 1] {
            continue;
        }
        let first = row.values[0].unwrap_or_default();
        let last = row.values[n - 1].unwrap_or_default();
        let (mut net, mut buys) = (Decimal::ZERO, Decimal::ZERO);
        if let Some(a) = &row.asset {
            for f in inp
                .ops
                .iter()
                .filter(in_period)
                .filter(|p| &p.operation.asset == a)
            {
                let Some(f) = flow_brl(f) else { continue };
                net += f;
                if f > Decimal::ZERO {
                    buys += f;
                }
            }
        }
        row.gain = Some(Gain {
            value: (last - first - net).round_dp(2),
            base: (first + buys).round_dp(2),
        });
    }
    // Desde o início: valor final + todas as vendas − todas as compras até a
    // última coluna; a base é o total comprado.
    for row in rows.values_mut().flatten() {
        let Some(a) = &row.asset else { continue };
        if row.missing[n - 1] {
            continue;
        }
        let (mut net, mut buys) = (Decimal::ZERO, Decimal::ZERO);
        for p in inp
            .ops
            .iter()
            .filter(|p| p.operation.date <= end && &p.operation.asset == a)
        {
            let Some(f) = flow_brl(p) else { continue };
            net += f;
            if f > Decimal::ZERO {
                buys += f;
            }
        }
        if buys.is_zero() {
            continue;
        }
        let last = row.values[n - 1].unwrap_or_default();
        row.since = Some(Gain {
            value: (last - net).round_dp(2),
            base: buys.round_dp(2),
        });
    }

    let mut groups: Vec<Group> = rows
        .into_iter()
        .map(|((id, label), mut rows)| {
            rows.sort_by(|a, b| a.name.cmp(&b.name));
            if year.is_none() {
                rows = merge_sold(rows, n);
            }
            let total = (0..n)
                .map(|i| {
                    let vals: Vec<Decimal> = rows.iter().filter_map(|r| r.values[i]).collect();
                    (!vals.is_empty()).then(|| vals.iter().sum())
                })
                .collect();
            Group {
                id,
                investment: rows.iter().all(|r| r.asset.is_some()),
                gain: sum_gains(rows.iter()),
                since: sum_since(rows.iter()),
                label,
                rows,
                total,
            }
        })
        .collect();
    groups.sort_by_key(|g| {
        (
            ORDER.iter().position(|o| *o == g.id).unwrap_or(9),
            g.label.clone(),
        )
    });

    let sum = |pick: &dyn Fn(&Group, &Row) -> bool| -> Vec<Option<Decimal>> {
        (0..n)
            .map(|i| {
                let vals: Vec<Decimal> = groups
                    .iter()
                    .flat_map(|g| g.rows.iter().filter(move |r| pick(g, r)))
                    .filter_map(|r| r.values[i])
                    .collect();
                (!vals.is_empty()).then(|| vals.iter().sum())
            })
            .collect()
    };
    let total = sum(&|_, _| true);
    // Investimento é tudo que tem código no motor: ativos e cofrinhos (e o
    // FMP-FGTS); FGTS, conta e benefício não têm.
    let investments = sum(&|_, r| r.asset.is_some());
    let all_rows = || groups.iter().flat_map(|g| g.rows.iter());
    // No total geral, o resultado é a variação simples do patrimônio: dinheiro
    // que sai da conta e vira investimento se anula, e só o investimento
    // registra compras e vendas.
    let total_gain = match (total[0], total[n - 1]) {
        (None, None) => None,
        (a, b) => {
            let a = a.unwrap_or_default();
            Some(Gain {
                value: b.unwrap_or_default() - a,
                base: a,
            })
        }
    };
    let investments_gain = sum_gains(all_rows().filter(|r| r.asset.is_some()));
    let investments_since = sum_since(all_rows().filter(|r| r.asset.is_some()));

    // Dinheiro novo nos investimentos entre uma coluna e a anterior.
    let new_money = (0..n)
        .map(|i| {
            let empty = |j: usize| investments[j].is_none();
            if i == 0 || (empty(i) && empty(i - 1)) {
                return None;
            }
            let (from, to) = (cols[i - 1].date, cols[i].date);
            let flows = inp
                .ops
                .iter()
                .filter(|p| p.operation.date > from && p.operation.date <= to)
                .filter_map(flow_brl);
            Some(flows.sum::<Decimal>().round_dp(2))
        })
        .collect();

    let incomplete = groups
        .iter()
        .any(|g| g.rows.iter().any(|r| r.missing.iter().any(|m| *m)));
    Ok(EvolutionView {
        years,
        year,
        columns: cols,
        groups,
        total,
        investments,
        new_money,
        total_gain,
        investments_gain,
        investments_since,
        incomplete,
    })
}

/// Em "Todos os anos", os ativos que já não estão na carteira viram uma
/// linha só por grupo: os totais dos anos passados continuam completos sem
/// listar o que foi vendido.
fn merge_sold(rows: Vec<Row>, n: usize) -> Vec<Row> {
    let (held, sold): (Vec<Row>, Vec<Row>) = rows
        .into_iter()
        .partition(|r| r.savings_id.is_some() || r.values[n - 1].is_some());
    if sold.is_empty() {
        return held;
    }
    let values = (0..n)
        .map(|i| {
            let vals: Vec<Decimal> = sold.iter().filter_map(|r| r.values[i]).collect();
            (!vals.is_empty()).then(|| vals.iter().sum())
        })
        .collect();
    let merged = Row {
        asset: Some(String::new()),
        savings_id: None,
        name: "Já vendidos".into(),
        detail: Some(match sold.len() {
            1 => "1 ativo".into(),
            k => format!("{k} ativos"),
        }),
        values,
        missing: (0..n).map(|i| sold.iter().any(|r| r.missing[i])).collect(),
        gain: sum_gains(sold.iter()),
        since: sum_since(sold.iter()),
    };
    held.into_iter().chain(std::iter::once(merged)).collect()
}

/// Primeiro dia com patrimônio (lançamento ou saldo).
fn first_day(ops: &[PricedOperation], reports: &[SavingsReport]) -> Option<NaiveDate> {
    ops.iter()
        .map(|p| p.operation.date)
        .chain(
            reports
                .iter()
                .filter_map(|r| r.units_history.first().map(|(d, _)| *d)),
        )
        .min()
}

pub fn evolution(v: &Vault, year: Option<i32>, today: NaiveDate) -> Result<EvolutionView, String> {
    let all = crate::views::engine_operations(v)?;
    let (ops, _) =
        carteira_core::operations::price_operations(&carteira_core::fees::default_rules(), &all)
            .map_err(s)?;
    let reports = crate::views::savings_reports(v)?;
    let Some(first) = first_day(&ops, &reports) else {
        return Ok(EvolutionView {
            years: Vec::new(),
            year,
            columns: Vec::new(),
            groups: Vec::new(),
            total: Vec::new(),
            investments: Vec::new(),
            new_money: Vec::new(),
            total_gain: None,
            investments_gain: None,
            investments_since: None,
            incomplete: false,
        });
    };
    let years: Vec<i32> = (first.year()..=today.year()).rev().collect();
    let year = year.filter(|y| years.contains(y));
    let tickers: Vec<String> = ops
        .iter()
        .map(|p| p.operation.asset.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let table = v.price_table(&tickers).map_err(s)?;
    let fx = v.fx_table().map_err(s)?;
    let inp = Inputs {
        ops: &ops,
        table: &table,
        fx: &fx,
        reports: &reports,
    };
    build(v, &inp, year, years, first, today)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn colunas_do_ano_atual_e_de_todos_os_anos() {
        // Sexta, 16/10/2026: ontem é quinta, 15/10.
        let c = columns(Some(2026), d(2024, 3, 1), d(2026, 10, 16));
        assert_eq!(c[0].kind, ColumnKind::Base);
        assert_eq!(c[0].date, d(2025, 12, 31));
        assert_eq!(c[9].date, d(2026, 9, 30));
        assert_eq!(c[10].kind, ColumnKind::Yesterday);
        assert_eq!(c[10].date, d(2026, 10, 15));
        assert_eq!(c[11].kind, ColumnKind::Today);
        // Ano atual vai só até o mês corrente.
        assert_eq!(c.len(), 12);
        // Segunda: ontem é a sexta anterior.
        let c = columns(Some(2026), d(2024, 3, 1), d(2026, 10, 19));
        assert_eq!(c[10].date, d(2026, 10, 16));
        // Ano passado: 31/12 anterior e os 12 meses.
        let c = columns(Some(2025), d(2024, 3, 1), d(2026, 10, 16));
        assert_eq!(c.len(), 13);
        // Primeiro ano: sem base, mas com os 12 meses (antes do início, em branco).
        let c = columns(Some(2024), d(2024, 3, 1), d(2026, 10, 16));
        assert_eq!(c[0].date, d(2024, 1, 31));
        assert_eq!(c.len(), 12);
        let c = columns(None, d(2024, 3, 1), d(2026, 10, 16));
        let dates: Vec<_> = c.iter().map(|x| x.date).collect();
        assert_eq!(
            dates,
            vec![d(2024, 12, 31), d(2025, 12, 31), d(2026, 10, 16)]
        );
    }

    #[test]
    fn grupos_dos_ativos() {
        // O FMP-FGTS é fundo de investimento, no banco onde foi aplicado.
        assert_eq!(
            group_of(Some("FUNDO"), BRL, "Banco A").1,
            "Fundos · Banco A"
        );
        assert_eq!(group_of(Some("ETF"), USD, "").0, "exterior");
        assert_eq!(group_of(Some("CRIPTO"), USD, "").0, "cripto");
        assert_eq!(group_of(Some("FII"), BRL, "").0, "fii");
        assert_eq!(group_of(Some("ACAO"), BRL, "").0, "b3");
        assert_eq!(group_of(None, BRL, "").0, "b3");
    }

    #[test]
    fn patrimonio_separa_investimentos_e_dinheiro_novo() {
        use carteira_core::model::Side;
        use carteira_core::operations::Operation;
        use carteira_core::savings::{MoveKind, Savings, SavingsMove};
        use carteira_storage::{KdfParams, NewQuote};
        use rust_decimal_macros::dec;

        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.11.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        let trade = |asset: &str, date, side, q, price| Operation {
            id: String::new(),
            account: "Corretora X".into(),
            asset: asset.into(),
            date,
            side,
            quantity: q,
            price,
            manual_fees: Some(Decimal::ZERO),
            note: None,
            currency: BRL.into(),
            from_account: None,
            residue: None,
            ir: None,
            iof: None,
        };
        let buy = |date, q| trade("TEST3", date, Side::Compra, q, dec!(10));
        v.create_operation(buy(d(2026, 1, 5), dec!(100))).unwrap();
        v.create_operation(buy(d(2026, 2, 10), dec!(50))).unwrap();
        // OUTR3: comprada em janeiro e vendida toda em 15/02 por R$ 130.
        v.create_operation(trade(
            "OUTR3",
            d(2026, 1, 5),
            Side::Compra,
            dec!(10),
            dec!(10),
        ))
        .unwrap();
        v.create_operation(trade(
            "OUTR3",
            d(2026, 2, 15),
            Side::Venda,
            dec!(10),
            dec!(13),
        ))
        .unwrap();
        v.save_quotes(&[NewQuote {
            ticker: "OUTR3".into(),
            date: d(2026, 1, 30),
            close: dec!(11),
            source: "YAHOO".into(),
        }])
        .unwrap();
        let q = |date, close| NewQuote {
            ticker: "TEST3".into(),
            date,
            close,
            source: "YAHOO".into(),
        };
        v.save_quotes(&[
            q(d(2026, 1, 5), dec!(10)),
            q(d(2026, 1, 30), dec!(11)),
            q(d(2026, 2, 27), dec!(12)),
        ])
        .unwrap();
        let conta = v
            .create_savings(Savings {
                id: String::new(),
                kind: SavingsKind::Conta,
                account: "Banco X".into(),
                name: "Conta".into(),
                rate_pct: Decimal::ZERO,
                tax_free: false,
                note: None,
            })
            .unwrap();
        v.create_savings_move(SavingsMove {
            id: String::new(),
            savings_id: conta.id.clone(),
            date: d(2026, 1, 10),
            kind: MoveKind::Deposito,
            amount: dec!(500),
            ir: None,
            iof: None,
            note: None,
        })
        .unwrap();

        let view = evolution(&v, Some(2026), d(2026, 3, 4)).unwrap();
        assert_eq!(view.years, vec![2026]);
        // Primeiro ano, ano atual: jan, fev, ontem (03/03) e hoje.
        assert_eq!(view.columns.len(), 4);
        let ids: Vec<_> = view.groups.iter().map(|g| g.id).collect();
        assert_eq!(ids, vec!["b3", "dinheiro"]);
        assert!(view.groups[0].investment);
        assert!(!view.groups[1].investment);
        // Janeiro: 100 × 11 + 10 × 11 + 500 na conta.
        assert_eq!(view.investments[0], Some(dec!(1210)));
        assert_eq!(view.total[0], Some(dec!(1710)));
        // Fevereiro: 150 × 12; a OUTR3 vendida não aparece mais (o dinheiro
        // foi para outro lugar). Entraram 500 e saíram 130.
        let rv = &view.groups[0];
        let outr = rv
            .rows
            .iter()
            .find(|r| r.asset.as_deref() == Some("OUTR3"))
            .unwrap();
        assert_eq!(outr.values[1], None);
        assert_eq!(view.investments[1], Some(dec!(1800)));
        assert_eq!(view.new_money[1], Some(dec!(370)));
        assert_eq!(view.new_money[0], None);
        // No período, a venda conta: 130 recebidos − 110 de janeiro = +20,
        // não uma queda de 100%.
        let g = outr.gain.unwrap();
        assert_eq!((g.value, g.base), (dec!(20), dec!(110)));
        // Desde o início: 130 recebidos − 100 pagos na compra.
        let since = outr.since.unwrap();
        assert_eq!((since.value, since.base), (dec!(30), dec!(100)));
        // TEST3: valor de hoje − 1.500 comprados; a conta não tem.
        let test = rv
            .rows
            .iter()
            .find(|r| r.asset.as_deref() == Some("TEST3"))
            .unwrap();
        let today = test.values[view.columns.len() - 1].unwrap();
        assert_eq!(test.since.unwrap().value, today - dec!(1500));
        assert!(view.groups[1].since.is_none());
        assert_eq!(
            view.investments_since.unwrap().value,
            today - dec!(1500) + dec!(30)
        );
        // O resultado dos investimentos bate com a soma dos rendimentos.
        let inv = view.investments_gain.unwrap();
        let last = view.investments[view.columns.len() - 1].unwrap();
        let flows: Decimal = view.new_money.iter().flatten().sum();
        assert_eq!(inv.value, last - dec!(1210) - flows);
        let total = view.total_gain.unwrap();
        assert_eq!(
            total.value,
            view.total[view.columns.len() - 1].unwrap() - dec!(1710)
        );
        // Conta: o saldo de janeiro não se repete em fevereiro; hoje mostra o
        // último saldo informado.
        let conta = &view.groups[1].rows[0];
        assert_eq!(conta.values[0], Some(dec!(500)));
        assert_eq!(conta.values[1], None);
        assert_eq!(conta.values[3], Some(dec!(500)));
        assert!(!view.incomplete);

        let all = evolution(&v, None, d(2026, 3, 4)).unwrap();
        assert_eq!(all.year, None);
        assert_eq!(all.columns.len(), 1);
        // OUTR3 foi vendida antes do período: não aparece.
        let names: Vec<_> = all.groups[0].rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["TEST3"]);
    }

    #[test]
    fn todos_os_anos_junta_os_vendidos() {
        use rust_decimal_macros::dec;
        let row = |name: &str, values: Vec<Option<Decimal>>, gain| Row {
            asset: Some(name.into()),
            savings_id: None,
            name: name.into(),
            detail: None,
            missing: vec![false; values.len()],
            values,
            gain: Some(Gain {
                value: gain,
                base: dec!(100),
            }),
            since: None,
        };
        let rows = vec![
            row("AAAA3", vec![Some(dec!(100)), Some(dec!(120))], dec!(20)),
            row("BBBB3", vec![Some(dec!(100)), None], dec!(5)),
            row("CCCC3", vec![Some(dec!(50)), None], dec!(-3)),
        ];
        let merged = merge_sold(rows, 2);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].name, "Já vendidos");
        assert_eq!(merged[1].detail.as_deref(), Some("2 ativos"));
        assert_eq!(merged[1].values, vec![Some(dec!(150)), None]);
        assert_eq!(merged[1].gain.unwrap().value, dec!(2));
    }
}
