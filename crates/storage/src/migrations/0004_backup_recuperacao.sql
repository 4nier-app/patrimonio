-- Etapa 3: preferências do cofre (e-mail da chave de recuperação, último backup).
CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
