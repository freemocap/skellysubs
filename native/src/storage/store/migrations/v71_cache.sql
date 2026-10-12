CREATE TABLE IF NOT EXISTS inference_cache_settings (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1),
 capacity_bytes INTEGER NOT NULL CHECK(capacity_bytes BETWEEN 0 AND 104857600000),
 clock INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO inference_cache_settings(singleton,capacity_bytes) VALUES(1,268435456);
CREATE TABLE IF NOT EXISTS inference_blobs (
 digest TEXT PRIMARY KEY, payload BLOB NOT NULL CHECK(length(payload)>0)
);
