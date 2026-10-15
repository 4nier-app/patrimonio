//! Ativos, lançamentos e cotações gravados no cofre.
//!
//! Toda criação, alteração ou remoção de lançamento gera um evento no log
//! auditável com o estado antes e depois. Remover é lógico: a linha fica no
//! banco marcada com a data de remoção.

use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

use carteira_core::model::{FxRate, FxSource, Quote, Side, BRL, USD};
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

/// Conta (corretora) e a moeda em que ela opera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub name: String,
    pub currency: String,
}

/// Boletim de câmbio de um dia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewFxRate {
    pub date: NaiveDate,
    pub buy: Decimal,
    pub sell: Decimal,
    pub source: String,
}

/// Par gravado em `fx_rates` para o dólar.
pub const USD_BRL: &str = "USD/BRL";

#[derive(Debug, Clone, Serialize)]
pub struct QuoteUpdate {
    pub started_at: String,
    pub source: String,
    pub ok: bool,
    pub items: i64,
    pub message: String,
}

pub(crate) fn bad(msg: impl Into<String>) -> VaultError {
    VaultError::Invalid(msg.into())
}

fn dec(s: String, col: usize) -> rusqlite::Result<Decimal> {
    Decimal::from_str(&s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn date(s: String, col: usize) -> rusqlite::Result<NaiveDate> {
    NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn new_id() -> Result<String> {
    let mut bytes = [0u8; 8];
    getrandom::getrandom(&mut bytes).map_err(|e| VaultError::Kdf(e.to_string()))?;
    Ok(format!("op-{}", hex::encode(bytes)))
}

/// Normaliza o código digitado: maiúsculas, sem espaços. O código do mercado
/// fracionário da B3 ("ABCD3F") vira o do lote padrão ("ABCD3"): é o mesmo
/// ativo, com a mesma cotação e o mesmo preço médio.
pub fn normalize_ticker(t: &str) -> String {
    let t = t.trim().to_uppercase();
    match t.strip_suffix('F') {
        Some(base) if is_b3_code(base) => base.to_string(),
        _ => t,
    }
}

/// Quatro letras e um ou dois números, como ABCD3 ou ABCD11.
fn is_b3_code(t: &str) -> bool {
    let b = t.as_bytes();
    (b.len() == 5 || b.len() == 6)
        && b[..4].iter().all(u8::is_ascii_alphabetic)
        && b[4..].iter().all(u8::is_ascii_digit)
}

const SELECT_OP: &str = "SELECT o.id, o.account, o.ticker, o.trade_date, o.side, o.quantity, o.price, o.manual_fees, o.note,
       COALESCE(a.currency, 'BRL'), o.from_account, o.residue
     FROM operations o LEFT JOIN accounts a ON a.name = o.account";

fn row_to_op(r: &rusqlite::Row<'_>) -> rusqlite::Result<Operation> {
    let side: String = r.get(4)?;
    Ok(Operation {
        id: r.get(0)?,
        account: r.get(1)?,
        asset: r.get(2)?,
        date: date(r.get(3)?, 3)?,
        side: Side::parse(&side).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                format!("lado inválido: {side}").into(),
            )
        })?,
        quantity: dec(r.get(5)?, 5)?,
        price: dec(r.get(6)?, 6)?,
        manual_fees: r
            .get::<_, Option<String>>(7)?
            .map(|s| dec(s, 7))
            .transpose()?,
        note: r.get(8)?,
        currency: r.get(9)?,
        from_account: r.get(10)?,
        residue: r
            .get::<_, Option<String>>(11)?
            .map(|s| dec(s, 11))
            .transpose()?,
    })
}

