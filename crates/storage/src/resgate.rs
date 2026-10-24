//! Valor de resgate das corretoras em dólar: quanto chegaria no Brasil se o
//! saldo fosse trazido hoje. Usa o dólar comercial menos a taxa de conversão
//! da corretora e o IOF. Os impostos continuam na PTAX (Lei 14.754/2023).

use std::collections::BTreeMap;

use carteira_core::format::{brl as brl_fmt, number};
use carteira_core::model::{FxRate, FxSource, USD};
use carteira_ledger::Action;
use chrono::NaiveDate;
use rusqlite::params;
use rust_decimal::Decimal;

use crate::records::{bad, dec, FxTable, USD_BRL_COMMERCIAL};
use crate::{now_iso, Result, Vault};

/// IOF na compra de reais com dólares do exterior, em %. Fixo no aplicativo:
/// se o governo mudar, sai uma versão nova.
pub const IOF_PCT: Decimal = Decimal::from_parts(38, 0, 0, false, 2);

/// Prefixo da origem do câmbio quando o saldo está pelo valor de resgate.
pub const RESGATE: &str = "RESGATE";

/// Taxa de conversão mais IOF, em %.
pub fn total_pct(fee_pct: Decimal) -> Decimal {
    fee_pct + IOF_PCT
}

fn pct_br(p: Decimal) -> String {
    format!("{}%", number(p, 2))
}

/// "conversão 1,00% + IOF 0,38%".
pub fn fee_breakdown(fee_pct: Decimal) -> String {
    format!("conversão {} + IOF {}", pct_br(fee_pct), pct_br(IOF_PCT))
}

/// Conta do valor de resgate de um saldo em dólar, para mostrar ao lado do
/// valor; `None` quando o saldo está pela PTAX.
pub fn explain_value(
    usd: Decimal,
    brl: Decimal,
    value: &FxRate,
    ptax: Option<&FxRate>,
) -> Option<String> {
    let how = value.source.strip_prefix(RESGATE)?.trim_start_matches(": ");
    let mut out = format!(
        "US$ {} × {how} = {} que chegariam no Brasil",
        number(usd, 2),
        brl_fmt(brl)
    );
    if let Some(p) = ptax {
        out.push_str(&format!(
            "\nPara o imposto de renda: × PTAX {} ({}) = {}",
            number(p.rate, 4),
            p.date.format("%d/%m/%Y"),
            brl_fmt(usd * p.rate)
        ));
    }
    Some(out)
}

/// Câmbio para avaliar saldos: PTAX por padrão, valor de resgate nas contas
/// com taxa de conversão informada.
#[derive(Debug, Default)]
pub struct ValueFx {
    ptax: FxTable,
    commercial: FxTable,
    fees: BTreeMap<String, Decimal>,
}

impl ValueFx {
    pub fn new(ptax: FxTable, commercial: FxTable, fees: BTreeMap<String, Decimal>) -> Self {
        Self {
            ptax,
            commercial,
            fees,
        }
    }

    /// Taxa de conversão informada para a conta, em %.
    pub fn fee(&self, account: &str) -> Option<Decimal> {
        self.fees.get(account).copied()
    }

    /// PTAX de cada dia (operações, impostos e o dólar como índice).
    pub fn ptax(&self) -> &FxTable {
        &self.ptax
    }

    pub fn commercial_on_or_before(&self, date: NaiveDate) -> Option<FxRate> {
        self.commercial.usd_brl_on_or_before(date)
    }
}

impl FxSource for ValueFx {
    fn usd_brl_on_or_before(&self, date: NaiveDate) -> Option<FxRate> {
        self.ptax.usd_brl_on_or_before(date)
    }

    fn value_rate(&self, account: &str, date: NaiveDate) -> Option<FxRate> {
        let Some(fee) = self.fee(account) else {
            return self.ptax.usd_brl_on_or_before(date);
        };
        let total = total_pct(fee);
        let (base, name) = match self.commercial.usd_brl_on_or_before(date) {
            Some(c) => (c, "dólar comercial"),
            None => (self.ptax.usd_brl_on_or_before(date)?, "PTAX"),
        };
        // A corretora desconta a taxa de conversão e depois o IOF, um sobre o
        // outro (US$ 100 a 5,0372 com 1% + 0,38% = R$ 496,79).
        let hundred = Decimal::from(100);
        let factor = (Decimal::ONE - fee / hundred) * (Decimal::ONE - IOF_PCT / hundred);
        Some(FxRate {
            date: base.date,
            rate: (base.rate * factor).round_dp(8),
            source: format!(
                "{RESGATE}: {name} {} ({}) − {} ({})",
                number(base.rate, 4),
                base.date.format("%d/%m/%Y"),
                pct_br(total),
                fee_breakdown(fee)
            ),
        })
    }

    fn cost_rate(&self, account: &str, date: NaiveDate) -> Option<FxRate> {
        // O IOF e a taxa da remessa foram pagos antes, ao mandar o dinheiro:
        // a compra vale pelo mesmo valor de resgate do dia, e o ganho fica só
        // com a variação do ativo e do dólar.
        self.value_rate(account, date)
    }
}

