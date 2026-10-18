//! Bens (imóvel e veículo) com as obras, e a corretagem de cada conta, com o
//! mesmo registro de auditoria dos lançamentos.

use std::collections::BTreeMap;

use carteira_core::operations::{Brokerage, BrokerageTable};
use carteira_core::owned::{Improvement, OwnedAsset, OwnedKind};
use carteira_ledger::Action;
use rusqlite::{params, OptionalExtension};
use rust_decimal::Decimal;

use crate::records::{bad, date, dec, new_id};
use crate::{now_iso, Result, Vault};

const SELECT: &str = "SELECT id, kind, name, bought_on, price, sold_on, note FROM assets_owned";
const SELECT_WORK: &str =
    "SELECT id, owned_id, done_on, amount, description FROM asset_improvements";

fn json<T: serde::Serialize>(t: &T) -> String {
    serde_json::to_string(t).expect("registro serializável")
}

fn row_to_owned(r: &rusqlite::Row<'_>) -> rusqlite::Result<OwnedAsset> {
    let kind: String = r.get(1)?;
    Ok(OwnedAsset {
        id: r.get(0)?,
        kind: OwnedKind::parse(&kind).unwrap_or_default(),
        name: r.get(2)?,
        bought_on: date(r.get(3)?, 3)?,
        price: dec(r.get(4)?, 4)?,
        sold_on: r
            .get::<_, Option<String>>(5)?
            .map(|s| date(s, 5))
            .transpose()?,
        note: r.get(6)?,
    })
}

fn row_to_work(r: &rusqlite::Row<'_>) -> rusqlite::Result<Improvement> {
    Ok(Improvement {
        id: r.get(0)?,
        owned_id: r.get(1)?,
        date: date(r.get(2)?, 2)?,
        amount: dec(r.get(3)?, 3)?,
        description: r.get(4)?,
    })
}

fn prepare(a: &mut OwnedAsset) -> Result<()> {
    a.name = a.name.trim().to_string();
    a.note = a
        .note
        .take()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    a.validate().map_err(bad)
}

