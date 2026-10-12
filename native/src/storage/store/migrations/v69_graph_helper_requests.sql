-- Explicit repeated helpers own a fresh run for an existing message.
CREATE TABLE IF NOT EXISTS graph_helper_requests (
 run_id TEXT PRIMARY KEY,
 engine_id TEXT NOT NULL REFERENCES graph_engines(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
 artifact_id TEXT NOT NULL,
 turn_id TEXT NOT NULL REFERENCES turns(id) ON DELETE CASCADE,
 message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
 scope TEXT NOT NULL,
 operation TEXT NOT NULL
);
CREATE TRIGGER IF NOT EXISTS graph_helper_request_fixed BEFORE UPDATE ON graph_helper_requests
BEGIN SELECT RAISE(ABORT,'Helper request ownership is immutable'); END;
CREATE TRIGGER IF NOT EXISTS graph_helper_request_owner BEFORE INSERT ON graph_helper_requests
WHEN NOT EXISTS(SELECT 1 FROM messages m WHERE m.id=NEW.message_id AND m.turn_id=NEW.turn_id)
 OR EXISTS(SELECT 1 FROM graph_engines e JOIN turns t ON t.id=NEW.turn_id WHERE e.id=NEW.engine_id AND e.conversation_id!=t.conversation_id)
BEGIN SELECT RAISE(ABORT,'Helper request source or conversation differs'); END;
CREATE TRIGGER IF NOT EXISTS graph_engine_helper_owners BEFORE INSERT ON graph_engines
WHEN EXISTS(SELECT 1 FROM graph_helper_requests o JOIN turns t ON t.id=o.turn_id WHERE o.engine_id=NEW.id AND t.conversation_id!=NEW.conversation_id)
BEGIN SELECT RAISE(ABORT,'Helper graph and conversation differ'); END;
DROP VIEW IF EXISTS graph_conversation_runs;
CREATE VIEW graph_conversation_runs AS
 SELECT o.run_id,o.engine_id,o.artifact_id,o.turn_id,o.channel,coalesce(e.scope,'') AS scope
 FROM turn_execution_owners o LEFT JOIN conversation_graph_effects e ON e.turn_id=o.turn_id WHERE o.executor='graph'
 UNION ALL
 SELECT run_id,engine_id,artifact_id,turn_id,'speech',scope FROM graph_speech_requests
 UNION ALL
 SELECT run_id,engine_id,artifact_id,turn_id,'helper',scope FROM graph_helper_requests;
