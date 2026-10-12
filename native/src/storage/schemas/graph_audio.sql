-- Execution receipts survive cache eviction. Alignment is protected source data.
CREATE TABLE graph_audio_receipts (
 id TEXT PRIMARY KEY NOT NULL,
 engine_id TEXT NOT NULL REFERENCES graph_engines(id) ON DELETE CASCADE,
 execution_id TEXT NOT NULL CHECK(length(execution_id) BETWEEN 1 AND 20 AND length(CAST(execution_id AS BLOB))=length(execution_id) AND execution_id NOT GLOB '*[^0-9]*' AND substr(execution_id,1,1)!='0' AND (length(execution_id)<20 OR execution_id<='18446744073709551615')),
 audio_digest TEXT NOT NULL CHECK(length(audio_digest)=64),
 audio_bytes INTEGER NOT NULL CHECK(audio_bytes BETWEEN 1 AND 4194304),
 alignment TEXT CHECK(alignment IS NULL OR json_valid(alignment)),
 UNIQUE(engine_id,execution_id)
);
CREATE TRIGGER graph_audio_receipt_fixed BEFORE UPDATE ON graph_audio_receipts
BEGIN SELECT RAISE(ABORT,'Graph audio receipts are immutable'); END;
CREATE TABLE graph_audio_cache (
 receipt_id TEXT PRIMARY KEY NOT NULL REFERENCES graph_audio_receipts(id) ON DELETE CASCADE,
 request_key TEXT NOT NULL,
 blob_digest TEXT NOT NULL REFERENCES inference_blobs(digest),
 last_used INTEGER NOT NULL
);
CREATE INDEX graph_audio_request ON graph_audio_cache(request_key,last_used);
