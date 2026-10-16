-- Fundos de investimento: IR e IOF retidos pelo banco no resgate (e no
-- come-cotas), informados como no extrato. Ficam vazios nos outros ativos.
ALTER TABLE operations ADD COLUMN ir TEXT;
ALTER TABLE operations ADD COLUMN iof TEXT;
