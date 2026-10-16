-- Etapa 11: saldos e reservas (cofrinhos, FGTS, conta corrente, cartão benefício).
CREATE TABLE savings (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,  -- COFRINHO | FGTS | CONTA | BENEFICIO
    account     TEXT NOT NULL,  -- banco ou emissor (Nubank, Caixa, Swile...)
    name        TEXT NOT NULL,
    rate_pct    TEXT NOT NULL,  -- % do CDI (cofrinho), só para estimar
    tax_free    INTEGER NOT NULL DEFAULT 0,
    note        TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    removed_at  TEXT
);
CREATE TABLE savings_moves (
    id          TEXT PRIMARY KEY,
    savings_id  TEXT NOT NULL REFERENCES savings (id),
    move_date   TEXT NOT NULL,
    kind        TEXT NOT NULL,  -- DEPOSITO | RESGATE | SALDO
    amount      TEXT NOT NULL,
    ir          TEXT,
    iof         TEXT,
    note        TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    removed_at  TEXT
);
CREATE INDEX savings_moves_by_savings ON savings_moves (savings_id, move_date);
