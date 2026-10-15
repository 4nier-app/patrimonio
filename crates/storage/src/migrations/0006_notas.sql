-- Etapa 4: notas de corretagem importadas de PDF.
-- `hash` é o SHA-256 do texto da nota: a mesma nota, vinda de qualquer
-- arquivo, não entra duas vezes enquanto seus lançamentos existirem.
CREATE TABLE imported_notes (
    hash          TEXT PRIMARY KEY,
    broker        TEXT NOT NULL,
    number        TEXT,
    trade_date    TEXT NOT NULL,
    account       TEXT NOT NULL,
    operation_ids TEXT NOT NULL,
    imported_at   TEXT NOT NULL
);
