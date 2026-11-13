-- Etapa 17: lista oficial da Bolsa de Londres (ETFs, ETCs e ETNs em dólar),
-- baixada todo mês. Guarda os dados da ficha de cada ativo e a data da lista.
CREATE TABLE lse_listings (
    ticker      TEXT PRIMARY KEY,
    issuer      TEXT NOT NULL,
    name        TEXT NOT NULL,
    isin        TEXT NOT NULL,
    country     TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('ETF', 'ETC', 'ETN')),
    start_date  TEXT,
    list_date   TEXT
);
