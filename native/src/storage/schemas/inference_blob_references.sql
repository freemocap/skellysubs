CREATE INDEX IF NOT EXISTS ai_content_chunk_blob ON ai_content_chunks(blob_digest);
CREATE INDEX IF NOT EXISTS graph_audio_cache_blob ON graph_audio_cache(blob_digest);
CREATE INDEX IF NOT EXISTS workspace_graph_audio_cache_blob ON workspace_graph_audio_cache(blob_digest);
CREATE INDEX IF NOT EXISTS workspace_reading_cache_blob ON workspace_reading_cache(blob_digest);
CREATE INDEX IF NOT EXISTS workspace_transcription_cache_blob ON workspace_transcription_cache(blob_digest);
