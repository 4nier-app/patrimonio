-- Etapa 2: contas em dólar, transferência de custódia e PTAX.

-- Cada conta tem uma moeda. Contas existentes eram todas da B3.
CREATE TABLE accounts (
    name        TEXT PRIMARY KEY,
    currency    TEXT NOT NULL CHECK (currency IN ('BRL', 'USD')),
    created_at  TEXT NOT NULL
);

INSERT INTO accounts (name, currency, created_at)
SELECT DISTINCT account, 'BRL', strftime('%Y-%m-%dT%H:%M:%SZ', 'now') FROM operations;

-- Recria a tabela de lançamentos para aceitar a transferência de custódia.
CREATE TABLE operations_new (
    id            TEXT PRIMARY KEY,
    account       TEXT NOT NULL REFERENCES accounts (name),
    ticker        TEXT NOT NULL REFERENCES assets (ticker),
    trade_date    TEXT NOT NULL,
    side          TEXT NOT NULL CHECK (side IN ('COMPRA', 'VENDA', 'TRANSFERENCIA')),
    quantity      TEXT NOT NULL,
    price         TEXT NOT NULL,
    manual_fees   TEXT,
    note          TEXT,
    -- Transferência: conta de origem e quantidade que ficou para trás.
    from_account  TEXT REFERENCES accounts (name),
    residue       TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    removed_at    TEXT
);

INSERT INTO operations_new (id, account, ticker, trade_date, side, quantity, price, manual_fees, note, created_at, updated_at, removed_at)
SELECT id, account, ticker, trade_date, side, quantity, price, manual_fees, note, created_at, updated_at, removed_at FROM operations;

DROP TABLE operations;
ALTER TABLE operations_new RENAME TO operations;
CREATE INDEX operations_date ON operations (trade_date);
CREATE INDEX operations_ticker ON operations (ticker);

-- Câmbio do Banco Central (PTAX de fechamento), um boletim por dia útil.
CREATE TABLE fx_rates (
    id          INTEGER PRIMARY KEY,
    rate_date   TEXT NOT NULL,
    pair        TEXT NOT NULL,
    buy         TEXT NOT NULL,
    sell        TEXT NOT NULL,
    source      TEXT NOT NULL,
    fetched_at  TEXT NOT NULL,
    UNIQUE (rate_date, pair, source)
);
