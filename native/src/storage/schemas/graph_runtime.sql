-- Native graph bytes are protected source data, never diagnostics or UI catalogs.
CREATE TABLE graph_engines (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    catalog TEXT NOT NULL CHECK(length(catalog)=64),
    stamp TEXT NOT NULL CHECK(length(stamp)<=512 AND json_valid(stamp)),
    checkpoint BLOB NOT NULL CHECK(typeof(checkpoint)='blob'),
    UNIQUE(conversation_id,catalog)
);
CREATE TRIGGER graph_engine_owner_fixed BEFORE UPDATE OF id,conversation_id,catalog ON graph_engines
BEGIN SELECT RAISE(ABORT,'Graph engine ownership is immutable'); END;
CREATE TABLE graph_records (
    engine_id TEXT NOT NULL REFERENCES graph_engines(id) ON DELETE CASCADE,
    key TEXT NOT NULL CHECK(json_valid(key)),
    payload BLOB NOT NULL CHECK(typeof(payload)='blob'),
    PRIMARY KEY(engine_id,key)
);
CREATE TRIGGER graph_record_owner_fixed BEFORE UPDATE OF engine_id,key ON graph_records
BEGIN SELECT RAISE(ABORT,'Graph record ownership is immutable'); END;
CREATE TABLE graph_archives (
    engine_id TEXT NOT NULL REFERENCES graph_engines(id) ON DELETE CASCADE,
    checksum TEXT NOT NULL CHECK(length(checksum)=64),
    stamp TEXT NOT NULL CHECK(length(stamp)<=512 AND json_valid(stamp)),
    payload BLOB NOT NULL CHECK(typeof(payload)='blob'),
    PRIMARY KEY(engine_id,checksum)
);
CREATE TRIGGER graph_archive_fixed BEFORE UPDATE ON graph_archives
BEGIN SELECT RAISE(ABORT,'Graph archives are immutable'); END;
