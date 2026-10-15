-- Etapa 3: índices de comparação (CDI, IPCA, Ibovespa, IVVB11).
CREATE TABLE index_values (
    series     TEXT NOT NULL,
    value_date TEXT NOT NULL,
    value      TEXT NOT NULL,
    source     TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    UNIQUE (series, value_date)
);
