-- Provider wire identity belongs to the native producer, not a consumer attempt.
-- No source content or credentials are stored here.
CREATE TABLE graph_transport_identities (
    engine_id TEXT NOT NULL REFERENCES graph_engines(id) ON DELETE CASCADE,
    execution_id TEXT NOT NULL CHECK(
        length(execution_id) BETWEEN 1 AND 20
        AND length(CAST(execution_id AS BLOB))=length(execution_id)
        AND execution_id NOT GLOB '*[^0-9]*'
        AND substr(execution_id,1,1) BETWEEN '1' AND '9'
        AND (length(execution_id)<20 OR execution_id<='18446744073709551615')
    ),
    artifact_id TEXT NOT NULL CHECK(
        length(artifact_id)=64 AND length(CAST(artifact_id AS BLOB))=64
        AND artifact_id NOT GLOB '*[^0-9a-f]*'
    ),
    operation_contract TEXT NOT NULL CHECK(length(operation_contract) BETWEEN 3 AND 700 AND json_valid(operation_contract) AND json_type(operation_contract)='text'),
    attempt_id TEXT NOT NULL UNIQUE CHECK(
        length(attempt_id)=43 AND length(CAST(attempt_id AS BLOB))=43
        AND substr(attempt_id,1,10) NOT GLOB '*[^0-9]*'
        AND substr(attempt_id,11,1)='-'
        AND substr(attempt_id,12) NOT GLOB '*[^0-9a-f]*'
    ),
    operation_id TEXT NOT NULL UNIQUE CHECK(
        length(operation_id)=32 AND length(CAST(operation_id AS BLOB))=32
        AND operation_id NOT GLOB '*[^0-9a-f]*'
    ),
    PRIMARY KEY(engine_id,execution_id)
);
CREATE TRIGGER graph_transport_identity_fixed BEFORE UPDATE ON graph_transport_identities
BEGIN SELECT RAISE(ABORT,'Graph transport identity is immutable'); END;
