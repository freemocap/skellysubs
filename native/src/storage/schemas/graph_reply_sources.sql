-- A future reply's source identity belongs to its declared effect, not an attempt.
-- Existing effects without a reservation retain their historical publication contract.
CREATE TABLE conversation_graph_reply_sources (
    effect_id TEXT PRIMARY KEY NOT NULL REFERENCES conversation_graph_effects(id) ON DELETE CASCADE,
    message_id TEXT NOT NULL UNIQUE CHECK(length(message_id)>0)
);
CREATE TRIGGER conversation_graph_reply_source_fixed BEFORE UPDATE ON conversation_graph_reply_sources
BEGIN SELECT RAISE(ABORT,'Graph reply source identity is immutable'); END;
CREATE TRIGGER conversation_graph_reply_source_new BEFORE INSERT ON conversation_graph_reply_sources
WHEN EXISTS(SELECT 1 FROM messages WHERE id=NEW.message_id)
 OR EXISTS(SELECT 1 FROM conversation_graph_publications WHERE effect_id=NEW.effect_id)
BEGIN SELECT RAISE(ABORT,'Graph reply source must be reserved before publication'); END;
CREATE TRIGGER conversation_graph_publication_reserved BEFORE INSERT ON conversation_graph_publications
WHEN EXISTS(SELECT 1 FROM conversation_graph_reply_sources r WHERE r.effect_id=NEW.effect_id AND r.message_id!=NEW.message_id)
BEGIN SELECT RAISE(ABORT,'Graph publication differs from reserved source identity'); END;
