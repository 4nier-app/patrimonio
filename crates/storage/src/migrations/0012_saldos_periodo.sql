-- Etapa 12: período de cada saldo (desde quando existe e, se encerrado, até
-- quando), para a Evolução mostrar a linha só nesses meses.
ALTER TABLE savings ADD COLUMN since TEXT;  -- dia 1 do mês de início
ALTER TABLE savings ADD COLUMN until TEXT;  -- dia 1 do mês de encerramento
-- Saldos já cadastrados começam no mês do primeiro movimento (ou do cadastro).
UPDATE savings SET since = COALESCE(
    (SELECT substr(MIN(m.move_date), 1, 7) || '-01' FROM savings_moves m
      WHERE m.savings_id = savings.id AND m.removed_at IS NULL),
    substr(created_at, 1, 7) || '-01'
);
