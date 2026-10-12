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