impl Vault {
    /// Taxa de conversão (%) de cada corretora em dólar que a informou.
    pub fn exit_costs(&self) -> Result<BTreeMap<String, Decimal>> {
        let mut stmt = self
            .conn
            .prepare("SELECT account, fee_pct FROM account_exit_cost")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, dec(r.get(1)?, 1)?)))?;
        Ok(rows.collect::<std::result::Result<BTreeMap<_, _>, _>>()?)
    }

    /// Define a taxa de conversão da corretora (ou apaga, com `None`).
    pub fn set_exit_cost(&mut self, account: &str, fee_pct: Option<Decimal>) -> Result<()> {
        match self.get_account(account)? {
            None => return Err(bad("conta não encontrada")),
            Some(a) if a.currency != USD => {
                return Err(bad(
                    "o custo para trazer ao Brasil vale só para contas em dólar",
                ))
            }
            Some(_) => {}
        }
        if let Some(f) = fee_pct {
            if f < Decimal::ZERO || f > Decimal::from(10) {
                return Err(bad("a taxa de conversão vai de 0% a 10%"));
            }
        }
        let before = self.exit_costs()?.remove(account);
        match fee_pct {
            None => {
                self.conn.execute(
                    "DELETE FROM account_exit_cost WHERE account = ?1",
                    [account],
                )?;
            }
            Some(f) => {
                self.conn.execute(
                    "INSERT INTO account_exit_cost (account, fee_pct, updated_at) VALUES (?1, ?2, ?3)
                     ON CONFLICT (account) DO UPDATE SET fee_pct = ?2, updated_at = ?3",
                    params![account, f.normalize().to_string(), now_iso()],
                )?;
            }
        }
        if before == fee_pct {
            return Ok(());
        }
        let show = |f: Decimal| {
            serde_json::json!({ "taxa_conversao_pct": f.normalize().to_string(), "iof_pct": IOF_PCT.to_string() })
                .to_string()
        };
        self.append_event(
            "CUSTO_RESGATE",
            account,
            match (before, fee_pct) {
                (None, _) => Action::Criada,
                (_, None) => Action::Removida,
                _ => Action::Alterada,
            },
            before.map(show).as_deref(),
            fee_pct.map(show).as_deref(),
            None,
        )?;
        Ok(())
    }

    /// Câmbio para avaliar os saldos (PTAX ou valor de resgate por conta).
    pub fn value_fx(&self) -> Result<ValueFx> {
        Ok(ValueFx::new(
            self.fx_table()?,
            self.fx_pair_table(USD_BRL_COMMERCIAL)?,
            self.exit_costs()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KdfParams, NewFxRate};
    use rust_decimal_macros::dec;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, day).unwrap()
    }

    fn rate(day: u32, r: Decimal, source: &str) -> NewFxRate {
        NewFxRate {
            date: d(day),
            buy: r,
            sell: r,
            source: source.into(),
        }
    }

    #[test]
    fn valor_de_resgate_por_conta() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.2.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        v.conn
            .execute_batch(
                "INSERT INTO accounts (name, currency, created_at) VALUES ('Corretora US', 'USD', '');
                 INSERT INTO accounts (name, currency, created_at) VALUES ('Outra US', 'USD', '');
                 INSERT INTO accounts (name, currency, created_at) VALUES ('Banco', 'BRL', '');",
            )
            .unwrap();
        assert!(v.set_exit_cost("Banco", Some(dec!(1))).is_err());
        assert!(v.set_exit_cost("Corretora US", Some(dec!(11))).is_err());
        v.set_exit_cost("Corretora US", Some(dec!(1.00))).unwrap();
        assert_eq!(v.exit_costs().unwrap()["Corretora US"], dec!(1));

        v.save_fx_rates(&[rate(2, dec!(5.10), "PTAX")]).unwrap();
        v.save_fx_pair(USD_BRL_COMMERCIAL, &[rate(2, dec!(5.0372), "YAHOO")])
            .unwrap();
        let fx = v.value_fx().unwrap();
        // PTAX nas operações e nas contas sem taxa.
        assert_eq!(fx.usd_brl_on_or_before(d(3)).unwrap().rate, dec!(5.10));
        assert_eq!(fx.value_rate("Outra US", d(3)).unwrap().rate, dec!(5.10));
        // Dólar comercial menos 1,38%: US$ 100 viram R$ 496,79.
        let r = fx.value_rate("Corretora US", d(3)).unwrap();
        assert_eq!((r.rate * dec!(100)).round_dp(2), dec!(496.79));
        assert!(r.source.starts_with(RESGATE));
        assert!(r.source.contains("1,38%"), "{}", r.source);
        // A compra vale pelo mesmo valor de resgate do dia; sem taxa, a PTAX.
        assert_eq!(fx.cost_rate("Corretora US", d(3)).unwrap().rate, r.rate);
        assert_eq!(fx.cost_rate("Outra US", d(3)).unwrap().rate, dec!(5.10));
        let note = explain_value(
            dec!(100),
            dec!(496.79),
            &r,
            Some(&fx.usd_brl_on_or_before(d(3)).unwrap()),
        )
        .unwrap();
        assert_eq!(
            note,
            "US$ 100,00 × dólar comercial 5,0372 (02/03/2026) − 1,38% (conversão 1,00% + IOF 0,38%) = R$ 496,79 que chegariam no Brasil\nPara o imposto de renda: × PTAX 5,1000 (02/03/2026) = R$ 510,00"
        );
        assert!(explain_value(
            dec!(1),
            dec!(5),
            &fx.value_rate("Outra US", d(3)).unwrap(),
            None
        )
        .is_none());

        v.set_exit_cost("Corretora US", None).unwrap();
        assert!(v.exit_costs().unwrap().is_empty());
        let fx = v.value_fx().unwrap();
        assert_eq!(
            fx.value_rate("Corretora US", d(3)).unwrap().rate,
            dec!(5.10)
        );
    }
}
