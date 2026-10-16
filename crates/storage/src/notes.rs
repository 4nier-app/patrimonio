//! Notas de corretagem importadas de PDF.
//!
//! A leitura do PDF acontece na tela (cada corretora tem seu leitor). Aqui
//! ficam o registro da nota, para que ela não seja importada duas vezes, e a
//! busca do código da B3 pelo nome de pregão, que algumas notas trazem no
//! lugar do código.

use carteira_core::operations::{validate, Operation};
use carteira_ledger::Action;
use chrono::NaiveDate;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::records::{bad, normalize_ticker};
use crate::{now_iso, Result, Vault};

/// Uma nota pronta para gravar, já conferida pelo usuário.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteImport {
    /// SHA-256 do texto da nota.
    pub hash: String,
    pub broker: String,
    pub number: Option<String>,
    pub date: NaiveDate,
    pub account: String,
    pub operations: Vec<Operation>,
}

/// Situação de uma nota antes de importar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoteStatus {
    pub hash: String,
    /// A nota já foi importada e algum dos lançamentos dela ainda existe.
    pub imported: bool,
    pub imported_at: Option<String>,
}

impl Vault {
    pub fn note_status(&self, hash: &str) -> Result<NoteStatus> {
        let row: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT operation_ids, imported_at FROM imported_notes WHERE hash = ?1",
                [hash],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((ids, at)) = row else {
            return Ok(NoteStatus {
                hash: hash.into(),
                imported: false,
                imported_at: None,
            });
        };
        let ids: Vec<String> = serde_json::from_str(&ids).unwrap_or_default();
        let mut alive = false;
        for id in &ids {
            if self.get_operation(id)?.is_some() {
                alive = true;
                break;
            }
        }
        Ok(NoteStatus {
            hash: hash.into(),
            imported: alive,
            imported_at: alive.then_some(at),
        })
    }

    /// Grava os lançamentos da nota e registra a importação no histórico.
    /// Nada é gravado se algum lançamento for inválido.
    pub fn import_note(&mut self, note: NoteImport) -> Result<Vec<Operation>> {
        if note.operations.is_empty() {
            return Err(bad("a nota não tem lançamentos para importar"));
        }
        if self.note_status(&note.hash)?.imported {
            return Err(bad(format!(
                "a nota {} de {} já foi importada",
                note.number.as_deref().unwrap_or("sem número"),
                note.date.format("%d/%m/%Y")
            )));
        }
        let account = note.account.trim().to_string();
        let mut ops = note.operations;
        for (i, op) in ops.iter_mut().enumerate() {
            op.account = account.clone();
            op.asset = normalize_ticker(&op.asset);
            validate(op).map_err(|e| bad(format!("linha {}: {e}", i + 1)))?;
        }
        let mut created = Vec::with_capacity(ops.len());
        for op in ops {
            created.push(self.create_operation(op)?);
        }
        let ids: Vec<&str> = created.iter().map(|o| o.id.as_str()).collect();
        let ids_json = serde_json::to_string(&ids).map_err(|e| bad(e.to_string()))?;
        let now = now_iso();
        self.conn.execute(
            "INSERT OR REPLACE INTO imported_notes (hash, broker, number, trade_date, account, operation_ids, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![note.hash, note.broker, note.number, note.date.to_string(), account, ids_json, now],
        )?;
        let summary = serde_json::json!({
            "corretora": note.broker,
            "numero": note.number,
            "pregao": note.date.to_string(),
            "conta": account,
            "lancamentos": ids,
        });
        self.append_event(
            "NOTA",
            &note.hash,
            Action::Criada,
            None,
            Some(&summary.to_string()),
            None,
        )?;
        Ok(created)
    }

