-- Native graph bytes are protected source data, never diagnostics or UI catalogs.
CREATE TABLE workspace_graph_engines (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES learner(id) ON DELETE CASCADE,
    catalog TEXT NOT NULL CHECK(length(catalog)=64),
    stamp TEXT NOT NULL CHECK(length(stamp)<=512 AND json_valid(stamp)),
    checkpoint BLOB NOT NULL CHECK(typeof(checkpoint)='blob'),
    UNIQUE(workspace_id,catalog)
);
CREATE TRIGGER workspace_graph_engine_owner_fixed BEFORE UPDATE OF id,workspace_id,catalog ON workspace_graph_engines
BEGIN SELECT RAISE(ABORT,'Graph engine ownership is immutable'); END;
CREATE TABLE workspace_graph_records (
    engine_id TEXT NOT NULL REFERENCES workspace_graph_engines(id) ON DELETE CASCADE,
    key TEXT NOT NULL CHECK(json_valid(key)),
    payload BLOB NOT NULL CHECK(typeof(payload)='blob'),
    PRIMARY KEY(engine_id,key)
);
CREATE TRIGGER workspace_graph_record_owner_fixed BEFORE UPDATE OF engine_id,key ON workspace_graph_records
BEGIN SELECT RAISE(ABORT,'Graph record ownership is immutable'); END;
CREATE TABLE workspace_graph_archives (
    engine_id TEXT NOT NULL REFERENCES workspace_graph_engines(id) ON DELETE CASCADE,
    checksum TEXT NOT NULL CHECK(length(checksum)=64),
    stamp TEXT NOT NULL CHECK(length(stamp)<=512 AND json_valid(stamp)),
    payload BLOB NOT NULL CHECK(typeof(payload)='blob'),
    PRIMARY KEY(engine_id,checksum)
);
CREATE TRIGGER workspace_graph_archive_fixed BEFORE UPDATE ON workspace_graph_archives
BEGIN SELECT RAISE(ABORT,'Graph archives are immutable'); END;

-- Provider wire identity belongs to the native producer, not a consumer attempt.
-- No source content or credentials are stored here.
CREATE TABLE workspace_graph_transport_identities (
    engine_id TEXT NOT NULL REFERENCES workspace_graph_engines(id) ON DELETE CASCADE,
    execution_id TEXT NOT NULL CHECK(
        length(execution_id) BETWEEN 1 AND 20
        AND length(CAST(execution_id AS BLOB))=length(execution_id)
        AND execution_id NOT GLOB '*[^0-9]*'
        AND substr(execution_id,1,1) BETWEEN '1' AND '9'
        AND (length(execution_id)<20 OR execution_id<='18446744073709551615')
    ),
    artifact_id TEXT NOT NULL CHECK(
        length(artifact_id)=64 AND length(CAST(artifact_id AS BLOB))=64
        AND artifact_id NOT GLOB '*[^0-9a-f]*'
    ),
    operation_contract TEXT NOT NULL CHECK(length(operation_contract) BETWEEN 3 AND 700 AND json_valid(operation_contract) AND json_type(operation_contract)='text'),
    attempt_id TEXT NOT NULL UNIQUE CHECK(
        length(attempt_id)=43 AND length(CAST(attempt_id AS BLOB))=43
        AND substr(attempt_id,1,10) NOT GLOB '*[^0-9]*'
        AND substr(attempt_id,11,1)='-'
        AND substr(attempt_id,12) NOT GLOB '*[^0-9a-f]*'
    ),
    operation_id TEXT NOT NULL UNIQUE CHECK(
        length(operation_id)=32 AND length(CAST(operation_id AS BLOB))=32
        AND operation_id NOT GLOB '*[^0-9a-f]*'
    ),
    PRIMARY KEY(engine_id,execution_id)
);
CREATE TRIGGER workspace_graph_transport_identity_fixed BEFORE UPDATE ON workspace_graph_transport_identities
BEGIN SELECT RAISE(ABORT,'Graph transport identity is immutable'); END;

