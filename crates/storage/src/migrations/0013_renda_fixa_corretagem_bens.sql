-- Etapa 13: renda fixa (produto, taxa e vencimento nos saldos), corretagem
-- por conta e bens (imóvel e carro) com as obras.
ALTER TABLE savings ADD COLUMN product TEXT;
ALTER TABLE savings ADD COLUMN rate_desc TEXT;
ALTER TABLE savings ADD COLUMN maturity TEXT;

CREATE TABLE account_brokerage (
    account     TEXT PRIMARY KEY REFERENCES accounts (name),
    per_order   TEXT NOT NULL,   -- corretagem fixa por ordem, em reais
    iss_pct     TEXT NOT NULL,   -- ISS sobre a corretagem, em %
    since       TEXT NOT NULL,   -- vale para ordens a partir desta data
    updated_at  TEXT NOT NULL
);

CREATE TABLE assets_owned (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,  -- IMOVEL | VEICULO
    name          TEXT NOT NULL,
    bought_on     TEXT NOT NULL,
    price         TEXT NOT NULL,  -- valor pago na compra
    sold_on       TEXT,           -- venda (some do patrimônio depois)
    note          TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    removed_at    TEXT
);
CREATE TABLE asset_improvements (
    id            TEXT PRIMARY KEY,
    owned_id      TEXT NOT NULL REFERENCES assets_owned (id),
    done_on       TEXT NOT NULL,
    amount        TEXT NOT NULL,
    description   TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    removed_at    TEXT
);
CREATE INDEX asset_improvements_by_owned ON asset_improvements (owned_id, done_on);
