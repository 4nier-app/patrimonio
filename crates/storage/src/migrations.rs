//! Migrações do esquema, aplicadas em ordem e registradas em `schema_migrations`.

use rusqlite::{params, Connection};

const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("migrations/0001_inicial.sql")),
    (2, include_str!("migrations/0002_lancamentos.sql")),
    (3, include_str!("migrations/0003_exterior.sql")),
];

pub fn run(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);",
    )?;
    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?;
    for (version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > current) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            params![version, super::now_iso()],
        )?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etapa_2_preserva_lancamentos_existentes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);",
        )
        .unwrap();
        for (version, sql) in &MIGRATIONS[..2] {
            conn.execute_batch(sql).unwrap();
            conn.execute("INSERT INTO schema_migrations VALUES (?1, 'x')", [version])
                .unwrap();
        }
        conn.execute_batch(
            "INSERT INTO assets (ticker, source, updated_at) VALUES ('TEST3', 'teste', 'x');
             INSERT INTO operations (id, account, ticker, trade_date, side, quantity, price, created_at, updated_at)
             VALUES ('op-1', 'Corretora X', 'TEST3', '2026-01-05', 'COMPRA', '10', '12.5', 'x', 'x');",
        )
        .unwrap();
        run(&conn).unwrap();
        let (side, qty): (String, String) = conn
            .query_row(
                "SELECT side, quantity FROM operations WHERE id = 'op-1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((side.as_str(), qty.as_str()), ("COMPRA", "10"));
        let currency: String = conn
            .query_row(
                "SELECT currency FROM accounts WHERE name = 'Corretora X'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(currency, "BRL");
        conn.execute(
            "INSERT INTO operations (id, account, ticker, trade_date, side, quantity, price, created_at, updated_at)
             VALUES ('op-2', 'Corretora X', 'TEST3', '2026-01-06', 'TRANSFERENCIA', '1', '0', 'x', 'x')",
            [],
        )
        .unwrap();
    }
}
