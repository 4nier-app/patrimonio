//! Ativos, lançamentos e cotações gravados no cofre.
//!
//! Toda criação, alteração ou remoção de lançamento gera um evento no log
//! auditável com o estado antes e depois. Remover é lógico: a linha fica no
//! banco marcada com a data de remoção.

use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

use carteira_core::model::{Quote, Side};
use carteira_core::operations::{validate, Operation};
use carteira_core::returns::PriceSource;
use carteira_ledger::Action;
use chrono::NaiveDate;
use rusqlite::{params, OptionalExtension};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{now_iso, Result, Vault, VaultError};

/// Origem preferida quando há mais de uma cotação para o mesmo dia.
pub const SOURCE_PRIORITY: &[&str] = &["B3-COTAHIST", "MANUAL", "YAHOO"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub ticker: String,
    pub name: String,
    /// ACAO, ETF, FII, BDR, OUTRO.
    pub kind: String,
    pub market: String,
    pub currency: String,
    pub isin: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewQuote {
    pub ticker: String,
    pub date: NaiveDate,
    pub close: Decimal,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuoteUpdate {
    pub started_at: String,
    pub source: String,
    pub ok: bool,
    pub items: i64,
    pub message: String,
}

fn bad(msg: impl Into<String>) -> VaultError {
    VaultError::Invalid(msg.into())
}

fn dec(s: String, col: usize) -> rusqlite::Result<Decimal> {
    Decimal::from_str(&s).map_err(|e| rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(e)))
}

fn date(s: String, col: usize) -> rusqlite::Result<NaiveDate> {
    NaiveDate::parse_from_str(&s, "%Y-%m-%d")
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(e)))
}

fn new_id() -> Result<String> {
    let mut bytes = [0u8; 8];
    getrandom::getrandom(&mut bytes).map_err(|e| VaultError::Kdf(e.to_string()))?;
    Ok(format!("op-{}", hex::encode(bytes)))
}

/// Normaliza o código digitado: maiúsculas, sem espaços.
pub fn normalize_ticker(t: &str) -> String {
    t.trim().to_uppercase()
}

const SELECT_OP: &str =
    "SELECT id, account, ticker, trade_date, side, quantity, price, manual_fees, note FROM operations";

fn row_to_op(r: &rusqlite::Row<'_>) -> rusqlite::Result<Operation> {
    let side: String = r.get(4)?;
    Ok(Operation {
        id: r.get(0)?,
        account: r.get(1)?,
        asset: r.get(2)?,
        date: date(r.get(3)?, 3)?,
        side: Side::parse(&side).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, format!("lado inválido: {side}").into())
        })?,
        quantity: dec(r.get(5)?, 5)?,
        price: dec(r.get(6)?, 6)?,
        manual_fees: r.get::<_, Option<String>>(7)?.map(|s| dec(s, 7)).transpose()?,
        note: r.get(8)?,
    })
}

fn op_json(op: &Operation) -> String {
    serde_json::to_string(op).expect("operação serializável")
}

impl Vault {
    // ---------- ativos ----------

