-- Etapa 9: proventos (dividendos, JCP, rendimentos de FII, juros).
CREATE TABLE incomes (
    id         TEXT PRIMARY KEY,
    account    TEXT NOT NULL,
    ticker     TEXT NOT NULL,
    kind       TEXT NOT NULL,      -- DIVIDENDO | JCP | RENDIMENTO | JUROS | OUTRO
    pay_date   TEXT NOT NULL,      -- data do pagamento
    ex_date    TEXT,               -- data com (opcional)
    gross      TEXT NOT NULL,      -- valor bruto, na moeda da conta
    withheld   TEXT NOT NULL,      -- imposto retido na fonte
    currency   TEXT NOT NULL,
    note       TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    removed_at TEXT
);
CREATE INDEX incomes_pay_date ON incomes (pay_date);