    /// Códigos da B3 com esse nome de pregão ("PETROBRAS") cujo número bate com
    /// a classe da nota ("PN" → 4). Mais de um resultado: o usuário escolhe.
    pub fn find_b3_by_name(&self, name: &str, class: &str) -> Result<Vec<String>> {
        let suffixes: &[&str] = match class.to_uppercase().as_str() {
            "ON" => &["3"],
            "PN" => &["4"],
            "PNA" => &["5"],
            "PNB" => &["6"],
            "PNC" => &["7"],
            "PND" => &["8"],
            "UNT" => &["11"],
            "CI" => &["11", "12", "13"],
            "DR1" | "DR2" | "DR3" => &["31", "32", "33", "34", "35", "39"],
            _ => &[],
        };
        let mut stmt = self.conn.prepare(
            "SELECT ticker FROM assets WHERE market = 'B3' AND upper(trim(name)) = upper(trim(?1)) ORDER BY ticker",
        )?;
        let tickers = stmt
            .query_map([name], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tickers
            .into_iter()
            .filter(|t| {
                suffixes.iter().any(|s| {
                    t.strip_suffix(s).is_some_and(|base| {
                        base.len() == 4 && base.chars().all(|c| c.is_ascii_alphabetic())
                    })
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KdfParams;
    use carteira_core::model::Side;
    use rust_decimal_macros::dec;

    fn op(ticker: &str, qty: rust_decimal::Decimal, price: rust_decimal::Decimal) -> Operation {
        Operation {
            id: String::new(),
            account: String::new(),
            asset: ticker.into(),
            date: NaiveDate::from_ymd_opt(2026, 3, 2).unwrap(),
            side: Side::Compra,
            quantity: qty,
            price,
            manual_fees: Some(dec!(0.05)),
            note: Some("Nota 1".into()),
            currency: "BRL".into(),
            from_account: None,
            residue: None,
            ir: None,
            iof: None,
        }
    }

    #[test]
    fn importa_uma_vez_e_libera_quando_os_lancamentos_somem() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.6.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        let note = NoteImport {
            hash: "abc".into(),
            broker: "Corretora Sintética".into(),
            number: Some("1".into()),
            date: NaiveDate::from_ymd_opt(2026, 3, 2).unwrap(),
            account: " Conta Teste ".into(),
            operations: vec![
                op("test3f", dec!(7), dec!(10.5)),
                op("TEST11", dec!(100), dec!(9.87)),
            ],
        };
        let created = v.import_note(note.clone()).unwrap();
        assert_eq!(created.len(), 2);
        assert_eq!(created[0].asset, "TEST3");
        assert_eq!(created[0].account, "Conta Teste");
        assert!(v.note_status("abc").unwrap().imported);
        assert!(v
            .import_note(note.clone())
            .unwrap_err()
            .to_string()
            .contains("já foi importada"));
        assert!(v.verify_integrity().unwrap().ok);
        for o in &created {
            v.remove_operation(&o.id, Some("teste")).unwrap();
        }
        assert!(!v.note_status("abc").unwrap().imported);
        assert_eq!(v.import_note(note).unwrap().len(), 2);
    }

    #[test]
    fn linha_invalida_nao_grava_nada() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.6.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        let note = NoteImport {
            hash: "x".into(),
            broker: "B".into(),
            number: None,
            date: NaiveDate::from_ymd_opt(2026, 3, 2).unwrap(),
            account: "Conta".into(),
            operations: vec![op("TEST3", dec!(1), dec!(1)), op("TEST4", dec!(0), dec!(1))],
        };
        assert!(v.import_note(note).is_err());
        assert!(v.list_operations().unwrap().is_empty());
    }

    #[test]
    fn acha_codigo_pelo_nome_de_pregao() {
        let tmp = tempfile::tempdir().unwrap();
        let v = Vault::create_with_params(
            tmp.path(),
            "senha-de-teste-123",
            "0.6.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        for (t, n) in [
            ("ABCD3", "EMPRESA A"),
            ("ABCD4", "EMPRESA A"),
            ("EFGH11", "FII TESTE"),
            ("XYZW3", "OUTRA"),
        ] {
            v.conn
                .execute(
                    "INSERT INTO assets (ticker, name, kind, market, currency, source, updated_at) VALUES (?1, ?2, 'ACAO', 'B3', 'BRL', 'T', '')",
                    [t, n],
                )
                .unwrap();
        }
        assert_eq!(v.find_b3_by_name("Empresa A", "PN").unwrap(), vec!["ABCD4"]);
        assert_eq!(v.find_b3_by_name("EMPRESA A", "ON").unwrap(), vec!["ABCD3"]);
        assert_eq!(
            v.find_b3_by_name("FII TESTE", "CI").unwrap(),
            vec!["EFGH11"]
        );
        assert!(v.find_b3_by_name("NAO EXISTE", "ON").unwrap().is_empty());
    }
}
