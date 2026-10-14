//! Migrações do esquema, aplicadas em ordem e registradas em `schema_migrations`.

use rusqlite::{params, Connection};

const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("migrations/0001_inicial.sql")),
    (2, include_str!("migrations/0002_lancamentos.sql")),
];

pub fn run(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);",
    )?;
    let current: i64 = conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))?;
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
