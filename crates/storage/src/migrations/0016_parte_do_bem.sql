-- Etapa 16: parte do bem que é da pessoa (bem em comum com o cônjuge).
-- O patrimônio soma só esta parte; Impostos continua com o valor total.
ALTER TABLE assets_owned ADD COLUMN share TEXT NOT NULL DEFAULT '100';
