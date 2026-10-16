-- Etapa 10: eventos corporativos (bonificação, desdobramento, grupamento).
CREATE TABLE corporate_events (
    id             TEXT PRIMARY KEY,
    ticker         TEXT NOT NULL,
    kind           TEXT NOT NULL,  -- BONIFICACAO | DESDOBRAMENTO | GRUPAMENTO
    data_com       TEXT NOT NULL,  -- último dia com direito
    proportion     TEXT NOT NULL,  -- % na bonificação; N no desdobramento (1:N) e no grupamento (N:1)
    cost_per_share TEXT NOT NULL,  -- bonificação: custo por ação nova informado pela empresa
    note           TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    removed_at     TEXT
);
CREATE INDEX corporate_events_ticker ON corporate_events (ticker, data_com);
