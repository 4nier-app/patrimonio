//! Cofrinhos (caixinhas): renda fixa com resgate a qualquer hora, que rende um
//! percentual do CDI que o banco muda quando quer.
//!
//! O que vale é o saldo que o banco mostra. O usuário lança depósitos e
//! resgates e, de tempos em tempos, informa o saldo do dia. Entre dois saldos
//! informados, o app só estima o crescimento pelo CDI (à taxa cadastrada,
//! 100% se não houver), e cada saldo novo corrige a estimativa. Assim o
//! rendimento de um período é sempre `saldo final − saldo inicial − depósitos
//! + resgates`, qualquer que tenha sido a taxa.
//!
//! Para entrar na carteira, na rentabilidade e no Painel como qualquer outro
//! ativo, o cofrinho vira uma "cota" que vale R$ 1 no primeiro depósito e
//! cresce pelo CDI: depósito é compra de cotas, resgate é venda (com o IR e o
//! IOF retidos como desconto, como nos fundos) e o saldo informado ajusta a
//! quantidade de cotas sem dinheiro entrar ([`Side::Ajuste`]).

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
}

impl Savings {
    pub fn validate(&self) -> Result<(), String> {
        if self.account.trim().is_empty() {
            return Err("informe o banco (conta)".into());
        }
        if self.name.trim().is_empty() {
            return Err("dê um nome".into());
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
    date: NaiveDate,
    units: Decimal,
    cost: Decimal,
}

/// Imposto (IR, IOF) de resgatar `units` cotas valendo `unit` cada, tirando
/// dos lotes mais antigos primeiro. Não altera os lotes.
fn tax_of(
    lots: &[Lot],
    units: Decimal,
    unit: Decimal,
    date: NaiveDate,
    tax_free: bool,
) -> (Decimal, Decimal) {
    if tax_free {
        return (Decimal::ZERO, Decimal::ZERO);
    }
    let mut left = units;
    let (mut ir, mut iof) = (Decimal::ZERO, Decimal::ZERO);
    for lot in lots {
        if left <= Decimal::ZERO {
            break;
        }
        let take = left.min(lot.units);
        if lot.units.is_zero() {
            continue;
        }
        let cost = lot.cost * take / lot.units;
        let gain = take * unit - cost;
        left -= take;
        if gain <= Decimal::ZERO {
            continue;
        }
        let days = (date - lot.date).num_days();
        let iof_lot = gain * iof_rate(days);
        iof += iof_lot;
        ir += (gain - iof_lot) * ir_rate(days);
    }
    (cents(ir), cents(iof))
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
    /// Saldo bruto estimado hoje e o que sobraria depois do IR e do IOF.
    pub balance: Decimal,
    pub balance_net: Decimal,
    /// Quanto do saldo é dinheiro depositado (custo das cotas).
    pub invested: Decimal,
    pub last_informed: Option<NaiveDate>,
    pub checks: Vec<BalanceCheck>,
    pub redemptions: Vec<Redemption>,
    /// Saldo bruto em 31/12 de cada ano (para Bens e Direitos).
    pub year_end: Vec<(i32, Decimal)>,
}

/// Monta o cofrinho a partir dos movimentos e do CDI diário (% ao dia, série
/// 12 do Banco Central). Um dia sem CDI (fim de semana, feriado) não rende.
pub fn build(
    savings: &Savings,
    moves: &[SavingsMove],
    cdi: &BTreeMap<NaiveDate, Decimal>,
    today: NaiveDate,
) -> SavingsReport {
    let ticker = ticker(&savings.id);
    let mut moves: Vec<&SavingsMove> = moves.iter().filter(|m| m.date <= today).collect();
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
    };
    let Some(first) = moves.first().map(|m| m.date) else {
        return report;
    };

    // Valor da cota: R$ 1 no primeiro dia; cada dia útil seguinte soma o CDI
    // do dia útil anterior (o dinheiro rende de um dia para o outro).
    let rate = savings.rate_pct / Decimal::ONE_HUNDRED;
    let mut unit = Decimal::ONE;
    let mut values = vec![(first, unit)];
    match savings.kind {
        SavingsKind::Cofrinho => {
            let mut prev_cdi: Option<Decimal> = cdi.get(&first).copied();
            for (d, v) in cdi.range(first.succ_opt().unwrap_or(first)..=today) {
                if let Some(p) = prev_cdi {
                    unit = (unit * (Decimal::ONE + p / Decimal::ONE_HUNDRED * rate)).round_dp(12);
                }
                values.push((*d, unit));
                prev_cdi = Some(*v);
            }
        }
        SavingsKind::Fgts => {
            // 3% ao ano, capitalizados por mês como a Caixa faz (dia 10),
            // aqui espalhados dia a dia: 1,03^(1/365) por dia corrido.
            let daily = Decimal::new(10_000_809_868, 10);
            let mut d = first;
            while let Some(next) = d.succ_opt().filter(|n| *n <= today) {
                unit = (unit * daily).round_dp(12);
                if next.day() == 1 || next == today {
                    values.push((next, unit));
                }
                d = next;
            }
        }
        SavingsKind::Conta | SavingsKind::Beneficio => {}
    }
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
            note: Some(savings.name.clone()),
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
                    date: m.date,
                    units: q,
                    cost: m.amount,
                });
                report.operations.push(Operation {
                    quantity: q,
                    ..base
                });
            }
            MoveKind::Resgate => {
                let informed = m.ir.is_some() || m.iof.is_some();
                let (mut ir, mut iof) = (m.ir.unwrap_or_default(), m.iof.unwrap_or_default());
                let mut gross = m.amount + ir + iof;
                if !informed {
                    // O imposto depende do bruto, que depende do imposto:
                    // poucas voltas bastam para fechar no centavo.
                    for _ in 0..5 {
                        (ir, iof) = tax_of(
                            &lots,
                            gross / u,
                            u,
                            m.date,
                            savings.tax_free || !savings.kind.is_investment(),
                        );
                        gross = m.amount + ir + iof;
                    }
                }
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
                    estimated: !informed,
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
                        date: m.date,
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
    }
    let u = unit_on(today);
    while today.year() > last_year {
        year_end.insert(last_year, snapshot(&lots, last_year));
        last_year += 1;
    }
    report.balance = cents(units(&lots) * u);
    report.invested = cents(lots.iter().map(|l| l.cost).sum());
    let (ir, iof) = tax_of(
        &lots,
        units(&lots),
        u,
        today,
        savings.tax_free || !savings.kind.is_investment(),
    );
    report.balance_net = report.balance - ir - iof;
    report.year_end = year_end.into_iter().collect();
    report.unit_values = values;
    report
}

