-- Esquema inicial: log de eventos e histórico do sistema.

CREATE TABLE event_log (
    seq          INTEGER PRIMARY KEY,
    occurred_at  TEXT    NOT NULL,
    entity       TEXT    NOT NULL,
    entity_id    TEXT    NOT NULL,
    action       TEXT    NOT NULL CHECK (action IN ('CRIADA', 'ALTERADA', 'REMOVIDA')),
    before_json  TEXT,
    after_json   TEXT,
    reason       TEXT,
    app_version  TEXT    NOT NULL,
    prev_hash    TEXT    NOT NULL,
    hash         TEXT    NOT NULL UNIQUE
);

CREATE INDEX event_log_entity ON event_log (entity, entity_id);

-- O log só aceita inclusão. Alterar ou apagar um evento é recusado pelo banco;
-- uma edição feita por fora (outro programa) é detectada pela cadeia de hash.
CREATE TRIGGER event_log_no_update BEFORE UPDATE ON event_log
BEGIN
    SELECT RAISE(ABORT, 'event_log é somente inclusão');
END;

CREATE TRIGGER event_log_no_delete BEFORE DELETE ON event_log
BEGIN
    SELECT RAISE(ABORT, 'event_log é somente inclusão');
END;

CREATE TABLE app_version_history (
    version       TEXT PRIMARY KEY,
    installed_at  TEXT NOT NULL,
    changelog     TEXT NOT NULL
);
