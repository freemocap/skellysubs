-- Domain effects are stable across attempts; native graph records own execution.
CREATE TABLE conversation_graph_effects (
    id TEXT PRIMARY KEY NOT NULL,
    turn_id TEXT NOT NULL REFERENCES turn_execution_owners(turn_id) ON DELETE CASCADE,
    node_key TEXT NOT NULL CHECK(length(node_key)>0),
    output_port TEXT NOT NULL CHECK(length(output_port)>0),
    role TEXT NOT NULL CHECK(role='coach_reply'),
    scope TEXT NOT NULL,
    language_id TEXT NOT NULL CHECK(length(language_id)>0),
    variety_id TEXT NOT NULL CHECK(length(variety_id)>0),
    award_source TEXT NOT NULL UNIQUE CHECK(award_source='graph-effect:'||id),
    UNIQUE(turn_id,node_key,role)
);
CREATE TRIGGER conversation_graph_effect_fixed BEFORE UPDATE ON conversation_graph_effects
BEGIN SELECT RAISE(ABORT,'Graph effect attribution is immutable'); END;
CREATE TRIGGER conversation_graph_effect_owner BEFORE INSERT ON conversation_graph_effects
WHEN NOT EXISTS(
    SELECT 1 FROM turn_execution_owners o JOIN turns t ON t.id=o.turn_id
    JOIN conversations c ON c.id=t.conversation_id
    WHERE o.turn_id=NEW.turn_id AND o.executor='graph' AND o.channel='coach'
    AND c.language_id=NEW.language_id
    AND json_extract(t.context,'$.practiceSettings.varietyId')=NEW.variety_id)
BEGIN SELECT RAISE(ABORT,'Graph effect owner or attribution differs'); END;

CREATE TABLE conversation_graph_publications (
    effect_id TEXT PRIMARY KEY NOT NULL REFERENCES conversation_graph_effects(id) ON DELETE CASCADE,
    attempt_id TEXT NOT NULL CHECK(length(attempt_id) BETWEEN 1 AND 20 AND length(CAST(attempt_id AS BLOB))=length(attempt_id) AND attempt_id NOT GLOB '*[^0-9]*' AND substr(attempt_id,1,1)!='0' AND (length(attempt_id)<20 OR attempt_id<='18446744073709551615')),
    execution_id TEXT NOT NULL CHECK(length(execution_id) BETWEEN 1 AND 20 AND length(CAST(execution_id AS BLOB))=length(execution_id) AND execution_id NOT GLOB '*[^0-9]*' AND substr(execution_id,1,1)!='0' AND (length(execution_id)<20 OR execution_id<='18446744073709551615')),
    message_id TEXT NOT NULL UNIQUE REFERENCES messages(id) ON DELETE CASCADE
);
CREATE TRIGGER conversation_graph_publication_fixed BEFORE UPDATE ON conversation_graph_publications
BEGIN SELECT RAISE(ABORT,'Graph publication provenance is immutable'); END;
CREATE TRIGGER conversation_graph_publication_owner BEFORE INSERT ON conversation_graph_publications
WHEN NOT EXISTS(
    SELECT 1 FROM conversation_graph_effects e JOIN turns t ON t.id=e.turn_id
    JOIN messages m ON m.id=NEW.message_id
    WHERE e.id=NEW.effect_id AND m.turn_id=e.turn_id
    AND m.conversation_id=t.conversation_id AND m.role='assistant')
BEGIN SELECT RAISE(ABORT,'Graph publication message owner differs'); END;