use chrono::Datelike;

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

    /// CDI de 0,05% ao dia em todos os dias úteis de 2026.
    fn cdi() -> BTreeMap<NaiveDate, Decimal> {
        let mut m = BTreeMap::new();
        let mut day = d(1, 2);
        while day.year() == 2026 {
            if day.weekday().num_days_from_monday() < 5 {
                m.insert(day, dec!(0.05));
            }
            day = day.succ_opt().unwrap();
        }
        m
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
        let r = build(&cofrinho(), &moves, &cdi(), d(9, 30));
        assert_eq!(r.balance, dec!(912));
        // Resgate cedo: o IOF come quase todo o rendimento, e o bruto passa
        // um pouco dos 300.
        let red = &r.redemptions[0];
        assert!(red.estimated);
        assert!(red.gross > dec!(300) && red.gross < dec!(301));
        assert!(red.iof > Decimal::ZERO);
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
        let r = build(&cofrinho(), &moves, &cdi(), d(4, 1));
        assert!(r.checks[0].suspicious);
        assert_eq!(r.last_informed, Some(d(3, 31)));
    }

    #[test]
    fn estimativa_pelo_cdi_e_saldo_de_fim_de_ano() {
        let moves = vec![mv("a", d(1, 5), MoveKind::Deposito, dec!(10000))];
        let r = build(&cofrinho(), &moves, &cdi(), d(12, 31));
        // ~250 dias úteis a 0,05%: cerca de 13%.
        assert!(r.balance > dec!(11200) && r.balance < dec!(11500));
        assert_eq!(r.invested, dec!(10000));
        // Mais de 360 dias? Não: 17,5% só depois de 360; aqui 20%.
        let gain = r.balance - r.invested;
        assert_eq!(r.balance_net, r.balance - (gain * dec!(0.20)).round_dp(2));
        assert_eq!(r.year_end, vec![]);
        let r2 = build(
            &cofrinho(),
            &moves,
            &cdi(),
            NaiveDate::from_ymd_opt(2027, 1, 10).unwrap(),
        );
        assert_eq!(r2.year_end[0].0, 2026);
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
        let r = build(&fgts, &moves, &BTreeMap::new(), d(12, 31));
        assert!(
            !r.checks[0].suspicious,
            "depósitos do empregador não são lançados"
        );
        assert_eq!(r.invested, dec!(20000));
        // De julho a dezembro, 3% ao ano: cerca de 1,4%.
        assert!(
            r.balance > dec!(23300) && r.balance < dec!(23360),
            "{}",
            r.balance
        );
        assert_eq!(r.balance_net, r.balance, "FGTS não tem IR");

        let conta = Savings {
            kind: SavingsKind::Conta,
            ..cofrinho()
        };
        let moves = vec![
            mv("s1", d(3, 1), MoveKind::Saldo, dec!(1500)),
            mv("s2", d(4, 1), MoveKind::Saldo, dec!(400)),
        ];
        let r = build(&conta, &moves, &cdi(), d(5, 1));
        assert_eq!(r.balance, dec!(400));
        assert_eq!(r.balance_net, dec!(400));
        assert!(!r.checks[0].suspicious);
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