impl Vault {
    pub fn list_owned(&self) -> Result<Vec<OwnedAsset>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT} WHERE removed_at IS NULL ORDER BY bought_on, name"
        ))?;
        let rows = stmt.query_map([], row_to_owned)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get_owned(&self, id: &str) -> Result<Option<OwnedAsset>> {
        Ok(self
            .conn
            .query_row(
                &format!("{SELECT} WHERE id = ?1 AND removed_at IS NULL"),
                [id],
                row_to_owned,
            )
            .optional()?)
    }

    pub fn create_owned(&mut self, mut a: OwnedAsset) -> Result<OwnedAsset> {
        prepare(&mut a)?;
        a.id = new_id()?.replacen("op-", "bm-", 1);
        let now = now_iso();
        self.conn.execute(
            "INSERT INTO assets_owned (id, kind, name, bought_on, price, sold_on, note, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                a.id,
                a.kind.as_str(),
                a.name,
                a.bought_on.to_string(),
                a.price.to_string(),
                a.sold_on.map(|d| d.to_string()),
                a.note,
                now
            ],
        )?;
        self.append_event(
            "BEM",
            &a.id.clone(),
            Action::Criada,
            None,
            Some(&json(&a)),
            None,
        )?;
        Ok(a)
    }

    pub fn update_owned(&mut self, mut a: OwnedAsset) -> Result<OwnedAsset> {
        let before = self
            .get_owned(&a.id)?
            .ok_or_else(|| bad("bem não encontrado"))?;
        prepare(&mut a)?;
        if a.kind != before.kind {
            return Err(bad("não dá para trocar o tipo do bem; cadastre um novo"));
        }
        if let Some(w) = self
            .list_improvements()?
            .iter()
            .find(|w| w.owned_id == a.id && w.date < a.bought_on)
        {
            return Err(bad(format!(
                "a obra \"{}\" é antes da nova data de compra",
                w.description
            )));
        }
        self.conn.execute(
            "UPDATE assets_owned SET name = ?2, bought_on = ?3, price = ?4, sold_on = ?5, note = ?6,
               updated_at = ?7 WHERE id = ?1",
            params![
                a.id,
                a.name,
                a.bought_on.to_string(),
                a.price.to_string(),
                a.sold_on.map(|d| d.to_string()),
                a.note,
                now_iso()
            ],
        )?;
        self.append_event(
            "BEM",
            &a.id.clone(),
            Action::Alterada,
            Some(&json(&before)),
            Some(&json(&a)),
            None,
        )?;
        Ok(a)
    }

    /// Remove o bem e, junto, as obras dele.
    pub fn remove_owned(&mut self, id: &str) -> Result<()> {
        let before = self
            .get_owned(id)?
            .ok_or_else(|| bad("bem não encontrado"))?;
        let now = now_iso();
        self.conn.execute(
            "UPDATE asset_improvements SET removed_at = ?2 WHERE owned_id = ?1 AND removed_at IS NULL",
            params![id, now],
        )?;
        self.conn.execute(
            "UPDATE assets_owned SET removed_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        self.append_event(
            "BEM",
            id,
            Action::Removida,
            Some(&json(&before)),
            None,
            None,
        )?;
        Ok(())
    }

    pub fn list_improvements(&self) -> Result<Vec<Improvement>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT_WORK} WHERE removed_at IS NULL ORDER BY done_on, created_at"
        ))?;
        let rows = stmt.query_map([], row_to_work)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    fn get_improvement(&self, id: &str) -> Result<Option<Improvement>> {
        Ok(self
            .conn
            .query_row(
                &format!("{SELECT_WORK} WHERE id = ?1 AND removed_at IS NULL"),
                [id],
                row_to_work,
            )
            .optional()?)
    }

    fn check_work(&self, w: &mut Improvement) -> Result<()> {
        w.description = w.description.trim().to_string();
        let owner = self
            .get_owned(&w.owned_id)?
            .ok_or_else(|| bad("bem não encontrado"))?;
        w.validate(&owner).map_err(bad)
    }

    pub fn create_improvement(&mut self, mut w: Improvement) -> Result<Improvement> {
        self.check_work(&mut w)?;
        w.id = new_id()?.replacen("op-", "ob-", 1);
        let now = now_iso();
        self.conn.execute(
            "INSERT INTO asset_improvements (id, owned_id, done_on, amount, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![
                w.id,
                w.owned_id,
                w.date.to_string(),
                w.amount.to_string(),
                w.description,
                now
            ],
        )?;
        self.append_event(
            "OBRA",
            &w.id.clone(),
            Action::Criada,
            None,
            Some(&json(&w)),
            None,
        )?;
        Ok(w)
    }

    pub fn update_improvement(&mut self, mut w: Improvement) -> Result<Improvement> {
        let before = self
            .get_improvement(&w.id)?
            .ok_or_else(|| bad("obra não encontrada"))?;
        w.owned_id = before.owned_id.clone();
        self.check_work(&mut w)?;
        self.conn.execute(
            "UPDATE asset_improvements SET done_on = ?2, amount = ?3, description = ?4, updated_at = ?5
             WHERE id = ?1",
            params![
                w.id,
                w.date.to_string(),
                w.amount.to_string(),
                w.description,
                now_iso()
            ],
        )?;
        self.append_event(
            "OBRA",
            &w.id.clone(),
            Action::Alterada,
            Some(&json(&before)),
            Some(&json(&w)),
            None,
        )?;
        Ok(w)
    }

    pub fn remove_improvement(&mut self, id: &str) -> Result<()> {
        let before = self
            .get_improvement(id)?
            .ok_or_else(|| bad("obra não encontrada"))?;
        self.conn.execute(
            "UPDATE asset_improvements SET removed_at = ?2 WHERE id = ?1",
            params![id, now_iso()],
        )?;
        self.append_event(
            "OBRA",
            id,
            Action::Removida,
            Some(&json(&before)),
            None,
            None,
        )?;
        Ok(())
    }

    /// Corretagem configurada de cada conta.
    pub fn brokerage_table(&self) -> Result<BrokerageTable> {
        let mut stmt = self
            .conn
            .prepare("SELECT account, per_order, iss_pct, since FROM account_brokerage")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Brokerage {
                    per_order: dec(r.get(1)?, 1)?,
                    iss_pct: dec(r.get(2)?, 2)?,
                    since: date(r.get(3)?, 3)?,
                },
            ))
        })?;
        Ok(rows.collect::<std::result::Result<BTreeMap<_, _>, _>>()?)
    }

    /// Define (ou zera) a corretagem de uma conta para as ordens a partir de
    /// `since`. Ordens com taxa informada à mão (notas importadas) não mudam.
    pub fn set_brokerage(&mut self, account: &str, b: Brokerage) -> Result<()> {
        if self.get_account(account)?.is_none() {
            return Err(bad("conta não encontrada"));
        }
        if b.per_order < Decimal::ZERO || b.iss_pct < Decimal::ZERO || b.iss_pct > Decimal::from(20)
        {
            return Err(bad("corretagem não pode ser negativa e o ISS vai até 20%"));
        }
        let before = self.brokerage_table()?.remove(account);
        if b.per_order.is_zero() {
            self.conn.execute(
                "DELETE FROM account_brokerage WHERE account = ?1",
                [account],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO account_brokerage (account, per_order, iss_pct, since, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (account) DO UPDATE SET per_order = ?2, iss_pct = ?3, since = ?4, updated_at = ?5",
                params![
                    account,
                    b.per_order.to_string(),
                    b.iss_pct.to_string(),
                    b.since.to_string(),
                    now_iso()
                ],
            )?;
        }
        let after = (!b.per_order.is_zero()).then_some(&b);
        self.append_event(
            "CORRETAGEM",
            account,
            if before.is_none() {
                Action::Criada
            } else {
                Action::Alterada
            },
            before.as_ref().map(json).as_deref(),
            after.map(json).as_deref(),
            None,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KdfParams;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn bens_obras_e_corretagem_com_auditoria() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.2.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        let casa = v
            .create_owned(OwnedAsset {
                id: String::new(),
                kind: OwnedKind::Imovel,
                name: " Casa ".into(),
                bought_on: d(2020, 5, 10),
                price: dec!(300000),
                sold_on: None,
                note: None,
            })
            .unwrap();
        assert_eq!(casa.name, "Casa");
        let obra = v
            .create_improvement(Improvement {
                id: String::new(),
                owned_id: casa.id.clone(),
                date: d(2022, 3, 1),
                amount: dec!(50000),
                description: "Cozinha".into(),
            })
            .unwrap();
        // Mudar a compra para depois da obra não pode.
        assert!(v
            .update_owned(OwnedAsset {
                bought_on: d(2023, 1, 1),
                ..casa.clone()
            })
            .is_err());
        v.update_improvement(Improvement {
            amount: dec!(60000),
            ..obra
        })
        .unwrap();
        assert_eq!(v.list_improvements().unwrap()[0].amount, dec!(60000));
        v.remove_owned(&casa.id).unwrap();
        assert!(v.list_owned().unwrap().is_empty());
        assert!(v.list_improvements().unwrap().is_empty());

        v.set_brokerage(
            "Corretora",
            Brokerage {
                per_order: dec!(4.90),
                iss_pct: dec!(5),
                since: d(2026, 1, 1),
            },
        )
        .unwrap_err();
        assert!(v.brokerage_table().unwrap().is_empty());
    }
}
