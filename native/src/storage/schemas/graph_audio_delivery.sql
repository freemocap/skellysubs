-- Delivery is owned by the adopted native consumer, independently of cached bytes.
CREATE TABLE IF NOT EXISTS graph_audio_deliveries (
 engine_id TEXT NOT NULL REFERENCES graph_engines(id) ON DELETE CASCADE,
 turn_id TEXT NOT NULL REFERENCES turns(id) ON DELETE CASCADE,
 node_key TEXT NOT NULL,
 attempt_id TEXT NOT NULL,
 receipt_id TEXT NOT NULL REFERENCES graph_audio_receipts(id) ON DELETE CASCADE,
 message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
 audio_digest TEXT NOT NULL,
 PRIMARY KEY(engine_id,attempt_id)
);
CREATE TRIGGER IF NOT EXISTS graph_audio_delivery_immutable BEFORE UPDATE ON graph_audio_deliveries
BEGIN SELECT RAISE(ABORT,'native audio delivery is immutable'); END;
