CREATE TABLE IF NOT EXISTS ai_content_records (
 id TEXT PRIMARY KEY,
 conversation_engine TEXT REFERENCES graph_engines(id) ON DELETE CASCADE,
 workspace_engine TEXT REFERENCES workspace_graph_engines(id) ON DELETE CASCADE,
 recording_id TEXT REFERENCES transcription_attempts(id) ON DELETE CASCADE,
 execution_id TEXT,
 kind TEXT NOT NULL,
 media_type TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('pending','complete','partial','interrupted')),
 availability TEXT NOT NULL CHECK(availability IN ('retained','evicted')),
 byte_count INTEGER NOT NULL DEFAULT 0 CHECK(byte_count>=0),
 digest TEXT,
 credentials_removed INTEGER NOT NULL DEFAULT 0 CHECK(credentials_removed IN (0,1)),
 last_used INTEGER NOT NULL,
 CHECK((conversation_engine IS NOT NULL)+(workspace_engine IS NOT NULL)+(recording_id IS NOT NULL)=1)
);
CREATE INDEX IF NOT EXISTS ai_content_execution ON ai_content_records(conversation_engine,workspace_engine,execution_id);
CREATE INDEX IF NOT EXISTS ai_content_recording ON ai_content_records(recording_id);
CREATE TABLE IF NOT EXISTS ai_content_chunks (
 record_id TEXT NOT NULL REFERENCES ai_content_records(id) ON DELETE CASCADE,
 sequence INTEGER NOT NULL CHECK(sequence>=0),
 blob_digest TEXT NOT NULL REFERENCES inference_blobs(digest),
 PRIMARY KEY(record_id,sequence)
) WITHOUT ROWID;
