//! Saldos e reservas (cofrinhos, FGTS, conta corrente, cartão benefício) e
//! seus movimentos, com o mesmo registro de auditoria dos lançamentos.

use carteira_core::savings::{self, MoveKind, Savings, SavingsKind, SavingsMove};
use carteira_ledger::Action;
use chrono::{Datelike, NaiveDate};
use rusqlite::{params, OptionalExtension};

use crate::records::{bad, date, dec, new_id, Asset};
use crate::{now_iso, Result, Vault};

const SELECT: &str =
    "SELECT id, kind, account, name, rate_pct, tax_free, note, since, until FROM savings";
const SELECT_MOVE: &str =
    "SELECT id, savings_id, move_date, kind, amount, ir, iof, note FROM savings_moves";

fn row_to_savings(r: &rusqlite::Row<'_>) -> rusqlite::Result<Savings> {
    let kind: String = r.get(1)?;
    Ok(Savings {
        id: r.get(0)?,
        kind: SavingsKind::parse(&kind).unwrap_or_default(),
        account: r.get(2)?,
        name: r.get(3)?,
        rate_pct: dec(r.get(4)?, 4)?,
        tax_free: r.get::<_, i64>(5)? != 0,
        note: r.get(6)?,
        since: r
            .get::<_, Option<String>>(7)?
            .map(|s| date(s, 7))
            .transpose()?,
        until: r
            .get::<_, Option<String>>(8)?
            .map(|s| date(s, 8))
            .transpose()?,
    })
}

fn opt_dec(v: Option<String>, col: usize) -> rusqlite::Result<Option<rust_decimal::Decimal>> {
    v.map(|s| dec(s, col)).transpose()
}

fn row_to_move(r: &rusqlite::Row<'_>) -> rusqlite::Result<SavingsMove> {
    let kind: String = r.get(3)?;
    Ok(SavingsMove {
        id: r.get(0)?,
        savings_id: r.get(1)?,
        date: date(r.get(2)?, 2)?,
        kind: MoveKind::parse(&kind).unwrap_or(MoveKind::Saldo),
        amount: dec(r.get(4)?, 4)?,
        ir: opt_dec(r.get(5)?, 5)?,
        iof: opt_dec(r.get(6)?, 6)?,
        note: r.get(7)?,
    })
}

fn json<T: serde::Serialize>(t: &T) -> String {
    serde_json::to_string(t).expect("registro serializável")
}

fn clean(note: Option<String>) -> Option<String> {
    note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty())
}

fn prepare(s: &mut Savings) -> Result<()> {
    s.account = s.account.trim().to_string();
    s.name = s.name.trim().to_string();
    s.note = clean(s.note.take());
    // Desde e até valem pelo mês: guarda o dia 1. Sem "desde", começa no mês
    // atual (a tela sempre pede).
    let month = |d: NaiveDate| d.with_day(1).expect("dia 1 existe");
    s.since = Some(month(
        s.since.unwrap_or_else(|| chrono::Local::now().date_naive()),
    ));
    s.until = s.until.map(month);
    // FGTS não tem banco; o nome só é pedido do segundo FGTS em diante.
    if s.kind == SavingsKind::Fgts {
        s.account.clear();
    }
    if s.kind != SavingsKind::Cofrinho {
        s.rate_pct = rust_decimal::Decimal::ONE_HUNDRED;
        s.tax_free = true;
    }
    s.validate().map_err(bad)
}

fn prepare_move(m: &mut SavingsMove) -> Result<()> {
    m.note = clean(m.note.take());
    if m.kind != MoveKind::Resgate {
        m.ir = None;
        m.iof = None;
    }
    m.validate().map_err(bad)
}

impl Vault {
    /// Cofrinho entra no catálogo como ativo "RF:<id>", para a carteira e o
    /// Painel mostrarem o nome.
    fn catalog_savings(&mut self, s: &Savings) -> Result<()> {
        if !s.kind.is_investment() {
            return Ok(());
        }
        self.upsert_assets(&[Asset {
            ticker: savings::ticker(&s.id),
            name: format!("{} · {}", s.title(), s.account),
            kind: savings::KIND.into(),
            market: savings::KIND.into(),
            currency: carteira_core::model::BRL.into(),
            isin: None,
            source: savings::SOURCE.into(),
        }])?;
        Ok(())
    }

