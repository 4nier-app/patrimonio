//! Proventos recebidos, com o mesmo registro de auditoria dos lançamentos.

use carteira_core::income::{Income, IncomeKind};
use carteira_ledger::Action;
use rusqlite::{params, OptionalExtension};

use crate::records::{bad, date, dec, new_id, normalize_ticker};
use crate::{now_iso, Result, Vault};

const SELECT: &str =
    "SELECT id, account, ticker, kind, pay_date, ex_date, gross, withheld, currency, note
                      FROM incomes";

fn row_to_income(r: &rusqlite::Row<'_>) -> rusqlite::Result<Income> {
    let kind: String = r.get(3)?;
    Ok(Income {
        id: r.get(0)?,
        account: r.get(1)?,
        asset: r.get(2)?,
        kind: IncomeKind::parse(&kind).unwrap_or(IncomeKind::Outro),
        pay_date: date(r.get(4)?, 4)?,
        ex_date: r
            .get::<_, Option<String>>(5)?
            .map(|s| date(s, 5))
            .transpose()?,
        gross: dec(r.get(6)?, 6)?,
        withheld: dec(r.get(7)?, 7)?,
        currency: r.get(8)?,
        note: r.get(9)?,
    })
}

fn income_json(i: &Income) -> String {
    serde_json::to_string(i).expect("provento serializável")
}

