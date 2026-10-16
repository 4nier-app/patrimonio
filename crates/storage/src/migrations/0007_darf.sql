-- Etapa 6: DARFs marcados como pagos na tela de Impostos.
CREATE TABLE darf_payments (
    month      TEXT PRIMARY KEY,   -- competência "AAAA-MM"
    paid_on    TEXT NOT NULL,      -- data do pagamento
    amount     TEXT NOT NULL,      -- valor pago
    created_at TEXT NOT NULL
);
