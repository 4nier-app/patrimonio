-- Etapa 15: Fiagro (fundo do agronegócio) segue as regras de imposto do fundo
-- imobiliário e passa a ficar junto com os FIIs. O catálogo da B3 já chega
-- assim; aqui só corrige os que estavam guardados com outra classe.
UPDATE assets SET kind = 'FII' WHERE market = 'B3' AND name LIKE 'FIAGRO%';
