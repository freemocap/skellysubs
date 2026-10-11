-- Workspace-owned daily facts. Empty scope denotes app use; other scopes are language IDs.
-- No source foreign key: deleting a message must not erase a practiced day.
CREATE TABLE IF NOT EXISTS streak_days (
    scope TEXT NOT NULL,
    day INTEGER NOT NULL,
    run_length INTEGER NOT NULL CHECK(run_length > 0 AND run_length <= 4294967295),
    occurred_at INTEGER NOT NULL,
    utc_offset_seconds INTEGER NOT NULL CHECK(utc_offset_seconds BETWEEN -86400 AND 86400),
    session_id TEXT NOT NULL,
    claimed INTEGER NOT NULL DEFAULT 0 CHECK(claimed IN (0,1)),
    policy TEXT NOT NULL CHECK(policy = 'daily-streak-1'),
    PRIMARY KEY(scope,day)
);
