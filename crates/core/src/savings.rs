//! Cofrinhos (caixinhas): renda fixa com resgate a qualquer hora, que rende um
//! percentual do CDI que o banco muda quando quer.
//!
//! O que vale é o saldo que o banco mostra. O usuário lança depósitos e
//! resgates e informa o saldo na Evolução, mês a mês. O app não estima nada
//! entre dois saldos (nem CDI, nem IR): o rendimento aparece quando o saldo
//! novo é informado. Assim o rendimento de um período é sempre `saldo final −
//! saldo inicial − depósitos + resgates`, qualquer que tenha sido a taxa.
//!
//! Para entrar na carteira, na rentabilidade e no Painel como qualquer outro
//! ativo, o cofrinho vira uma "cota" de R$ 1: depósito é compra de cotas,
//! resgate é venda e o saldo informado ajusta a quantidade de cotas sem
//! dinheiro entrar ([`Side::Ajuste`]).

use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::model::{Side, BRL};
use crate::operations::Operation;

/// Prefixo do código do cofrinho na carteira: "RF:<id>".
pub const PREFIX: &str = "RF:";
pub const KIND: &str = "RENDA_FIXA";
pub const SOURCE: &str = "CALCULADO";

pub fn is_savings(asset: &str) -> bool {
    asset.starts_with(PREFIX)
}

pub fn ticker(id: &str) -> String {
    format!("{PREFIX}{}", id.to_uppercase())
}

/// Tipo de saldo. Só o cofrinho é investimento: os outros aparecem no
/// patrimônio, em "Outros saldos", mas ficam fora da rentabilidade e dos
/// impostos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SavingsKind {
    /// Cofrinho, caixinha, conta remunerada: rende um % do CDI.
    #[default]
    Cofrinho,
    /// FGTS: 3% ao ano estimados; TR e distribuição de lucros entram pelo
    /// saldo informado.
    Fgts,
    /// Conta corrente: sem rendimento.
    Conta,
    /// Cartão benefício (VR, VA): sem rendimento.
    Beneficio,
}

impl SavingsKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SavingsKind::Cofrinho => "COFRINHO",
            SavingsKind::Fgts => "FGTS",
            SavingsKind::Conta => "CONTA",
            SavingsKind::Beneficio => "BENEFICIO",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "COFRINHO" => SavingsKind::Cofrinho,
            "FGTS" => SavingsKind::Fgts,
            "CONTA" => SavingsKind::Conta,
            "BENEFICIO" => SavingsKind::Beneficio,
            _ => return None,
        })
    }

    pub fn is_investment(self) -> bool {
        self == SavingsKind::Cofrinho
    }

    pub fn label(self) -> &'static str {
        match self {
            SavingsKind::Cofrinho => "Cofrinho",
            SavingsKind::Fgts => "FGTS",
            SavingsKind::Conta => "Conta corrente",
            SavingsKind::Beneficio => "Cartão benefício",
        }
    }
}

/// Saldo cadastrado (cofrinho, FGTS, conta, benefício).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Savings {
    pub id: String,
    #[serde(default)]
    pub kind: SavingsKind,
    pub account: String,
    pub name: String,
    /// Percentual do CDI usado só para estimar entre dois saldos informados.
    pub rate_pct: Decimal,
    /// LCI, LCA e afins: sem IR.
    #[serde(default)]
    pub tax_free: bool,
    pub note: Option<String>,
    /// Desde quando existe (dia 1 do mês): a Evolução mostra a linha a partir
    /// daí.
    #[serde(default)]
    pub since: Option<NaiveDate>,
    /// Mês de encerramento (dia 1), para conta fechada: depois dele o saldo
    /// some e não conta mais no total.
    #[serde(default)]
    pub until: Option<NaiveDate>,
}

