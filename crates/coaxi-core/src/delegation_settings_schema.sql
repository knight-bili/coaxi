-- Installed by the complete delegation migration, never lazily by a read or a settings write.
CREATE TABLE delegation_settings (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    version INTEGER NOT NULL CHECK(typeof(version)='integer' AND version>=1),
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
    depth_limit INTEGER NOT NULL CHECK(typeof(depth_limit)='integer' AND depth_limit BETWEEN 1 AND 8),
    per_runtime_defaults TEXT NOT NULL
        CHECK(json_valid(per_runtime_defaults) AND json_type(per_runtime_defaults)='object'),
    completed_cache_max_mb INTEGER NOT NULL CHECK(typeof(completed_cache_max_mb)='integer' AND completed_cache_max_mb>=0),
    updated_at TEXT NOT NULL
);
INSERT INTO delegation_settings VALUES(1,1,0,1,'{}',512,datetime('now'));
