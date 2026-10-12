-- Domain assessment receipts refer to native attempts without legacy execution rows.
CREATE TABLE conversation_graph_assessments (
 id TEXT PRIMARY KEY NOT NULL,
 turn_id TEXT NOT NULL REFERENCES turn_execution_owners(turn_id) ON DELETE CASCADE,
 node_key TEXT NOT NULL CHECK(length(node_key)>0),
 attempt_id TEXT NOT NULL CHECK(length(attempt_id)>0),
 execution_id TEXT NOT NULL CHECK(length(execution_id)>0),
 message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
 kind TEXT NOT NULL CHECK(kind IN ('coach_feedback','skill_assessment')),
 result TEXT NOT NULL CHECK(json_valid(result)),
 UNIQUE(turn_id,node_key,attempt_id)
);
CREATE TRIGGER conversation_graph_assessment_fixed BEFORE UPDATE ON conversation_graph_assessments
BEGIN SELECT RAISE(ABORT,'Graph assessment provenance is immutable'); END;
CREATE TRIGGER conversation_graph_assessment_owner BEFORE INSERT ON conversation_graph_assessments
WHEN NOT EXISTS(SELECT 1 FROM turn_execution_owners o JOIN messages m ON m.turn_id=o.turn_id
 WHERE o.turn_id=NEW.turn_id AND o.executor='graph' AND o.channel='persona_reply'
 AND m.id=NEW.message_id AND m.role='user')
BEGIN SELECT RAISE(ABORT,'Graph assessment source owner differs'); END;
CREATE TABLE conversation_graph_disclosures (
 assessment_id TEXT PRIMARY KEY NOT NULL REFERENCES conversation_graph_assessments(id) ON DELETE CASCADE,
 decision TEXT NOT NULL CHECK(json_valid(decision))
);
