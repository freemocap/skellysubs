-- Evictable product projections; native execution history is the authority.
CREATE TABLE workspace_reading_cache (
 run_id TEXT PRIMARY KEY NOT NULL REFERENCES workspace_graph_runs(run_id) ON DELETE CASCADE,
 source_key TEXT NOT NULL,
 blob_digest TEXT NOT NULL REFERENCES inference_blobs(digest),
 last_used INTEGER NOT NULL
);
CREATE INDEX workspace_reading_source ON workspace_reading_cache(source_key,last_used);
