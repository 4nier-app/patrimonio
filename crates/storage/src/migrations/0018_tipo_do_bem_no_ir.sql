-- Etapa 18: tipo do imóvel ou do veículo no IR (código do grupo 01 ou 02 da
-- ficha Bens e Direitos), para a tela Bens e Direitos já mostrar o código.
ALTER TABLE assets_owned ADD COLUMN ir_code TEXT;
