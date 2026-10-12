CREATE TABLE workspace_transcription_cache (
 run_id TEXT PRIMARY KEY REFERENCES workspace_graph_runs(run_id),
 request_key TEXT NOT NULL,
 blob_digest TEXT NOT NULL REFERENCES inference_blobs(digest),
 last_used INTEGER NOT NULL
);
CREATE INDEX workspace_transcription_request ON workspace_transcription_cache(request_key,last_used);