impl Vault {
    /// Normaliza e confere o provento: o ativo pelo código, a moeda pela conta.
    fn prepare_income(&self, i: &mut Income) -> Result<()> {
        i.asset = normalize_ticker(&i.asset);
        i.account = i.account.trim().to_string();
        i.note = i
            .note
            .take()
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty());
        let account = self
            .get_account(&i.account)?
            .ok_or_else(|| bad(format!("a conta \"{}\" não existe", i.account)))?;
        i.currency = account.currency;
        i.validate().map_err(bad)
    }

    pub fn create_income(&mut self, mut i: Income) -> Result<Income> {
        self.prepare_income(&mut i)?;
        i.id = new_id()?.replacen("op-", "pv-", 1);
        let now = now_iso();
        self.conn.execute(
            "INSERT INTO incomes (id, account, ticker, kind, pay_date, ex_date, gross, withheld, currency, note,
               created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
            params![
                i.id,
                i.account,
                i.asset,
                i.kind.as_str(),
                i.pay_date.to_string(),
                i.ex_date.map(|d| d.to_string()),
                i.gross.to_string(),
                i.withheld.to_string(),
                i.currency,
                i.note,
                now
            ],
        )?;
        self.append_event(
            "PROVENTO",
            &i.id.clone(),
            Action::Criada,
            None,
            Some(&income_json(&i)),
            None,
        )?;
        Ok(i)
    }

    pub fn get_income(&self, id: &str) -> Result<Option<Income>> {
        Ok(self
            .conn
            .query_row(
                &format!("{SELECT} WHERE id = ?1 AND removed_at IS NULL"),
                [id],
                row_to_income,
            )
            .optional()?)
    }

    pub fn update_income(&mut self, mut i: Income, reason: Option<&str>) -> Result<Income> {
        let before = self
            .get_income(&i.id)?
            .ok_or_else(|| bad("provento não encontrado"))?;
        self.prepare_income(&mut i)?;
        if before == i {
            return Ok(i);
        }
        self.conn.execute(
            "UPDATE incomes SET account = ?2, ticker = ?3, kind = ?4, pay_date = ?5, ex_date = ?6, gross = ?7,
               withheld = ?8, currency = ?9, note = ?10, updated_at = ?11 WHERE id = ?1",
            params![
                i.id,
                i.account,
                i.asset,
                i.kind.as_str(),
                i.pay_date.to_string(),
                i.ex_date.map(|d| d.to_string()),
                i.gross.to_string(),
                i.withheld.to_string(),
                i.currency,
                i.note,
                now_iso()
            ],
        )?;
        self.append_event(
            "PROVENTO",
            &i.id.clone(),
            Action::Alterada,
            Some(&income_json(&before)),
            Some(&income_json(&i)),
            reason,
        )?;
        Ok(i)
    }

    pub fn remove_income(&mut self, id: &str, reason: Option<&str>) -> Result<()> {
        let reason = reason.map(str::trim).filter(|r| !r.is_empty());
        let before = self
            .get_income(id)?
            .ok_or_else(|| bad("provento não encontrado"))?;
        self.conn.execute(
            "UPDATE incomes SET removed_at = ?2 WHERE id = ?1",
            params![id, now_iso()],
        )?;
        self.append_event(
            "PROVENTO",
            id,
            Action::Removida,
            Some(&income_json(&before)),
            None,
            reason,
        )?;
        Ok(())
    }

    /// Desfaz a remoção (o botão "Desfazer" logo depois de remover). Na
    /// Auditoria, entra como criação com o motivo "remoção desfeita".
    pub fn restore_income(&mut self, id: &str) -> Result<Income> {
        let item = self
            .conn
            .query_row(
                &format!("{SELECT} WHERE id = ?1 AND removed_at IS NOT NULL"),
                [id],
                row_to_income,
            )
            .optional()?
            .ok_or_else(|| bad("registro removido não encontrado"))?;
        self.conn
            .execute("UPDATE incomes SET removed_at = NULL WHERE id = ?1", [id])?;
        self.append_event(
            "PROVENTO",
            id,
            Action::Criada,
            None,
            Some(&income_json(&item)),
            Some("remoção desfeita"),
        )?;
        Ok(item)
    }

    /// Proventos ativos, do pagamento mais antigo para o mais recente.
    pub fn list_incomes(&self) -> Result<Vec<Income>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT} WHERE removed_at IS NULL ORDER BY pay_date, created_at"
        ))?;
        let rows = stmt.query_map([], row_to_income)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KdfParams;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn vault(dir: &std::path::Path) -> Vault {
        Vault::create_with_params(
            dir,
            "senha-de-teste-123",
            "0.2.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap()
    }

    fn income(account: &str) -> Income {
        Income {
            id: String::new(),
            account: account.into(),
            asset: "test3f".into(),
            kind: IncomeKind::Jcp,
            pay_date: NaiveDate::from_ymd_opt(2026, 3, 10).unwrap(),
            ex_date: None,
            gross: dec!(100),
            withheld: dec!(15),
            currency: "USD".into(),
            note: Some("  ".into()),
        }
    }

    #[test]
    fn provento_com_auditoria() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        assert!(v.create_income(income("Corretora X")).is_err());
        v.conn
            .execute(
                "INSERT INTO accounts (name, currency, created_at) VALUES ('Corretora X', 'BRL', '')",
                [],
            )
            .unwrap();
        let saved = v.create_income(income("Corretora X")).unwrap();
        assert_eq!(saved.asset, "TEST3");
        assert_eq!(saved.currency, "BRL");
        assert_eq!(saved.note, None);
        assert_eq!(v.list_incomes().unwrap(), vec![saved.clone()]);

        let mut changed = saved.clone();
        changed.gross = dec!(200);
        changed.withheld = dec!(30);
        v.update_income(changed.clone(), Some("valor corrigido"))
            .unwrap();
        assert_eq!(v.get_income(&saved.id).unwrap(), Some(changed));

        let mut bad = saved.clone();
        bad.withheld = dec!(500);
        assert!(v.update_income(bad, None).is_err());

        v.remove_income(&saved.id, None).unwrap();
        assert!(v.list_incomes().unwrap().is_empty());
        // Desfazer devolve o provento; um segundo desfazer não acha nada.
        v.restore_income(&saved.id).unwrap();
        assert_eq!(v.list_incomes().unwrap().len(), 1);
        assert!(v.restore_income(&saved.id).is_err());
        v.remove_income(&saved.id, None).unwrap();
        let events = v.list_events(50, 0).unwrap();
        let mut kinds: Vec<_> = events
            .iter()
            .filter(|e| e.data.entity == "PROVENTO")
            .map(|e| e.data.action)
            .collect();
        kinds.reverse();
        assert_eq!(
            kinds,
            vec![
                Action::Criada,
                Action::Alterada,
                Action::Removida,
                Action::Criada,
                Action::Removida
            ]
        );
        assert!(v.verify_integrity().unwrap().ok);
    }
}
