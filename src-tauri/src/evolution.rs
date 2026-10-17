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
    /// Mês que ainda não chegou: coluna em branco, para o ano ter os 12 meses.
    Future,
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
    pub name: String,
    pub detail: Option<String>,
    /// Valor em reais em cada coluna; `None` quando não havia o ativo.
    pub values: Vec<Option<Decimal>>,
    /// Coluna em que havia o ativo, mas faltou cotação ou PTAX.
    pub missing: Vec<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Group {
    pub id: &'static str,
    pub label: String,
    /// Entra no "Total investimentos".
    pub investment: bool,
    pub rows: Vec<Row>,
    pub total: Vec<Option<Decimal>>,
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
            if let Some(c) = current {
                // O mês corrente aparece como ontem e hoje; os seguintes, em branco.
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
                for m in c + 1..=12 {
                    cols.push(Column {
                        date: month_end(y, m),
                        kind: ColumnKind::Future,
                    });
                }
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

/// Grupo de cada linha, como na planilha.
fn group_of(
    kind: Option<&str>,
    currency: &str,
    name: &str,
    account: &str,
) -> (&'static str, String) {
    match kind {
        Some("FUNDO") if name.to_uppercase().contains("FGTS") => ("fgts", "FGTS".into()),
        Some("FUNDO") => ("fundos", format!("Fundos · {account}")),
        Some("CRIPTO") => ("rv", "Renda variável".into()),
        _ if currency == USD => ("exterior", "ETFs e ações no exterior".into()),
        _ => ("rv", "Renda variável".into()),
    }
}

const ORDER: [&str; 6] = ["fundos", "exterior", "rv", "rf", "fgts", "dinheiro"];

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
        if c.kind == ColumnKind::Future {
            continue;
        }
        for p in positions_with_fx(&trades, inp.table, Some(inp.fx), c.date) {
            if p.quantity.is_zero() || savings::is_savings(&p.asset) {
                continue;
            }
            let row = by_asset.entry(p.asset.clone()).or_insert_with(|| Row {
                asset: Some(p.asset.clone()),
                name: p.asset.clone(),
                detail: None,
                values: vec![None; n],
                missing: vec![false; n],
            });
            match p.market_value_brl {
                Some(val) => row.values[i] = Some(val),
                None => row.missing[i] = true,
            }
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
        let key = group_of(kind.as_deref(), &currency, &name, acc);
        rows.entry(key).or_default().push(row);
    }

    // Saldos: cofrinhos em renda fixa; FGTS; conta e benefício em dinheiro.
    for r in inp.reports {
        let values: Vec<Option<Decimal>> = cols
            .iter()
            .map(|c| {
                if c.kind == ColumnKind::Future {
                    return None;
                }
                let started = r.units_history.first().is_some_and(|(d, _)| *d <= c.date);
                started.then(|| r.balance_on(c.date))
            })
            .collect();
        if values.iter().all(|x| x.is_none_or(|v| v.is_zero())) {
            continue;
        }
        let key = match r.savings.kind {
            SavingsKind::Cofrinho => ("rf", "Renda fixa".to_string()),
            SavingsKind::Fgts => ("fgts", "FGTS".to_string()),
            SavingsKind::Conta | SavingsKind::Beneficio => ("dinheiro", "Dinheiro".to_string()),
        };
        rows.entry(key).or_default().push(Row {
            asset: r.savings.kind.is_investment().then(|| r.ticker.clone()),
            name: r.savings.name.clone(),
            detail: Some(format!(
                "{} · {}",
                r.savings.kind.label(),
                r.savings.account
            )),
            values,
            missing: vec![false; n],
        });
    }

    let mut groups: Vec<Group> = rows
        .into_iter()
        .map(|((id, label), mut rows)| {
            rows.sort_by(|a, b| a.name.cmp(&b.name));
            let total = (0..n)
                .map(|i| {
                    let vals: Vec<Decimal> = rows.iter().filter_map(|r| r.values[i]).collect();
                    (!vals.is_empty()).then(|| vals.iter().sum())
                })
                .collect();
            Group {
                id,
                investment: rows.iter().all(|r| r.asset.is_some()),
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

    // Dinheiro novo nos investimentos entre uma coluna e a anterior.
    let flow_brl = |p: &PricedOperation| -> Option<Decimal> {
        let t = p.trade()?;
        let cash = t.cash_flow();
        if t.currency == USD {
            inp.fx.usd_brl_on_or_before(t.date).map(|f| cash * f.rate)
        } else {
            Some(cash)
        }
    };
    let new_money = (0..n)
        .map(|i| {
            let empty = |j: usize| investments[j].is_none();
            if i == 0 || cols[i].kind == ColumnKind::Future || (empty(i) && empty(i - 1)) {
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
        incomplete,
    })
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
        // Novembro e dezembro em branco: o ano sempre tem os 12 meses.
        assert_eq!(c.len(), 14);
        assert_eq!(c[13].kind, ColumnKind::Future);
        assert_eq!(c[13].date, d(2026, 12, 31));
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
    fn grupos_como_na_planilha() {
        assert_eq!(
            group_of(Some("FUNDO"), BRL, "FMP - FGTS EMPRESA X", "Banco A").0,
            "fgts"
        );
        assert_eq!(
            group_of(Some("FUNDO"), BRL, "FUNDO TESTE", "Banco A").1,
            "Fundos · Banco A"
        );
        assert_eq!(group_of(Some("ETF"), USD, "", "").0, "exterior");
        assert_eq!(group_of(Some("CRIPTO"), USD, "", "").0, "rv");
        assert_eq!(group_of(Some("ACAO"), BRL, "", "").0, "rv");
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
        let buy = |date, q| Operation {
            id: String::new(),
            account: "Corretora X".into(),
            asset: "TEST3".into(),
            date,
            side: Side::Compra,
            quantity: q,
            price: dec!(10),
            manual_fees: Some(Decimal::ZERO),
            note: None,
            currency: BRL.into(),
            from_account: None,
            residue: None,
            ir: None,
            iof: None,
        };
        v.create_operation(buy(d(2026, 1, 5), dec!(100))).unwrap();
        v.create_operation(buy(d(2026, 2, 10), dec!(50))).unwrap();
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
        // Primeiro ano: jan, fev, ontem (03/03), hoje e abr a dez em branco.
        assert_eq!(view.columns.len(), 13);
        assert_eq!(view.total[12], None);
        assert_eq!(view.new_money[12], None);
        let ids: Vec<_> = view.groups.iter().map(|g| g.id).collect();
        assert_eq!(ids, vec!["rv", "dinheiro"]);
        assert!(view.groups[0].investment);
        assert!(!view.groups[1].investment);
        // Janeiro: 100 × 11 + 500 na conta.
        assert_eq!(view.investments[0], Some(dec!(1100)));
        assert_eq!(view.total[0], Some(dec!(1600)));
        // Fevereiro: 150 × 12; entraram R$ 500 novos.
        assert_eq!(view.investments[1], Some(dec!(1800)));
        assert_eq!(view.new_money[1], Some(dec!(500)));
        assert_eq!(view.new_money[0], None);
        assert!(!view.incomplete);

        let all = evolution(&v, None, d(2026, 3, 4)).unwrap();
        assert_eq!(all.year, None);
        assert_eq!(all.columns.len(), 1);
    }
}
