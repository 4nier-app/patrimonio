-- Etapa 14: custo para trazer ao Brasil o dinheiro de uma conta em dólar
-- (taxa de conversão da corretora; o IOF é fixo no aplicativo). Com ele, o
-- saldo da conta é mostrado pelo valor de resgate.
CREATE TABLE account_exit_cost (
    account     TEXT PRIMARY KEY REFERENCES accounts (name),
    fee_pct     TEXT NOT NULL,   -- taxa de conversão da corretora, em %
    updated_at  TEXT NOT NULL
);
