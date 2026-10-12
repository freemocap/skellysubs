CREATE TABLE turn_execution_owners (
    turn_id TEXT PRIMARY KEY NOT NULL REFERENCES turns(id) ON DELETE CASCADE,
    executor TEXT NOT NULL CHECK(executor IN ('legacy','graph')),
    channel TEXT NOT NULL CHECK(channel IN ('coach','persona_reply','persona_opening','unknown')),
    engine_id TEXT REFERENCES graph_engines(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    run_id TEXT,
    artifact_id TEXT,
    CHECK((executor='legacy' AND engine_id IS NULL AND run_id IS NULL AND artifact_id IS NULL)
        OR (executor='graph' AND engine_id IS NOT NULL AND run_id IS NOT NULL AND length(run_id)>0 AND artifact_id IS NOT NULL AND length(artifact_id)>0 AND channel!='unknown')),
    UNIQUE(engine_id,run_id)
);
CREATE TRIGGER turn_execution_owner_fixed BEFORE UPDATE ON turn_execution_owners
BEGIN SELECT RAISE(ABORT,'Turn execution ownership is immutable'); END;
CREATE TRIGGER turn_execution_conversation_fixed BEFORE UPDATE OF conversation_id ON turns
WHEN NEW.conversation_id!=OLD.conversation_id AND EXISTS(SELECT 1 FROM turn_execution_owners WHERE turn_id=OLD.id)
BEGIN SELECT RAISE(ABORT,'Turn conversation ownership is immutable'); END;
CREATE TRIGGER turn_execution_owner_conversation BEFORE INSERT ON turn_execution_owners
WHEN NEW.executor='graph' AND EXISTS(
    SELECT 1 FROM graph_engines e JOIN turns t ON t.id=NEW.turn_id
    WHERE e.id=NEW.engine_id AND e.conversation_id!=t.conversation_id)
BEGIN SELECT RAISE(ABORT,'Graph and turn owners differ'); END;
-- First admission may stage the turn association before creating its engine.
CREATE TRIGGER graph_engine_turn_owners BEFORE INSERT ON graph_engines
WHEN EXISTS(SELECT 1 FROM turn_execution_owners o JOIN turns t ON t.id=o.turn_id
    WHERE o.engine_id=NEW.id AND t.conversation_id!=NEW.conversation_id)
BEGIN SELECT RAISE(ABORT,'Graph and turn owners differ'); END;