    /// Do mesmo tipo e no mesmo banco, só um pode ficar sem nome: o segundo
    /// Itaú, ou o FGTS de outra pessoa, precisa de um para não confundir.
    fn check_name(&self, s: &Savings) -> Result<()> {
        if s.own_name().is_some() {
            return Ok(());
        }
        let bank = s.bank().to_lowercase();
        let taken = self.list_savings()?.iter().any(|o| {
            o.id != s.id
                && o.kind == s.kind
                && o.bank().to_lowercase() == bank
                && o.own_name().is_none()
        });
        if !taken {
            return Ok(());
        }
        Err(bad(if s.kind == SavingsKind::Fgts {
            "já existe um FGTS sem nome; dê um nome a este (de quem é o FGTS)".to_string()
        } else {
            format!(
                "já existe {} {} sem nome; dê um nome a este para diferenciar",
                s.kind.label().to_lowercase(),
                s.account
            )
        }))
    }

    pub fn create_savings(&mut self, mut s: Savings) -> Result<Savings> {
        prepare(&mut s)?;
        self.check_name(&s)?;
        s.id = new_id()?.replacen("op-", "sd-", 1);
        let now = now_iso();
        self.conn.execute(
            "INSERT INTO savings (id, kind, account, name, rate_pct, tax_free, note, since, until, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?9, ?10, ?8, ?8)",
            params![
                s.id,
                s.kind.as_str(),
                s.account,
                s.name,
                s.rate_pct.to_string(),
                s.tax_free as i64,
                s.note,
                now,
                s.since.map(|d| d.to_string()),
                s.until.map(|d| d.to_string())
            ],
        )?;
        self.catalog_savings(&s)?;
        self.append_event(
            "SALDO",
            &s.id.clone(),
            Action::Criada,
            None,
            Some(&json(&s)),
            None,
        )?;
        Ok(s)
    }

    pub fn get_savings(&self, id: &str) -> Result<Option<Savings>> {
        Ok(self
            .conn
            .query_row(
                &format!("{SELECT} WHERE id = ?1 AND removed_at IS NULL"),
                [id],
                row_to_savings,
            )
            .optional()?)
    }

    pub fn update_savings(&mut self, mut s: Savings) -> Result<Savings> {
        let before = self
            .get_savings(&s.id)?
            .ok_or_else(|| bad("saldo não encontrado"))?;
        prepare(&mut s)?;
        self.check_name(&s)?;
        if s.kind.is_investment() != before.kind.is_investment() {
            return Err(bad(
                "não dá para transformar um cofrinho em outro tipo de saldo (ou o contrário); cadastre um novo",
            ));
        }
        if before == s {
            return Ok(s);
        }
        self.conn.execute(
            "UPDATE savings SET kind = ?2, account = ?3, name = ?4, rate_pct = ?5, tax_free = ?6,
               note = ?7, updated_at = ?8, since = ?9, until = ?10 WHERE id = ?1",
            params![
                s.id,
                s.kind.as_str(),
                s.account,
                s.name,
                s.rate_pct.to_string(),
                s.tax_free as i64,
                s.note,
                now_iso(),
                s.since.map(|d| d.to_string()),
                s.until.map(|d| d.to_string())
            ],
        )?;
        self.catalog_savings(&s)?;
        self.append_event(
            "SALDO",
            &s.id.clone(),
            Action::Alterada,
            Some(&json(&before)),
            Some(&json(&s)),
            None,
        )?;
        Ok(s)
    }

    /// Remove o saldo e, junto, todos os movimentos dele.
    pub fn remove_savings(&mut self, id: &str) -> Result<()> {
        let before = self
            .get_savings(id)?
            .ok_or_else(|| bad("saldo não encontrado"))?;
        let now = now_iso();
        self.conn.execute(
            "UPDATE savings SET removed_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        self.append_event(
            "SALDO",
            id,
            Action::Removida,
            Some(&json(&before)),
            None,
            None,
        )?;
        Ok(())
    }

