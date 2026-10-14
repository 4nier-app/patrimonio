-- Etapa 1: catálogo de ativos, lançamentos e cotações.
-- Valores monetários e quantidades são gravados como texto decimal exato.

CREATE TABLE assets (
    ticker      TEXT PRIMARY KEY,
    name        TEXT NOT NULL DEFAULT '',
    kind        TEXT NOT NULL DEFAULT 'OUTRO',
    market      TEXT NOT NULL DEFAULT 'B3',
    currency    TEXT NOT NULL DEFAULT 'BRL',
    isin        TEXT,
    source      TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE operations (
    id           TEXT PRIMARY KEY,
    account      TEXT NOT NULL,
    ticker       TEXT NOT NULL REFERENCES assets (ticker),
    trade_date   TEXT NOT NULL,
    side         TEXT NOT NULL CHECK (side IN ('COMPRA', 'VENDA')),
    quantity     TEXT NOT NULL,
    price        TEXT NOT NULL,
    manual_fees  TEXT,
    note         TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    removed_at   TEXT
);

CREATE INDEX operations_date ON operations (trade_date);
CREATE INDEX operations_ticker ON operations (ticker);

CREATE TABLE quotes (
    id          INTEGER PRIMARY KEY,
    ticker      TEXT NOT NULL,
    quote_date  TEXT NOT NULL,
    close       TEXT NOT NULL,
    source      TEXT NOT NULL,
    fetched_at  TEXT NOT NULL,
    UNIQUE (ticker, quote_date, source)
);

CREATE INDEX quotes_ticker_date ON quotes (ticker, quote_date);

CREATE TABLE quote_updates (
    id           INTEGER PRIMARY KEY,
    started_at   TEXT NOT NULL,
    source       TEXT NOT NULL,
    ok           INTEGER NOT NULL,
    items        INTEGER NOT NULL,
    message      TEXT NOT NULL
);