pub(crate) fn op_json(op: &Operation) -> String {
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
                stmt.execute(params![
                    a.ticker, a.name, a.kind, a.market, a.currency, a.isin, a.source, now
                ])?;
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

    /// Um ativo do catálogo pelo código exato.
    pub fn get_asset(&self, ticker: &str) -> Result<Option<Asset>> {
        Ok(self
            .conn
            .query_row(
                "SELECT ticker, name, kind, market, currency, isin, source FROM assets WHERE ticker = ?1",
                [normalize_ticker(ticker)],
                |r| {
                    Ok(Asset {
                        ticker: r.get(0)?,
                        name: r.get(1)?,
                        kind: r.get(2)?,
                        market: r.get(3)?,
                        currency: r.get(4)?,
                        isin: r.get(5)?,
                        source: r.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn asset_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT count(*) FROM assets", [], |r| r.get(0))?)
    }

    /// Quantos ativos do catálogo de um mercado ("B3" ou "US").
    pub fn asset_count_in(&self, market: &str) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM assets WHERE market = ?1 AND source <> 'MANUAL'",
            [market],
            |r| r.get(0),
        )?)
    }

    /// Garante que o ativo existe e é da moeda da conta. Um código fora do
    /// catálogo entra como "OUTRO" no mercado da conta (B3 ou exterior).
    fn ensure_asset(&self, ticker: &str, currency: &str) -> Result<()> {
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT currency FROM assets WHERE ticker = ?1",
                [ticker],
                |r| r.get(0),
            )
            .optional()?;
        match existing {
            Some(c) if c != currency => Err(bad(format!(
                "{ticker} é negociado em {}, mas a conta é em {}",
                currency_name(&c),
                currency_name(currency)
            ))),
            Some(_) => Ok(()),
            None => {
                let market = if currency == USD { "US" } else { "B3" };
                self.conn.execute(
                    "INSERT INTO assets (ticker, name, kind, market, currency, source, updated_at)
                     VALUES (?1, '', 'OUTRO', ?2, ?3, 'MANUAL', ?4)",
                    params![ticker, market, currency, now_iso()],
                )?;
                Ok(())
            }
        }
    }

    // ---------- contas ----------

    pub fn get_account(&self, name: &str) -> Result<Option<Account>> {
        Ok(self
            .conn
            .query_row(
                "SELECT name, currency FROM accounts WHERE name = ?1",
                [name],
                |r| {
                    Ok(Account {
                        name: r.get(0)?,
                        currency: r.get(1)?,
                    })
                },
            )
            .optional()?)
    }

    /// Contas cadastradas, para sugerir no formulário.
    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, currency FROM accounts ORDER BY name")?;
        let rows = stmt.query_map([], |r| {
            Ok(Account {
                name: r.get(0)?,
                currency: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Ajusta a operação às contas: a moeda vem da conta (uma conta nova é
    /// criada com a moeda informada) e, na transferência, a conta de origem
    /// precisa existir e ter a mesma moeda.
    pub(crate) fn prepare_operation(&mut self, op: &mut Operation) -> Result<()> {
        op.asset = normalize_ticker(&op.asset);
        op.account = op.account.trim().to_string();
        op.from_account = op
            .from_account
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        if op.side != Side::Transferencia {
            op.from_account = None;
            op.residue = None;
        } else {
            op.price = Decimal::ZERO;
            op.manual_fees = None;
        }
        match self.get_account(&op.account)? {
            Some(acc) => op.currency = acc.currency,
            None => {
                if op.currency != BRL && op.currency != USD {
                    return Err(bad("moeda da conta inválida"));
                }
            }
        }
        validate(op).map_err(bad)?;
        if let Some(from) = &op.from_account {
            let origin = self
                .get_account(from)?
                .ok_or_else(|| bad(format!("a conta de origem \"{from}\" não existe")))?;
            if origin.currency != op.currency {
                return Err(bad(
                    "as contas de origem e destino precisam ter a mesma moeda",
                ));
            }
        }
        self.ensure_asset(&op.asset, &op.currency)?;
        if self.get_account(&op.account)?.is_none() {
            self.conn.execute(
                "INSERT INTO accounts (name, currency, created_at) VALUES (?1, ?2, ?3)",
                params![op.account, op.currency, now_iso()],
            )?;
        }
        Ok(())
    }

    // ---------- lançamentos ----------

    pub fn create_operation(&mut self, mut op: Operation) -> Result<Operation> {
        self.prepare_operation(&mut op)?;
        op.id = new_id()?;
        let now = now_iso();
        self.conn.execute(
            "INSERT INTO operations (id, account, ticker, trade_date, side, quantity, price, manual_fees, note,
               from_account, residue, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?11, ?12, ?10, ?10)",
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
                now,
                op.from_account,
                op.residue.map(|r| r.to_string())
            ],
        )?;
        self.append_event(
            "OPERACAO",
            &op.id.clone(),
            Action::Criada,
            None,
            Some(&op_json(&op)),
            None,
        )?;
        Ok(op)
    }

    pub fn get_operation(&self, id: &str) -> Result<Option<Operation>> {
        Ok(self
            .conn
            .query_row(
                &format!("{SELECT_OP} WHERE o.id = ?1 AND o.removed_at IS NULL"),
                [id],
                row_to_op,
            )
            .optional()?)
    }

    pub fn update_operation(
        &mut self,
        mut op: Operation,
        reason: Option<&str>,
    ) -> Result<Operation> {
        let before = self
            .get_operation(&op.id)?
            .ok_or_else(|| bad("lançamento não encontrado"))?;
        self.prepare_operation(&mut op)?;
        if before == op {
            return Ok(op);
        }
        self.conn.execute(
            "UPDATE operations SET account = ?2, ticker = ?3, trade_date = ?4, side = ?5, quantity = ?6, price = ?7,
               manual_fees = ?8, note = ?9, updated_at = ?10, from_account = ?11, residue = ?12 WHERE id = ?1",
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
                now_iso(),
                op.from_account,
                op.residue.map(|r| r.to_string())
            ],
        )?;
        self.append_event(
            "OPERACAO",
            &op.id.clone(),
            Action::Alterada,
            Some(&op_json(&before)),
            Some(&op_json(&op)),
            reason,
        )?;
        Ok(op)
    }

    pub fn remove_operation(&mut self, id: &str, reason: Option<&str>) -> Result<()> {
        let reason = reason.map(str::trim).filter(|r| !r.is_empty());
        let before = self
            .get_operation(id)?
            .ok_or_else(|| bad("lançamento não encontrado"))?;
        self.conn.execute(
            "UPDATE operations SET removed_at = ?2 WHERE id = ?1",
            params![id, now_iso()],
        )?;
        self.append_event(
            "OPERACAO",
            id,
            Action::Removida,
            Some(&op_json(&before)),
            None,
            reason,
        )?;
        Ok(())
    }

    /// Lançamentos ativos, do mais antigo para o mais recente.
    pub fn list_operations(&self) -> Result<Vec<Operation>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT_OP} WHERE o.removed_at IS NULL ORDER BY o.trade_date, o.created_at"
        ))?;
        let rows = stmt.query_map([], row_to_op)?;
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
                stmt.execute(params![
                    q.ticker,
                    q.date.to_string(),
                    q.close.to_string(),
                    q.source,
                    now
                ])?;
            }
        }
        tx.commit()?;
        Ok(quotes.len())
    }

    /// Indica se já existem cotações da origem para o ativo no intervalo.
    pub fn has_quotes(
        &self,
        ticker: &str,
        source: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM quotes WHERE ticker = ?1 AND source = ?2 AND quote_date BETWEEN ?3 AND ?4",
            params![ticker, source, from.to_string(), to.to_string()],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Data da última cotação gravada do ativo pela origem.
    pub fn last_quote_date(&self, ticker: &str, source: &str) -> Result<Option<NaiveDate>> {
        let d: Option<String> = self.conn.query_row(
            "SELECT max(quote_date) FROM quotes WHERE ticker = ?1 AND source = ?2",
            params![ticker, source],
            |r| r.get(0),
        )?;
        d.map(|s| date(s, 0)).transpose().map_err(Into::into)
    }

    /// Primeira data com cotação do ativo na fonte.
    pub fn first_quote_date(&self, ticker: &str, source: &str) -> Result<Option<NaiveDate>> {
        let d: Option<String> = self.conn.query_row(
            "SELECT min(quote_date) FROM quotes WHERE ticker = ?1 AND source = ?2",
            params![ticker, source],
            |r| r.get(0),
        )?;
        d.map(|s| date(s, 0)).transpose().map_err(Into::into)
    }

    /// Há alguma cotação do ativo (qualquer fonte) entre as duas datas?
    pub fn has_quote_between(&self, ticker: &str, from: NaiveDate, to: NaiveDate) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM quotes WHERE ticker = ?1 AND quote_date BETWEEN ?2 AND ?3",
            params![ticker, from.to_string(), to.to_string()],
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
                Ok(Quote {
                    id: r.get(0)?,
                    asset: r.get(1)?,
                    date: date(r.get(2)?, 2)?,
                    close: dec(r.get(3)?, 3)?,
                    source: r.get(4)?,
                })
            })?;
            for q in rows {
                table.insert(q?);
            }
        }
        Ok(table)
    }

    // ---------- índices de comparação ----------

    pub fn save_index_values(
        &mut self,
        series: &str,
        source: &str,
        values: &[(NaiveDate, Decimal)],
    ) -> Result<usize> {
        let now = now_iso();
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO index_values (series, value_date, value, source, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (series, value_date) DO UPDATE SET value = excluded.value, source = excluded.source,
                   fetched_at = excluded.fetched_at",
            )?;
            for (d, v) in values {
                stmt.execute(params![series, d.to_string(), v.to_string(), source, now])?;
            }
        }
        tx.commit()?;
        Ok(values.len())
    }

    /// Primeira e última data gravadas de um índice.
    pub fn index_range(&self, series: &str) -> Result<Option<(NaiveDate, NaiveDate)>> {
        let (a, b): (Option<String>, Option<String>) = self.conn.query_row(
            "SELECT min(value_date), max(value_date) FROM index_values WHERE series = ?1",
            [series],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        match (a, b) {
            (Some(a), Some(b)) => Ok(Some((date(a, 0)?, date(b, 1)?))),
            _ => Ok(None),
        }
    }

    /// Valores de um índice por data, e a fonte do mais recente.
    pub fn index_points(
        &self,
        series: &str,
    ) -> Result<(BTreeMap<NaiveDate, Decimal>, Option<String>)> {
        let mut stmt = self.conn.prepare(
            "SELECT value_date, value, source FROM index_values WHERE series = ?1 ORDER BY value_date",
        )?;
        let mut points = BTreeMap::new();
        let mut source = None;
        let rows = stmt.query_map([series], |r| {
            Ok((
                date(r.get(0)?, 0)?,
                dec(r.get(1)?, 1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (d, v, src) = row?;
            points.insert(d, v);
            source = Some(src);
        }
        Ok((points, source))
    }

    // ---------- câmbio ----------

    pub fn save_fx_rates(&mut self, rates: &[NewFxRate]) -> Result<usize> {
        let now = now_iso();
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO fx_rates (rate_date, pair, buy, sell, source, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (rate_date, pair, source) DO UPDATE SET buy = excluded.buy, sell = excluded.sell,
                   fetched_at = excluded.fetched_at",
            )?;
            for r in rates {
                stmt.execute(params![
                    r.date.to_string(),
                    USD_BRL,
                    r.buy.to_string(),
                    r.sell.to_string(),
                    r.source,
                    now
                ])?;
            }
        }
        tx.commit()?;
        Ok(rates.len())
    }

    /// Datas do primeiro e do último boletim de câmbio gravados.
    pub fn fx_range(&self) -> Result<Option<(NaiveDate, NaiveDate)>> {
        let (a, b): (Option<String>, Option<String>) = self.conn.query_row(
            "SELECT min(rate_date), max(rate_date) FROM fx_rates WHERE pair = ?1",
            [USD_BRL],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        match (a, b) {
            (Some(a), Some(b)) => Ok(Some((date(a, 0)?, date(b, 1)?))),
            _ => Ok(None),
        }
    }

    /// PTAX venda de cada dia, para converter dólar em reais.
    pub fn fx_table(&self) -> Result<FxTable> {
        let mut stmt = self.conn.prepare(
            "SELECT rate_date, sell, source FROM fx_rates WHERE pair = ?1 ORDER BY rate_date",
        )?;
        let rows = stmt.query_map([USD_BRL], |r| {
            Ok(FxRate {
                date: date(r.get(0)?, 0)?,
                rate: dec(r.get(1)?, 1)?,
                source: r.get(2)?,
            })
        })?;
        let mut table = FxTable::default();
        for r in rows {
            let r = r?;
            table.rates.insert(r.date, r);
        }
        Ok(table)
    }

    pub fn record_quote_update(
        &mut self,
        source: &str,
        ref_key: Option<&str>,
        ok: bool,
        items: i64,
        message: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO quote_updates (started_at, source, ref_key, ok, items, message) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![now_iso(), source, ref_key, ok, items, message],
        )?;
        Ok(())
    }

    /// Data (ISO) da última atualização bem-sucedida da origem, opcionalmente
    /// de um arquivo ou período específico.
    pub fn last_successful_update(
        &self,
        source: &str,
        ref_key: Option<&str>,
    ) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT max(started_at) FROM quote_updates WHERE source = ?1 AND ok = 1 AND (?2 IS NULL OR ref_key = ?2)",
                params![source, ref_key],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
    }

    pub fn list_quote_updates(&self, limit: i64) -> Result<Vec<QuoteUpdate>> {
        let mut stmt = self
            .conn
            .prepare("SELECT started_at, source, ok, items, message FROM quote_updates ORDER BY id DESC LIMIT ?1")?;
        let rows = stmt.query_map([limit], |r| {
            Ok(QuoteUpdate {
                started_at: r.get(0)?,
                source: r.get(1)?,
                ok: r.get(2)?,
                items: r.get(3)?,
                message: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

fn currency_name(c: &str) -> &'static str {
    if c == USD {
        "dólar"
    } else {
        "real"
    }
}

/// PTAX em memória, por data.
#[derive(Debug, Default)]
pub struct FxTable {
    rates: BTreeMap<NaiveDate, FxRate>,
}

impl FxTable {
    pub fn is_empty(&self) -> bool {
        self.rates.is_empty()
    }

    /// PTAX venda por data, para usar o dólar como índice de comparação.
    pub fn points(&self) -> BTreeMap<NaiveDate, Decimal> {
        self.rates.iter().map(|(d, r)| (*d, r.rate)).collect()
    }
}

impl FxSource for FxTable {
    fn usd_brl_on_or_before(&self, date: NaiveDate) -> Option<FxRate> {
        self.rates
            .range(..=date)
            .next_back()
            .map(|(_, r)| r.clone())
    }
}

/// Cotações em memória, por ativo e data, com a origem preferida por dia.
#[derive(Debug, Default)]
pub struct PriceTable {
    by_asset: HashMap<String, BTreeMap<NaiveDate, Quote>>,
}

fn rank(source: &str) -> usize {
    SOURCE_PRIORITY
        .iter()
        .position(|s| *s == source)
        .unwrap_or(SOURCE_PRIORITY.len())
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
        self.by_asset
            .get(asset)
            .and_then(|d| d.values().next_back())
    }
}

impl PriceSource for PriceTable {
    fn close_on_or_before(&self, asset: &str, date: NaiveDate) -> Option<Quote> {
        self.by_asset
            .get(asset)?
            .range(..=date)
            .next_back()
            .map(|(_, q)| q.clone())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::KdfParams;
    use rust_decimal_macros::dec;

    #[test]
    fn fracionario_vira_lote_padrao() {
        assert_eq!(normalize_ticker(" klbn4f "), "KLBN4");
        assert_eq!(normalize_ticker("ABCD11F"), "ABCD11");
        assert_eq!(normalize_ticker("ABCD3"), "ABCD3");
        // Códigos dos EUA terminados em F ficam como estão.
        assert_eq!(normalize_ticker("SCHF"), "SCHF");
        assert_eq!(normalize_ticker("F"), "F");
    }

    fn vault(dir: &std::path::Path) -> Vault {
        Vault::create_with_params(
            dir,
            "senha-de-teste-123",
            "0.2.0",
            KdfParams::fast_for_tests(),
        )
        .unwrap()
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
            currency: "BRL".into(),
            from_account: None,
            residue: None,
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
        v.update_operation(changed, Some("quantidade digitada errada"))
            .unwrap();
        assert_eq!(v.list_operations().unwrap()[0].quantity, dec!(200));

        v.remove_operation(&op.id, Some("lançamento duplicado"))
            .unwrap();
        assert!(v.list_operations().unwrap().is_empty());

        let events = v.list_events(10, 0).unwrap();
        let actions: Vec<_> = events
            .iter()
            .filter(|e| e.data.entity == "OPERACAO")
            .map(|e| e.data.action)
            .collect();
        assert_eq!(
            actions,
            vec![Action::Removida, Action::Alterada, Action::Criada]
        );
        let alterada = &events[1].data;
        assert!(alterada
            .before
            .as_ref()
            .unwrap()
            .contains("\"quantity\":\"100\""));
        assert!(alterada
            .after
            .as_ref()
            .unwrap()
            .contains("\"quantity\":\"200\""));
        assert_eq!(
            alterada.reason.as_deref(),
            Some("quantidade digitada errada")
        );
        assert!(v.verify_integrity().unwrap().ok);
    }

    #[test]
    fn recusa_lancamento_invalido() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        let mut op = sample();
        op.quantity = dec!(0);
        assert!(matches!(
            v.create_operation(op),
            Err(VaultError::Invalid(_))
        ));
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
        v.upsert_assets(&[
            a("ABCD3", "EMPRESA ABC"),
            a("ABCD4", "EMPRESA ABC"),
            a("XYZW3", "OUTRA ABCD"),
        ])
        .unwrap();
        let found: Vec<_> = v
            .search_assets("abcd", 10)
            .unwrap()
            .into_iter()
            .map(|a| a.ticker)
            .collect();
        assert_eq!(found, vec!["ABCD3", "ABCD4", "XYZW3"]);
        assert_eq!(v.asset_count().unwrap(), 3);
    }

    #[test]
    fn cotacoes_preferem_fonte_oficial() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        let d = |day| NaiveDate::from_ymd_opt(2026, 2, day).unwrap();
        let q = |day, close, source: &str| NewQuote {
            ticker: "TEST3".into(),
            date: d(day),
            close,
            source: source.into(),
        };
        v.save_quotes(&[
            q(10, dec!(10.00), "YAHOO"),
            q(10, dec!(10.01), "B3-COTAHIST"),
            q(11, dec!(11), "YAHOO"),
        ])
        .unwrap();
        v.save_quotes(&[q(11, dec!(11.50), "YAHOO")]).unwrap();
        let t = v.price_table(&["TEST3".into()]).unwrap();
        assert_eq!(
            t.close_on_or_before("TEST3", d(10)).unwrap().close,
            dec!(10.01)
        );
        assert_eq!(
            t.close_on_or_before("TEST3", d(15)).unwrap().close,
            dec!(11.50)
        );
        assert!(t.close_on_or_before("TEST3", d(9)).is_none());
        assert!(v.has_quotes("TEST3", "B3-COTAHIST", d(1), d(28)).unwrap());
        assert!(v
            .last_successful_update("B3-COTAHIST", Some("2026-02"))
            .unwrap()
            .is_none());
        v.record_quote_update("B3-COTAHIST", Some("2026-02"), true, 1, "ok")
            .unwrap();
        v.record_quote_update("B3-COTAHIST", Some("2026-03"), false, 0, "falhou")
            .unwrap();
        assert!(v
            .last_successful_update("B3-COTAHIST", Some("2026-02"))
            .unwrap()
            .is_some());
        assert!(v
            .last_successful_update("B3-COTAHIST", Some("2026-03"))
            .unwrap()
            .is_none());
        assert!(v
            .last_successful_update("B3-COTAHIST", None)
            .unwrap()
            .is_some());
    }

    #[test]
    fn conta_em_dolar_transferencia_e_ptax() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        let mut buy = sample();
        buy.account = "Exterior A".into();
        buy.asset = "abcd".into();
        buy.currency = "USD".into();
        buy.quantity = dec!(1.5);
        let buy = v.create_operation(buy).unwrap();
        assert_eq!(buy.currency, "USD");
        assert_eq!(
            v.get_account("Exterior A").unwrap().unwrap().currency,
            "USD"
        );

        // Conta existente: a moeda vem dela, não do formulário.
        let mut other = sample();
        other.account = "Exterior A".into();
        other.asset = "ABCD".into();
        assert_eq!(v.create_operation(other).unwrap().currency, "USD");

        // Ativo em dólar numa conta em reais é recusado.
        let mut wrong = sample();
        wrong.asset = "ABCD".into();
        assert!(matches!(
            v.create_operation(wrong),
            Err(VaultError::Invalid(_))
        ));

        let mut t = sample();
        t.side = Side::Transferencia;
        t.account = "Exterior B".into();
        t.currency = "USD".into();
        t.asset = "ABCD".into();
        t.quantity = dec!(1.4);
        t.from_account = Some("Exterior A".into());
        t.residue = Some(dec!(0.1));
        t.price = dec!(99);
        let t = v.create_operation(t).unwrap();
        assert_eq!(t.price, dec!(0));
        let stored = v.get_operation(&t.id).unwrap().unwrap();
        assert_eq!(stored.from_account.as_deref(), Some("Exterior A"));
        assert_eq!(stored.residue, Some(dec!(0.1)));
        assert_eq!(stored.currency, "USD");

        let mut bad_origin = t.clone();
        bad_origin.from_account = Some("Corretora X".into());
        v.create_operation(sample()).unwrap();
        assert!(
            v.update_operation(bad_origin, None).is_err(),
            "origem em reais e destino em dólar"
        );

        let d = |day| NaiveDate::from_ymd_opt(2026, 2, day).unwrap();
        v.save_fx_rates(&[
            NewFxRate {
                date: d(9),
                buy: dec!(5.10),
                sell: dec!(5.11),
                source: "BCB-PTAX".into(),
            },
            NewFxRate {
                date: d(10),
                buy: dec!(5.20),
                sell: dec!(5.21),
                source: "BCB-PTAX".into(),
            },
        ])
        .unwrap();
        let fx = v.fx_table().unwrap();
        assert_eq!(fx.usd_brl_on_or_before(d(14)).unwrap().rate, dec!(5.21));
        assert!(fx.usd_brl_on_or_before(d(8)).is_none());
        assert_eq!(v.fx_range().unwrap(), Some((d(9), d(10))));
        let accounts: Vec<_> = v
            .list_accounts()
            .unwrap()
            .into_iter()
            .map(|a| (a.name, a.currency))
            .collect();
        assert_eq!(accounts.len(), 3);
        assert!(v.verify_integrity().unwrap().ok);
    }
}