    pub fn restore_savings(&mut self, id: &str) -> Result<Savings> {
        let item = self
            .conn
            .query_row(
                &format!("{SELECT} WHERE id = ?1 AND removed_at IS NOT NULL"),
                [id],
                row_to_savings,
            )
            .optional()?
            .ok_or_else(|| bad("registro removido não encontrado"))?;
        self.conn
            .execute("UPDATE savings SET removed_at = NULL WHERE id = ?1", [id])?;
        self.append_event(
            "SALDO",
            id,
            Action::Criada,
            None,
            Some(&json(&item)),
            Some("remoção desfeita"),
        )?;
        Ok(item)
    }

    pub fn list_savings(&self) -> Result<Vec<Savings>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT} WHERE removed_at IS NULL ORDER BY account, name, created_at"
        ))?;
        let rows = stmt.query_map([], row_to_savings)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn create_savings_move(&mut self, mut m: SavingsMove) -> Result<SavingsMove> {
        prepare_move(&mut m)?;
        if self.get_savings(&m.savings_id)?.is_none() {
            return Err(bad("saldo não encontrado"));
        }
        m.id = new_id()?.replacen("op-", "sm-", 1);
        let now = now_iso();
        self.conn.execute(
            "INSERT INTO savings_moves (id, savings_id, move_date, kind, amount, ir, iof, note,
               created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                m.id,
                m.savings_id,
                m.date.to_string(),
                m.kind.as_str(),
                m.amount.to_string(),
                m.ir.map(|v| v.to_string()),
                m.iof.map(|v| v.to_string()),
                m.note,
                now
            ],
        )?;
        self.append_event(
            "SALDO_MOV",
            &m.id.clone(),
            Action::Criada,
            None,
            Some(&json(&m)),
            None,
        )?;
        Ok(m)
    }

    fn get_savings_move(&self, id: &str) -> Result<Option<SavingsMove>> {
        Ok(self
            .conn
            .query_row(
                &format!("{SELECT_MOVE} WHERE id = ?1 AND removed_at IS NULL"),
                [id],
                row_to_move,
            )
            .optional()?)
    }

    pub fn update_savings_move(&mut self, mut m: SavingsMove) -> Result<SavingsMove> {
        let before = self
            .get_savings_move(&m.id)?
            .ok_or_else(|| bad("movimento não encontrado"))?;
        prepare_move(&mut m)?;
        m.savings_id = before.savings_id.clone();
        if before == m {
            return Ok(m);
        }
        self.conn.execute(
            "UPDATE savings_moves SET move_date = ?2, kind = ?3, amount = ?4, ir = ?5, iof = ?6,
               note = ?7, updated_at = ?8 WHERE id = ?1",
            params![
                m.id,
                m.date.to_string(),
                m.kind.as_str(),
                m.amount.to_string(),
                m.ir.map(|v| v.to_string()),
                m.iof.map(|v| v.to_string()),
                m.note,
                now_iso()
            ],
        )?;
        self.append_event(
            "SALDO_MOV",
            &m.id.clone(),
            Action::Alterada,
            Some(&json(&before)),
            Some(&json(&m)),
            None,
        )?;
        Ok(m)
    }

    pub fn remove_savings_move(&mut self, id: &str) -> Result<()> {
        let before = self
            .get_savings_move(id)?
            .ok_or_else(|| bad("movimento não encontrado"))?;
        self.conn.execute(
            "UPDATE savings_moves SET removed_at = ?2 WHERE id = ?1",
            params![id, now_iso()],
        )?;
        self.append_event(
            "SALDO_MOV",
            id,
            Action::Removida,
            Some(&json(&before)),
            None,
            None,
        )?;
        Ok(())
    }

    pub fn restore_savings_move(&mut self, id: &str) -> Result<SavingsMove> {
        let item = self
            .conn
            .query_row(
                &format!("{SELECT_MOVE} WHERE id = ?1 AND removed_at IS NOT NULL"),
                [id],
                row_to_move,
            )
            .optional()?
            .ok_or_else(|| bad("registro removido não encontrado"))?;
        self.conn.execute(
            "UPDATE savings_moves SET removed_at = NULL WHERE id = ?1",
            [id],
        )?;
        self.append_event(
            "SALDO_MOV",
            id,
            Action::Criada,
            None,
            Some(&json(&item)),
            Some("remoção desfeita"),
        )?;
        Ok(item)
    }

    /// Movimentos ativos de saldos ativos, por data.
    pub fn list_savings_moves(&self) -> Result<Vec<SavingsMove>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT_MOVE} WHERE removed_at IS NULL
               AND savings_id IN (SELECT id FROM savings WHERE removed_at IS NULL)
             ORDER BY move_date, created_at"
        ))?;
        let rows = stmt.query_map([], row_to_move)?;
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
    fn so_um_fgts_sem_nome() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.2.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        // FGTS: sem banco; o primeiro sem nome, o segundo precisa de um.
        let fgts = |name: &str| Savings {
            id: String::new(),
            kind: SavingsKind::Fgts,
            account: "Caixa".into(),
            name: name.into(),
            rate_pct: dec!(0),
            tax_free: false,
            note: None,
            since: None,
            until: None,
        };
        let f1 = v.create_savings(fgts("")).unwrap();
        assert_eq!(f1.account, "");
        assert!(v.create_savings(fgts("")).is_err());
        assert!(
            v.create_savings(fgts("FGTS")).is_err(),
            "igual ao tipo é sem nome"
        );
        let f2 = v.create_savings(fgts("Esposa")).unwrap();
        let mut unnamed = f2.clone();
        unnamed.name = String::new();
        assert!(v.update_savings(unnamed).is_err());
        v.update_savings(f1).unwrap();

        // Conta: o segundo Itaú precisa de nome; outro banco, não.
        let conta = |account: &str, name: &str| Savings {
            kind: SavingsKind::Conta,
            account: account.into(),
            ..fgts(name)
        };
        v.create_savings(conta("Itaú", "")).unwrap();
        assert!(v.create_savings(conta("itaú", "")).is_err());
        v.create_savings(conta("Bradesco", "")).unwrap();
        v.create_savings(conta("Itaú", "Conjunta")).unwrap();
    }

    #[test]
    fn saldos_e_movimentos_com_auditoria() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.2.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        let s = v
            .create_savings(Savings {
                id: String::new(),
                kind: SavingsKind::Cofrinho,
                account: " Banco X ".into(),
                name: "Reserva".into(),
                rate_pct: dec!(105),
                tax_free: false,
                note: None,
                since: None,
                until: None,
            })
            .unwrap();
        assert_eq!(s.account, "Banco X");
        let asset = v.get_asset(&savings::ticker(&s.id)).unwrap().unwrap();
        assert_eq!(asset.name, "Reserva · Banco X");
        let vr = v
            .create_savings(Savings {
                id: String::new(),
                kind: SavingsKind::Beneficio,
                account: "Cartão Y".into(),
                name: "Refeição".into(),
                rate_pct: dec!(0),
                tax_free: false,
                note: None,
                since: None,
                until: None,
            })
            .unwrap();
        assert!(v.get_asset(&savings::ticker(&vr.id)).unwrap().is_none());
        let mut changed = vr.clone();
        changed.kind = SavingsKind::Cofrinho;
        assert!(v.update_savings(changed).is_err());

        let m = v
            .create_savings_move(SavingsMove {
                id: String::new(),
                savings_id: s.id.clone(),
                date: NaiveDate::from_ymd_opt(2026, 3, 2).unwrap(),
                kind: MoveKind::Deposito,
                amount: dec!(1000),
                ir: Some(dec!(1)),
                iof: None,
                note: None,
            })
            .unwrap();
        assert_eq!(m.ir, None, "IR só no resgate");
        let mut m2 = m.clone();
        m2.amount = dec!(1200);
        v.update_savings_move(m2.clone()).unwrap();
        assert_eq!(v.list_savings_moves().unwrap(), vec![m2]);

        v.remove_savings(&s.id).unwrap();
        assert!(v.list_savings_moves().unwrap().is_empty());
        v.restore_savings(&s.id).unwrap();
        assert_eq!(v.list_savings_moves().unwrap().len(), 1);
        v.remove_savings_move(&m.id).unwrap();
        v.restore_savings_move(&m.id).unwrap();
        assert_eq!(v.list_savings().unwrap().len(), 2);
        assert!(v.verify_integrity().unwrap().ok);
    }
}