impl Savings {
    /// Existia em algum momento entre `from` e `to`.
    pub fn active_between(&self, from: NaiveDate, to: NaiveDate) -> bool {
        self.since.is_none_or(|s| s <= to) && self.until.is_none_or(|u| last_day(u) >= from)
    }

    /// Ainda aberto em `date` (não passou do mês de encerramento).
    pub fn open_on(&self, date: NaiveDate) -> bool {
        self.until.is_none_or(|u| date <= last_day(u))
    }

    /// Nome dado pelo usuário. Em branco (ou igual ao tipo, como os cadastros
    /// antigos ficavam) não há nome próprio: o tipo e o banco já dizem tudo.
    pub fn own_name(&self) -> Option<&str> {
        let n = self.name.trim();
        (!n.is_empty() && n != self.kind.label()).then_some(n)
    }

    /// Banco ou empresa; o FGTS não tem (cadastros antigos podem ter guardado
    /// um).
    pub fn bank(&self) -> &str {
        if self.kind == SavingsKind::Fgts {
            ""
        } else {
            &self.account
        }
    }

    /// Nome para listas: o próprio ou o tipo ("Conta corrente").
    pub fn title(&self) -> &str {
        self.own_name().unwrap_or(self.kind.label())
    }

    pub fn validate(&self) -> Result<(), String> {
        if let (Some(s), Some(u)) = (self.since, self.until) {
            if u < s {
                return Err("o mês de encerramento é antes do início".into());
            }
        }
        if self.account.trim().is_empty() && self.kind != SavingsKind::Fgts {
            return Err("informe o banco (conta)".into());
        }
        if self.kind == SavingsKind::Cofrinho
            && (self.rate_pct <= Decimal::ZERO || self.rate_pct > Decimal::from(300))
        {
            return Err("a taxa precisa estar entre 0% e 300% do CDI".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MoveKind {
    /// Dinheiro que entrou no cofrinho.
    Deposito,
    /// Dinheiro que voltou para a conta (valor líquido, já sem IR e IOF).
    Resgate,
    /// Saldo que o banco mostra no fim do dia.
    Saldo,
}

impl MoveKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MoveKind::Deposito => "DEPOSITO",
            MoveKind::Resgate => "RESGATE",
            MoveKind::Saldo => "SALDO",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "DEPOSITO" => MoveKind::Deposito,
            "RESGATE" => MoveKind::Resgate,
            "SALDO" => MoveKind::Saldo,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavingsMove {
    pub id: String,
    pub savings_id: String,
    pub date: NaiveDate,
    pub kind: MoveKind,
    /// Depósito: valor depositado. Resgate: valor que caiu na conta. Saldo:
    /// saldo bruto mostrado pelo banco.
    pub amount: Decimal,
    /// Resgate: IR e IOF retidos, quando o banco mostrar. Vazios: estimados.
    #[serde(default)]
    pub ir: Option<Decimal>,
    #[serde(default)]
    pub iof: Option<Decimal>,
    pub note: Option<String>,
}

impl SavingsMove {
    pub fn validate(&self) -> Result<(), String> {
        if self.kind == MoveKind::Saldo {
            if self.amount < Decimal::ZERO {
                return Err("o saldo não pode ser negativo".into());
            }
        } else if self.amount <= Decimal::ZERO {
            return Err("o valor precisa ser maior que zero".into());
        }
        if self.ir.is_some_and(|v| v < Decimal::ZERO) || self.iof.is_some_and(|v| v < Decimal::ZERO)
        {
            return Err("IR e IOF não podem ser negativos".into());
        }
        if self.kind != MoveKind::Resgate && (self.ir.is_some() || self.iof.is_some()) {
            return Err("IR e IOF só existem no resgate".into());
        }
        Ok(())
    }
}

/// IR da renda fixa pela tabela regressiva, conforme os dias aplicados.
pub fn ir_rate(days: i64) -> Decimal {
    match days {
        ..=180 => Decimal::new(225, 3),
        181..=360 => Decimal::new(20, 2),
        361..=720 => Decimal::new(175, 3),
        _ => Decimal::new(15, 2),
    }
}

/// IOF sobre o rendimento nos primeiros 29 dias (tabela do Decreto 6.306).
pub fn iof_rate(days: i64) -> Decimal {
    const TABLE: [u32; 30] = [
        100, 96, 93, 90, 86, 83, 80, 76, 73, 70, 66, 63, 60, 56, 53, 50, 46, 43, 40, 36, 33, 30,
        26, 23, 20, 16, 13, 10, 6, 3,
    ];
    if (0..30).contains(&days) {
        Decimal::new(TABLE[days as usize] as i64, 2)
    } else {
        Decimal::ZERO
    }
}

/// Lote de um depósito: cotas que ainda estão no cofrinho e o custo delas.
#[derive(Debug, Clone)]
struct Lot {
    units: Decimal,
    cost: Decimal,
}

fn take(lots: &mut Vec<Lot>, units: Decimal) {
    let mut left = units;
    for lot in lots.iter_mut() {
        if left <= Decimal::ZERO {
            break;
        }
        let t = left.min(lot.units);
        if !lot.units.is_zero() {
            lot.cost -= lot.cost * t / lot.units;
        }
        lot.units -= t;
        left -= t;
    }
    lots.retain(|l| !crate::model::is_dust(l.units));
}

/// Ajuste de saldo: as cotas de todos os lotes mudam na mesma proporção, sem
/// mexer no custo.
fn scale(lots: &mut [Lot], factor: Decimal) {
    for lot in lots {
        lot.units *= factor;
    }
}

fn cents(v: Decimal) -> Decimal {
    v.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Resultado de um saldo informado: quanto o app esperava e a diferença.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BalanceCheck {
    pub move_id: String,
    pub date: NaiveDate,
    pub informed: Decimal,
    pub expected: Decimal,
    pub difference: Decimal,
    /// Diferença grande demais para ser só a taxa: deve faltar um movimento.
    pub suspicious: bool,
}

/// Resgate com o imposto usado (informado ou estimado).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Redemption {
    pub move_id: String,
    pub date: NaiveDate,
    pub net: Decimal,
    pub gross: Decimal,
    pub ir: Decimal,
    pub iof: Decimal,
    pub estimated: bool,
    /// Rendimento resgatado (bruto − custo das cotas resgatadas).
    pub gain: Decimal,
}

/// Tudo que se sabe de um cofrinho até `today`.
#[derive(Debug, Clone, Serialize)]
pub struct SavingsReport {
    pub savings: Savings,
    pub ticker: String,
    /// Valor da cota por dia (R$ 1 no primeiro depósito).
    pub unit_values: Vec<(NaiveDate, Decimal)>,
    /// Operações equivalentes, para a carteira e a rentabilidade.
    pub operations: Vec<Operation>,
    /// Saldo hoje (o último informado, mais depósitos e menos resgates
    /// depois dele); `balance_net` é igual, sem IR estimado.
    pub balance: Decimal,
    pub balance_net: Decimal,
    /// Quanto do saldo é dinheiro depositado (custo das cotas).
    pub invested: Decimal,
    pub last_informed: Option<NaiveDate>,
    pub checks: Vec<BalanceCheck>,
    pub redemptions: Vec<Redemption>,
    /// Saldo bruto em 31/12 de cada ano (para Bens e Direitos).
    pub year_end: Vec<(i32, Decimal)>,
    /// Cotas no fim de cada dia com movimento, para o saldo em qualquer data.
    pub units_history: Vec<(NaiveDate, Decimal)>,
}

impl SavingsReport {
    /// Saldo bruto estimado no fim de `date` (zero antes do primeiro movimento).
    pub fn balance_on(&self, date: NaiveDate) -> Decimal {
        let Some((_, units)) = self.units_history.iter().rev().find(|(d, _)| *d <= date) else {
            return Decimal::ZERO;
        };
        let unit = self
            .unit_values
            .iter()
            .rev()
            .find(|(d, _)| *d <= date)
            .map(|(_, u)| *u)
            .unwrap_or(Decimal::ONE);
        cents(*units * unit)
    }
}

/// Monta o saldo a partir dos movimentos e dos saldos informados.
pub fn build(savings: &Savings, moves: &[SavingsMove], today: NaiveDate) -> SavingsReport {
    let ticker = ticker(&savings.id);
    // Conta encerrada: o saldo some no dia seguinte ao fim do mês do "até".
    let closing = savings
        .until
        .and_then(|u| last_day(u).succ_opt())
        .filter(|c| *c <= today);
    let mut moves: Vec<&SavingsMove> = moves
        .iter()
        .filter(|m| m.date <= today && closing.is_none_or(|c| m.date < c))
        .collect();
    // No mesmo dia: depósitos, depois resgates, depois o saldo do fim do dia.
    moves.sort_by_key(|m| {
        (
            m.date,
            match m.kind {
                MoveKind::Deposito => 0,
                MoveKind::Resgate => 1,
                MoveKind::Saldo => 2,
            },
            m.id.clone(),
        )
    });
    let mut report = SavingsReport {
        savings: savings.clone(),
        ticker: ticker.clone(),
        unit_values: Vec::new(),
        operations: Vec::new(),
        balance: Decimal::ZERO,
        balance_net: Decimal::ZERO,
        invested: Decimal::ZERO,
        last_informed: None,
        checks: Vec::new(),
        redemptions: Vec::new(),
        year_end: Vec::new(),
        units_history: Vec::new(),
    };
    let Some(first) = moves.first().map(|m| m.date) else {
        return report;
    };

    // Valor da cota: R$ 1, sempre; nada é estimado entre um saldo e outro.
    let unit = Decimal::ONE;
    let mut values = vec![(first, unit)];
    if values.last().map(|(d, _)| *d) != Some(today) {
        values.push((today, unit));
    }
    let unit_on = |d: NaiveDate| -> Decimal {
        values
            .iter()
            .rev()
            .find(|(x, _)| *x <= d)
            .map(|(_, u)| *u)
            .unwrap_or(Decimal::ONE)
    };

    let mut lots: Vec<Lot> = Vec::new();
    let units = |lots: &[Lot]| lots.iter().map(|l| l.units).sum::<Decimal>();
    let mut year_end: BTreeMap<i32, Decimal> = BTreeMap::new();
    let mut last_year = first.year();
    let snapshot = |lots: &[Lot], y: i32| -> Decimal {
        let end = NaiveDate::from_ymd_opt(y, 12, 31).expect("31/12 existe");
        cents(units(lots) * unit_on(end))
    };
    for m in moves {
        while m.date.year() > last_year {
            year_end.insert(last_year, snapshot(&lots, last_year));
            last_year += 1;
        }
        let u = unit_on(m.date);
        let base = Operation {
            id: format!("{}:{}", ticker, m.id),
            account: savings.account.clone(),
            asset: ticker.clone(),
            date: m.date,
            side: Side::Compra,
            quantity: Decimal::ZERO,
            price: u,
            manual_fees: Some(Decimal::ZERO),
            note: Some(savings.title().to_string()),
            currency: BRL.into(),
            from_account: None,
            residue: None,
            ir: None,
            iof: None,
        };
        match m.kind {
            MoveKind::Deposito => {
                let q = m.amount / u;
                lots.push(Lot {
                    units: q,
                    cost: m.amount,
                });
                report.operations.push(Operation {
                    quantity: q,
                    ..base
                });
            }
            MoveKind::Resgate => {
                // IR e IOF só entram se o banco mostrou; o app não estima.
                let (ir, iof) = (m.ir.unwrap_or_default(), m.iof.unwrap_or_default());
                let gross = m.amount + ir + iof;
                let q = (gross / u).min(units(&lots));
                let cost: Decimal = {
                    let mut left = q;
                    let mut c = Decimal::ZERO;
                    for l in &lots {
                        if left <= Decimal::ZERO || l.units.is_zero() {
                            continue;
                        }
                        let t = left.min(l.units);
                        c += l.cost * t / l.units;
                        left -= t;
                    }
                    c
                };
                take(&mut lots, q);
                report.redemptions.push(Redemption {
                    move_id: m.id.clone(),
                    date: m.date,
                    net: m.amount,
                    gross: cents(gross),
                    ir,
                    iof,
                    estimated: false,
                    gain: cents(gross - cost),
                });
                report.operations.push(Operation {
                    side: Side::Venda,
                    quantity: q,
                    ir: Some(ir),
                    iof: Some(iof),
                    manual_fees: None,
                    ..base
                });
            }
            MoveKind::Saldo if units(&lots).is_zero() => {
                // Primeiro saldo sem depósitos lançados: é o saldo inicial,
                // tratado como um depósito.
                report.last_informed = Some(m.date);
                if m.amount > Decimal::ZERO {
                    let q = m.amount / u;
                    lots.push(Lot {
                        units: q,
                        cost: m.amount,
                    });
                    report.operations.push(Operation {
                        quantity: q,
                        ..base
                    });
                }
            }
            MoveKind::Saldo => {
                let held = units(&lots);
                let expected = cents(held * u);
                let difference = m.amount - expected;
                // A taxa varia pouco num mês; uma diferença acima de 2% do
                // saldo (e de R$ 5) é mais provável um movimento esquecido.
                let tolerance = (expected.abs() * Decimal::new(2, 2)).max(Decimal::from(5));
                report.checks.push(BalanceCheck {
                    move_id: m.id.clone(),
                    date: m.date,
                    informed: m.amount,
                    expected,
                    difference,
                    // Sem rendimento ou com depósitos que ninguém lança (FGTS),
                    // a diferença é só o movimento do período.
                    suspicious: savings.kind == SavingsKind::Cofrinho
                        && difference.abs() > tolerance,
                });
                report.last_informed = Some(m.date);
                let target = m.amount / u;
                if !difference.is_zero() {
                    scale(&mut lots, target / held);
                    report.operations.push(Operation {
                        side: Side::Ajuste,
                        quantity: target - held,
                        price: Decimal::ZERO,
                        ..base
                    });
                }
            }
        }
        let held = units(&lots);
        match report.units_history.last_mut() {
            Some((d, q)) if *d == m.date => *q = held,
            _ => report.units_history.push((m.date, held)),
        }
    }
    if let Some(c) = closing {
        while c.year() > last_year {
            year_end.insert(last_year, snapshot(&lots, last_year));
            last_year += 1;
        }
        let held = units(&lots);
        if !held.is_zero() {
            report.operations.push(Operation {
                id: format!("{ticker}:encerrada"),
                account: savings.account.clone(),
                asset: ticker.clone(),
                date: c,
                side: Side::Ajuste,
                quantity: -held,
                price: Decimal::ZERO,
                manual_fees: Some(Decimal::ZERO),
                note: Some(savings.title().to_string()),
                currency: BRL.into(),
                from_account: None,
                residue: None,
                ir: None,
                iof: None,
            });
            lots.clear();
            report.units_history.push((c, Decimal::ZERO));
        }
    }
    let u = unit_on(today);
    while today.year() > last_year {
        year_end.insert(last_year, snapshot(&lots, last_year));
        last_year += 1;
    }
    report.balance = cents(units(&lots) * u);
    report.invested = cents(lots.iter().map(|l| l.cost).sum());
    report.balance_net = report.balance;
    report.year_end = year_end.into_iter().collect();
    report.unit_values = values;
    report
}

use chrono::Datelike;

/// Último dia do mês de `d`.
fn last_day(d: NaiveDate) -> NaiveDate {
    let (y, m) = if d.month() == 12 {
        (d.year() + 1, 1)
    } else {
        (d.year(), d.month() + 1)
    };
    NaiveDate::from_ymd_opt(y, m, 1)
        .expect("dia 1 existe")
        .pred_opt()
        .expect("dia anterior existe")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, day).unwrap()
    }

    fn cofrinho() -> Savings {
        Savings {
            id: "c1".into(),
            kind: SavingsKind::Cofrinho,
            account: "Banco X".into(),
            name: "Reserva".into(),
            rate_pct: dec!(100),
            tax_free: false,
            note: None,
            since: None,
            until: None,
        }
    }

    fn mv(id: &str, date: NaiveDate, kind: MoveKind, amount: Decimal) -> SavingsMove {
        SavingsMove {
            id: id.into(),
            savings_id: "c1".into(),
            date,
            kind,
            amount,
            ir: None,
            iof: None,
            note: None,
        }
    }

    #[test]
    fn conta_encerrada_zera_depois_do_ate() {
        let mut c = cofrinho();
        c.since = Some(d(1, 1));
        c.until = Some(d(3, 1));
        let moves = vec![
            mv("a", d(1, 10), MoveKind::Saldo, dec!(1000)),
            mv("b", d(4, 10), MoveKind::Saldo, dec!(2000)),
        ];
        // Antes do fim de março, o saldo vale; o lançamento de abril é ignorado.
        let r = build(&c, &moves, d(3, 31));
        assert_eq!(r.balance, dec!(1000));
        let r = build(&c, &moves, d(5, 1));
        assert_eq!(r.balance, Decimal::ZERO);
        assert_eq!(r.units_history.last(), Some(&(d(4, 1), Decimal::ZERO)));
        assert!(c.active_between(d(3, 1), d(12, 31)));
        assert!(!c.active_between(d(4, 1), d(12, 31)));
        assert!(c.open_on(d(3, 31)) && !c.open_on(d(4, 1)));
    }

    #[test]
    fn rendimento_pelos_saldos_informados_com_retirada() {
        // O exemplo dado ao usuário: 1.000 em 01/09, retira 300, deposita 200,
        // banco mostra 912 em 30/09.
        let moves = vec![
            mv("a", d(9, 1), MoveKind::Deposito, dec!(1000)),
            mv("b", d(9, 10), MoveKind::Resgate, dec!(300)),
            mv("c", d(9, 20), MoveKind::Deposito, dec!(200)),
            mv("s", d(9, 30), MoveKind::Saldo, dec!(912)),
        ];
        let r = build(&cofrinho(), &moves, d(9, 30));
        assert_eq!(r.balance, dec!(912));
        // Sem estimativa: o resgate é o valor que saiu, sem IR nem IOF.
        let red = &r.redemptions[0];
        assert!(!red.estimated);
        assert_eq!(red.gross, dec!(300));
        assert_eq!(red.iof, Decimal::ZERO);
        // Esperado 900 (sem CDI); os 12 a mais são o rendimento do mês.
        assert_eq!(r.checks[0].expected, dec!(900));
        assert!(!r.checks[0].suspicious);
        // Depósito + ajuste + resgate etc. viram operações da carteira.
        assert_eq!(r.operations.len(), 4);
        assert_eq!(r.operations[3].side, Side::Ajuste);
    }

    #[test]
    fn saldo_muito_diferente_avisa_movimento_esquecido() {
        let moves = vec![
            mv("a", d(3, 2), MoveKind::Deposito, dec!(1000)),
            mv("s", d(3, 31), MoveKind::Saldo, dec!(700)),
        ];
        let r = build(&cofrinho(), &moves, d(4, 1));
        assert!(r.checks[0].suspicious);
        assert_eq!(r.last_informed, Some(d(3, 31)));
    }

    #[test]
    fn sem_estimativa_entre_saldos_e_saldo_de_fim_de_ano() {
        let moves = vec![
            mv("a", d(1, 5), MoveKind::Deposito, dec!(10000)),
            mv("s", d(6, 30), MoveKind::Saldo, dec!(10600)),
        ];
        let r = build(&cofrinho(), &moves, d(12, 31));
        // Nada cresce depois do último saldo informado.
        assert_eq!(r.balance, dec!(10600));
        assert_eq!(r.balance_on(d(3, 31)), dec!(10000));
        assert_eq!(r.balance_net, r.balance);
        assert_eq!(r.invested, dec!(10000));
        assert_eq!(r.year_end, vec![]);
        let r2 = build(
            &cofrinho(),
            &moves,
            NaiveDate::from_ymd_opt(2027, 1, 10).unwrap(),
        );
        assert_eq!(r2.year_end, vec![(2026, dec!(10600))]);
    }

    #[test]
    fn saldo_inicial_fgts_e_conta() {
        // Começa só informando o saldo: vira o saldo inicial.
        let fgts = Savings {
            kind: SavingsKind::Fgts,
            ..cofrinho()
        };
        let moves = vec![
            mv("s1", d(1, 10), MoveKind::Saldo, dec!(20000)),
            mv("s2", d(7, 10), MoveKind::Saldo, dec!(23000)),
        ];
        let r = build(&fgts, &moves, d(12, 31));
        assert!(
            !r.checks[0].suspicious,
            "depósitos do empregador não são lançados"
        );
        assert_eq!(r.invested, dec!(20000));
        // Sem estimativa: vale o último saldo informado.
        assert_eq!(r.balance, dec!(23000));
        assert_eq!(r.balance_net, r.balance, "FGTS não tem IR");

        let conta = Savings {
            kind: SavingsKind::Conta,
            ..cofrinho()
        };
        let moves = vec![
            mv("s1", d(3, 1), MoveKind::Saldo, dec!(1500)),
            mv("s2", d(4, 1), MoveKind::Saldo, dec!(400)),
        ];
        let r = build(&conta, &moves, d(5, 1));
        assert_eq!(r.balance, dec!(400));
        assert_eq!(r.balance_net, dec!(400));
        assert!(!r.checks[0].suspicious);
        assert_eq!(r.balance_on(d(3, 15)), dec!(1500));
        assert_eq!(r.balance_on(d(2, 1)), dec!(0));
    }

    #[test]
    fn nome_em_branco_ou_igual_ao_tipo_nao_e_nome_proprio() {
        let conta = |name: &str| Savings {
            kind: SavingsKind::Conta,
            name: name.into(),
            ..cofrinho()
        };
        assert!(conta("").validate().is_ok());
        assert_eq!(conta("").own_name(), None);
        assert_eq!(conta("Conta corrente").own_name(), None);
        assert_eq!(conta("Conta corrente").title(), "Conta corrente");
        assert_eq!(conta("Conjunta").own_name(), Some("Conjunta"));
        let fgts = Savings {
            kind: SavingsKind::Fgts,
            account: String::new(),
            ..conta("")
        };
        assert!(fgts.validate().is_ok(), "FGTS não tem banco");
        assert_eq!(fgts.bank(), "");
        assert!(conta("").validate().is_ok());
    }

    #[test]
    fn tabelas_de_ir_e_iof() {
        assert_eq!(ir_rate(10), dec!(0.225));
        assert_eq!(ir_rate(200), dec!(0.20));
        assert_eq!(ir_rate(500), dec!(0.175));
        assert_eq!(ir_rate(800), dec!(0.15));
        assert_eq!(iof_rate(0), dec!(1));
        assert_eq!(iof_rate(9), dec!(0.70));
        assert_eq!(iof_rate(29), dec!(0.03));
        assert_eq!(iof_rate(30), dec!(0));
    }
}