-- Product request identity, separate from native attempt/producer identity.
CREATE TABLE workspace_graph_runs (
 run_id TEXT PRIMARY KEY NOT NULL,
 engine_id TEXT NOT NULL REFERENCES workspace_graph_engines(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
 artifact_id TEXT NOT NULL,
 kind TEXT NOT NULL,
 context TEXT NOT NULL CHECK(json_valid(context))
);
CREATE TRIGGER workspace_graph_run_fixed BEFORE UPDATE ON workspace_graph_runs
BEGIN SELECT RAISE(ABORT,'Graph request ownership is immutable'); END;

-- Evictable product projections; native execution history is the authority.
CREATE TABLE workspace_reading_cache (
 run_id TEXT PRIMARY KEY NOT NULL REFERENCES workspace_graph_runs(run_id) ON DELETE CASCADE,
 source_key TEXT NOT NULL,
 blob_digest TEXT NOT NULL REFERENCES inference_blobs(digest),
 last_used INTEGER NOT NULL
);
CREATE INDEX workspace_reading_source ON workspace_reading_cache(source_key,last_used);

-- Execution receipts survive cache eviction. Alignment is protected source data.
CREATE TABLE workspace_graph_audio_receipts (
 id TEXT PRIMARY KEY NOT NULL,
 engine_id TEXT NOT NULL REFERENCES workspace_graph_engines(id) ON DELETE CASCADE,
 execution_id TEXT NOT NULL CHECK(length(execution_id) BETWEEN 1 AND 20 AND length(CAST(execution_id AS BLOB))=length(execution_id) AND execution_id NOT GLOB '*[^0-9]*' AND substr(execution_id,1,1)!='0' AND (length(execution_id)<20 OR execution_id<='18446744073709551615')),
 audio_digest TEXT NOT NULL CHECK(length(audio_digest)=64),
 audio_bytes INTEGER NOT NULL CHECK(audio_bytes BETWEEN 1 AND 4194304),
 alignment TEXT CHECK(alignment IS NULL OR json_valid(alignment)),
 UNIQUE(engine_id,execution_id)
);
CREATE TRIGGER workspace_graph_audio_receipt_fixed BEFORE UPDATE ON workspace_graph_audio_receipts
BEGIN SELECT RAISE(ABORT,'Graph audio receipts are immutable'); END;
CREATE TABLE workspace_graph_audio_cache (
 receipt_id TEXT PRIMARY KEY NOT NULL REFERENCES workspace_graph_audio_receipts(id) ON DELETE CASCADE,
 request_key TEXT NOT NULL,
 blob_digest TEXT NOT NULL REFERENCES inference_blobs(digest),
 last_used INTEGER NOT NULL
);
CREATE INDEX workspace_graph_audio_request ON workspace_graph_audio_cache(request_key,last_used);

-- Read-only receipt/cache unions preserve producer identities across owner types.
CREATE VIEW native_graph_audio_receipts AS
 SELECT *,0 AS workspace FROM graph_audio_receipts
 UNION ALL SELECT *,1 AS workspace FROM workspace_graph_audio_receipts;
CREATE VIEW native_graph_audio_cache AS
 SELECT *,0 AS workspace FROM graph_audio_cache
 UNION ALL SELECT *,1 AS workspace FROM workspace_graph_audio_cache;

CREATE TABLE workspace_graph_consumers (
 consumer_id TEXT PRIMARY KEY,
 run_id TEXT NOT NULL REFERENCES workspace_graph_runs(run_id),
 stream_id TEXT
);

CREATE TABLE workspace_transcription_cache (
 run_id TEXT PRIMARY KEY REFERENCES workspace_graph_runs(run_id),
 request_key TEXT NOT NULL,
 blob_digest TEXT NOT NULL REFERENCES inference_blobs(digest),
 last_used INTEGER NOT NULL
);
CREATE INDEX workspace_transcription_request ON workspace_transcription_cache(request_key,last_used);