    /// Grava ou atualiza o catálogo. Retorna quantos ativos foram gravados.
    pub fn upsert_assets(&mut self, assets: &[Asset]) -> Result<usize> {
        let now = now_iso();
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO assets (ticker, name, kind, market, currency, isin, source, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT (ticker) DO UPDATE SET name = excluded.name, kind = excluded.kind,
                   market = excluded.market, currency = excluded.currency, isin = excluded.isin,
                   source = excluded.source, updated_at = excluded.updated_at",
            )?;
            for a in assets {
                stmt.execute(params![a.ticker, a.name, a.kind, a.market, a.currency, a.isin, a.source, now])?;
            }
        }
        tx.commit()?;
        Ok(assets.len())
    }

    /// Busca por código ou nome; códigos que começam com o texto vêm primeiro.
    pub fn search_assets(&self, query: &str, limit: i64) -> Result<Vec<Asset>> {
        let q = query.trim().to_uppercase();
        let mut stmt = self.conn.prepare(
            "SELECT ticker, name, kind, market, currency, isin, source FROM assets
             WHERE ticker LIKE ?1 || '%' OR upper(name) LIKE '%' || ?1 || '%'
             ORDER BY (ticker LIKE ?1 || '%') DESC, length(ticker), ticker LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![q, limit], |r| {
            Ok(Asset {
                ticker: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                market: r.get(3)?,
                currency: r.get(4)?,
                isin: r.get(5)?,
                source: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn asset_count(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT count(*) FROM assets", [], |r| r.get(0))?)
    }

    /// Garante que o ativo existe; um código fora do catálogo entra como "OUTRO".
    fn ensure_asset(&self, ticker: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO assets (ticker, name, kind, market, currency, source, updated_at)
             VALUES (?1, '', 'OUTRO', 'B3', 'BRL', 'MANUAL', ?2)",
            params![ticker, now_iso()],
        )?;
        Ok(())
    }

    // ---------- lançamentos ----------

    pub fn create_operation(&mut self, mut op: Operation) -> Result<Operation> {
        op.asset = normalize_ticker(&op.asset);
        op.account = op.account.trim().to_string();
        validate(&op).map_err(bad)?;
        op.id = new_id()?;
        self.ensure_asset(&op.asset)?;
        let now = now_iso();
        self.conn.execute(
            "INSERT INTO operations (id, account, ticker, trade_date, side, quantity, price, manual_fees, note, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                op.id,
                op.account,
                op.asset,
                op.date.to_string(),
                op.side.as_str(),
                op.quantity.to_string(),
                op.price.to_string(),
                op.manual_fees.map(|f| f.to_string()),
                op.note,
                now
            ],
        )?;
        self.append_event("OPERACAO", &op.id.clone(), Action::Criada, None, Some(&op_json(&op)), None)?;
        Ok(op)
    }

    pub fn get_operation(&self, id: &str) -> Result<Option<Operation>> {
        Ok(self
            .conn
            .query_row(&format!("{SELECT_OP} WHERE id = ?1 AND removed_at IS NULL"), [id], row_to_op)
            .optional()?)
    }

    pub fn update_operation(&mut self, mut op: Operation, reason: Option<&str>) -> Result<Operation> {
        op.asset = normalize_ticker(&op.asset);
        op.account = op.account.trim().to_string();
        validate(&op).map_err(bad)?;
        let before = self.get_operation(&op.id)?.ok_or_else(|| bad("lançamento não encontrado"))?;
        if before == op {
            return Ok(op);
        }
        self.ensure_asset(&op.asset)?;
        self.conn.execute(
            "UPDATE operations SET account = ?2, ticker = ?3, trade_date = ?4, side = ?5, quantity = ?6, price = ?7,
               manual_fees = ?8, note = ?9, updated_at = ?10 WHERE id = ?1",
            params![
                op.id,
                op.account,
                op.asset,
                op.date.to_string(),
                op.side.as_str(),
                op.quantity.to_string(),
                op.price.to_string(),
                op.manual_fees.map(|f| f.to_string()),
                op.note,
                now_iso()
            ],
        )?;
        self.append_event("OPERACAO", &op.id.clone(), Action::Alterada, Some(&op_json(&before)), Some(&op_json(&op)), reason)?;
        Ok(op)
    }

    pub fn remove_operation(&mut self, id: &str, reason: &str) -> Result<()> {
        if reason.trim().is_empty() {
            return Err(bad("informe o motivo da remoção"));
        }
        let before = self.get_operation(id)?.ok_or_else(|| bad("lançamento não encontrado"))?;
        self.conn.execute("UPDATE operations SET removed_at = ?2 WHERE id = ?1", params![id, now_iso()])?;
        self.append_event("OPERACAO", id, Action::Removida, Some(&op_json(&before)), None, Some(reason.trim()))?;
        Ok(())
    }

    /// Lançamentos ativos, do mais antigo para o mais recente.
    pub fn list_operations(&self) -> Result<Vec<Operation>> {
        let mut stmt = self.conn.prepare(&format!("{SELECT_OP} WHERE removed_at IS NULL ORDER BY trade_date, created_at"))?;
        let rows = stmt.query_map([], row_to_op)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Contas já usadas, para sugerir no formulário.
    pub fn list_accounts(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT account FROM operations WHERE removed_at IS NULL ORDER BY account")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    // ---------- cotações ----------

    /// Grava cotações. A mesma origem no mesmo dia é substituída (o último
    /// preço do dia corrente muda até o fechamento).
    pub fn save_quotes(&mut self, quotes: &[NewQuote]) -> Result<usize> {
        let now = now_iso();
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO quotes (ticker, quote_date, close, source, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (ticker, quote_date, source) DO UPDATE SET close = excluded.close, fetched_at = excluded.fetched_at",
            )?;
            for q in quotes {
                stmt.execute(params![q.ticker, q.date.to_string(), q.close.to_string(), q.source, now])?;
            }
        }
        tx.commit()?;
        Ok(quotes.len())
    }

    /// Indica se já existem cotações da origem para o ativo no intervalo.
    pub fn has_quotes(&self, ticker: &str, source: &str, from: NaiveDate, to: NaiveDate) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM quotes WHERE ticker = ?1 AND source = ?2 AND quote_date BETWEEN ?3 AND ?4",
            params![ticker, source, from.to_string(), to.to_string()],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Carrega as cotações dos ativos para uso nos cálculos.
    pub fn price_table(&self, tickers: &[String]) -> Result<PriceTable> {
        let mut table = PriceTable::default();
        let mut stmt =
            self.conn.prepare("SELECT id, ticker, quote_date, close, source FROM quotes WHERE ticker = ?1 ORDER BY quote_date")?;
        for t in tickers {
            let rows = stmt.query_map([t], |r| {
                Ok(Quote { id: r.get(0)?, asset: r.get(1)?, date: date(r.get(2)?, 2)?, close: dec(r.get(3)?, 3)?, source: r.get(4)? })
            })?;
            for q in rows {
                table.insert(q?);
            }
        }
        Ok(table)
    }

    pub fn record_quote_update(&mut self, source: &str, ok: bool, items: i64, message: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO quote_updates (started_at, source, ok, items, message) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![now_iso(), source, ok, items, message],
        )?;
        Ok(())
    }

    pub fn list_quote_updates(&self, limit: i64) -> Result<Vec<QuoteUpdate>> {
        let mut stmt = self
            .conn
            .prepare("SELECT started_at, source, ok, items, message FROM quote_updates ORDER BY id DESC LIMIT ?1")?;
        let rows = stmt.query_map([limit], |r| {
            Ok(QuoteUpdate { started_at: r.get(0)?, source: r.get(1)?, ok: r.get(2)?, items: r.get(3)?, message: r.get(4)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

/// Cotações em memória, por ativo e data, com a origem preferida por dia.
#[derive(Debug, Default)]
pub struct PriceTable {
    by_asset: HashMap<String, BTreeMap<NaiveDate, Quote>>,
}

fn rank(source: &str) -> usize {
    SOURCE_PRIORITY.iter().position(|s| *s == source).unwrap_or(SOURCE_PRIORITY.len())
}

impl PriceTable {
    pub fn insert(&mut self, q: Quote) {
        let days = self.by_asset.entry(q.asset.clone()).or_default();
        match days.get(&q.date) {
            Some(existing) if rank(&existing.source) <= rank(&q.source) => {}
            _ => {
                days.insert(q.date, q);
            }
        }
    }

    pub fn latest(&self, asset: &str) -> Option<&Quote> {
        self.by_asset.get(asset).and_then(|d| d.values().next_back())
    }
}

impl PriceSource for PriceTable {
    fn close_on_or_before(&self, asset: &str, date: NaiveDate) -> Option<Quote> {
        self.by_asset.get(asset)?.range(..=date).next_back().map(|(_, q)| q.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KdfParams;
    use rust_decimal_macros::dec;

    fn vault(dir: &std::path::Path) -> Vault {
        Vault::create_with_params(dir, "senha-de-teste-123", "0.2.0", KdfParams::fast_for_tests()).unwrap()
    }

    fn sample() -> Operation {
        Operation {
            id: String::new(),
            account: " Corretora X ".into(),
            asset: " test3 ".into(),
            date: NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
            side: Side::Compra,
            quantity: dec!(100),
            price: dec!(12.34),
            manual_fees: None,
            note: None,
        }
    }

    #[test]
    fn cria_altera_remove_e_registra_eventos() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        let op = v.create_operation(sample()).unwrap();
        assert_eq!(op.asset, "TEST3");
        assert_eq!(op.account, "Corretora X");

        let mut changed = op.clone();
        changed.quantity = dec!(200);
        v.update_operation(changed, Some("quantidade digitada errada")).unwrap();
        assert_eq!(v.list_operations().unwrap()[0].quantity, dec!(200));

        assert!(v.remove_operation(&op.id, " ").is_err());
        v.remove_operation(&op.id, "lançamento duplicado").unwrap();
        assert!(v.list_operations().unwrap().is_empty());

        let events = v.list_events(10, 0).unwrap();
        let actions: Vec<_> = events.iter().filter(|e| e.data.entity == "OPERACAO").map(|e| e.data.action).collect();
        assert_eq!(actions, vec![Action::Removida, Action::Alterada, Action::Criada]);
        let alterada = &events[1].data;
        assert!(alterada.before.as_ref().unwrap().contains("\"quantity\":\"100\""));
        assert!(alterada.after.as_ref().unwrap().contains("\"quantity\":\"200\""));
        assert_eq!(alterada.reason.as_deref(), Some("quantidade digitada errada"));
        assert!(v.verify_integrity().unwrap().ok);
    }

    #[test]
    fn recusa_lancamento_invalido() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        let mut op = sample();
        op.quantity = dec!(0);
        assert!(matches!(v.create_operation(op), Err(VaultError::Invalid(_))));
    }

    #[test]
    fn catalogo_e_busca() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        let a = |t: &str, n: &str| Asset {
            ticker: t.into(),
            name: n.into(),
            kind: "ACAO".into(),
            market: "B3".into(),
            currency: "BRL".into(),
            isin: None,
            source: "teste".into(),
        };
        v.upsert_assets(&[a("ABCD3", "EMPRESA ABC"), a("ABCD4", "EMPRESA ABC"), a("XYZW3", "OUTRA ABCD")]).unwrap();
        let found: Vec<_> = v.search_assets("abcd", 10).unwrap().into_iter().map(|a| a.ticker).collect();
        assert_eq!(found, vec!["ABCD3", "ABCD4", "XYZW3"]);
        assert_eq!(v.asset_count().unwrap(), 3);
    }

    #[test]
    fn cotacoes_preferem_fonte_oficial() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        let d = |day| NaiveDate::from_ymd_opt(2026, 2, day).unwrap();
        let q = |day, close, source: &str| NewQuote { ticker: "TEST3".into(), date: d(day), close, source: source.into() };
        v.save_quotes(&[q(10, dec!(10.00), "YAHOO"), q(10, dec!(10.01), "B3-COTAHIST"), q(11, dec!(11), "YAHOO")]).unwrap();
        v.save_quotes(&[q(11, dec!(11.50), "YAHOO")]).unwrap();
        let t = v.price_table(&["TEST3".into()]).unwrap();
        assert_eq!(t.close_on_or_before("TEST3", d(10)).unwrap().close, dec!(10.01));
        assert_eq!(t.close_on_or_before("TEST3", d(15)).unwrap().close, dec!(11.50));
        assert!(t.close_on_or_before("TEST3", d(9)).is_none());
        assert!(v.has_quotes("TEST3", "B3-COTAHIST", d(1), d(28)).unwrap());
    }
}
