//! Eventos corporativos, com o mesmo registro de auditoria dos lançamentos.

use carteira_core::corporate::{CorporateEvent, EventKind};
use carteira_ledger::Action;
use rusqlite::{params, OptionalExtension};

use crate::records::{bad, date, dec, new_id, normalize_ticker};
use crate::{now_iso, Result, Vault};

const SELECT: &str = "SELECT id, ticker, kind, data_com, proportion, cost_per_share, note
                      FROM corporate_events";

fn row_to_event(r: &rusqlite::Row<'_>) -> rusqlite::Result<CorporateEvent> {
    let kind: String = r.get(2)?;
    Ok(CorporateEvent {
        id: r.get(0)?,
        asset: r.get(1)?,
        kind: EventKind::parse(&kind).unwrap_or(EventKind::Bonificacao),
        data_com: date(r.get(3)?, 3)?,
        proportion: dec(r.get(4)?, 4)?,
        cost_per_share: dec(r.get(5)?, 5)?,
        note: r.get(6)?,
    })
}

fn event_json(e: &CorporateEvent) -> String {
    serde_json::to_string(e).expect("evento serializável")
}

fn prepare(e: &mut CorporateEvent) -> Result<()> {
    e.asset = normalize_ticker(&e.asset);
    e.note = e
        .note
        .take()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    if e.kind != EventKind::Bonificacao {
        e.cost_per_share = rust_decimal::Decimal::ZERO;
    }
    e.validate().map_err(bad)
}

impl Vault {
    pub fn create_event(&mut self, mut e: CorporateEvent) -> Result<CorporateEvent> {
        prepare(&mut e)?;
        e.id = new_id()?.replacen("op-", "ev-", 1);
        let now = now_iso();
        self.conn.execute(
            "INSERT INTO corporate_events (id, ticker, kind, data_com, proportion, cost_per_share, note,
               created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                e.id,
                e.asset,
                e.kind.as_str(),
                e.data_com.to_string(),
                e.proportion.to_string(),
                e.cost_per_share.to_string(),
                e.note,
                now
            ],
        )?;
        self.append_event(
            "EVENTO",
            &e.id.clone(),
            Action::Criada,
            None,
            Some(&event_json(&e)),
            None,
        )?;
        Ok(e)
    }

    pub fn get_event(&self, id: &str) -> Result<Option<CorporateEvent>> {
        Ok(self
            .conn
            .query_row(
                &format!("{SELECT} WHERE id = ?1 AND removed_at IS NULL"),
                [id],
                row_to_event,
            )
            .optional()?)
    }

    pub fn update_event(
        &mut self,
        mut e: CorporateEvent,
        reason: Option<&str>,
    ) -> Result<CorporateEvent> {
        let before = self
            .get_event(&e.id)?
            .ok_or_else(|| bad("evento não encontrado"))?;
        prepare(&mut e)?;
        if before == e {
            return Ok(e);
        }
        self.conn.execute(
            "UPDATE corporate_events SET ticker = ?2, kind = ?3, data_com = ?4, proportion = ?5,
               cost_per_share = ?6, note = ?7, updated_at = ?8 WHERE id = ?1",
            params![
                e.id,
                e.asset,
                e.kind.as_str(),
                e.data_com.to_string(),
                e.proportion.to_string(),
                e.cost_per_share.to_string(),
                e.note,
                now_iso()
            ],
        )?;
        self.append_event(
            "EVENTO",
            &e.id.clone(),
            Action::Alterada,
            Some(&event_json(&before)),
            Some(&event_json(&e)),
            reason,
        )?;
        Ok(e)
    }

    pub fn remove_event(&mut self, id: &str) -> Result<()> {
        let before = self
            .get_event(id)?
            .ok_or_else(|| bad("evento não encontrado"))?;
        self.conn.execute(
            "UPDATE corporate_events SET removed_at = ?2 WHERE id = ?1",
            params![id, now_iso()],
        )?;
        self.append_event(
            "EVENTO",
            id,
            Action::Removida,
            Some(&event_json(&before)),
            None,
            None,
        )?;
        Ok(())
    }

    /// Desfaz a remoção (o botão "Desfazer" logo depois de remover). Na
    /// Auditoria, entra como criação com o motivo "remoção desfeita".
    pub fn restore_event(&mut self, id: &str) -> Result<CorporateEvent> {
        let item = self
            .conn
            .query_row(
                &format!("{SELECT} WHERE id = ?1 AND removed_at IS NOT NULL"),
                [id],
                row_to_event,
            )
            .optional()?
            .ok_or_else(|| bad("registro removido não encontrado"))?;
        self.conn.execute(
            "UPDATE corporate_events SET removed_at = NULL WHERE id = ?1",
            [id],
        )?;
        self.append_event(
            "EVENTO",
            id,
            Action::Criada,
            None,
            Some(&event_json(&item)),
            Some("remoção desfeita"),
        )?;
        Ok(item)
    }

    /// Eventos ativos, do mais antigo para o mais recente.
    pub fn list_corporate_events(&self) -> Result<Vec<CorporateEvent>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT} WHERE removed_at IS NULL ORDER BY data_com, created_at"
        ))?;
        let rows = stmt.query_map([], row_to_event)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KdfParams;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    #[test]
    fn evento_com_auditoria() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.2.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        let ev = CorporateEvent {
            id: String::new(),
            asset: "test3".into(),
            kind: EventKind::Desdobramento,
            data_com: NaiveDate::from_ymd_opt(2026, 3, 10).unwrap(),
            proportion: dec!(2),
            cost_per_share: dec!(5),
            note: None,
        };
        let saved = v.create_event(ev.clone()).unwrap();
        assert_eq!(saved.asset, "TEST3");
        assert_eq!(saved.cost_per_share, dec!(0));
        let mut bad = saved.clone();
        bad.proportion = dec!(1);
        assert!(v.update_event(bad, None).is_err());
        let mut changed = saved.clone();
        changed.proportion = dec!(3);
        v.update_event(changed.clone(), None).unwrap();
        assert_eq!(v.list_corporate_events().unwrap(), vec![changed]);
        v.remove_event(&saved.id).unwrap();
        assert!(v.list_corporate_events().unwrap().is_empty());
        assert!(v.verify_integrity().unwrap().ok);
    }
}
