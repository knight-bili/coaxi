CREATE TABLE native_lifecycle_source_handle (
 id TEXT PRIMARY KEY NOT NULL, owner_thread_id TEXT NOT NULL,
 import_id TEXT NOT NULL, import_revision TEXT NOT NULL,
 provider TEXT NOT NULL, native_session_id TEXT NOT NULL,
 location_json TEXT NOT NULL CHECK(json_valid(location_json)),
 source_revision TEXT NOT NULL, revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1)),
 created_at TEXT NOT NULL
);
CREATE INDEX native_lifecycle_source_import ON native_lifecycle_source_handle(owner_thread_id,import_id,created_at);
CREATE TABLE native_lifecycle_preview (
 id TEXT PRIMARY KEY NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('continuation','steer')),
 client_id TEXT NOT NULL DEFAULT 'desktop',
 owner_thread_id TEXT NOT NULL, digest TEXT NOT NULL,
 value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=2097152),
 expires_at_ms INTEGER NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE native_lifecycle_intent (
 command_id TEXT PRIMARY KEY NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('continuation','steer')),
 core_instance_id TEXT NOT NULL,client_id TEXT NOT NULL,
 owner_thread_id TEXT NOT NULL, preview_id TEXT NOT NULL REFERENCES native_lifecycle_preview(id),
 input_digest TEXT NOT NULL, state TEXT NOT NULL,
 value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=2097152),
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE native_lifecycle_source_lease (
 source_handle_id TEXT NOT NULL REFERENCES native_lifecycle_source_handle(id),
 provider TEXT NOT NULL, native_session_id TEXT NOT NULL,
 command_id TEXT NOT NULL UNIQUE REFERENCES native_lifecycle_intent(command_id),
 state TEXT NOT NULL CHECK(state IN ('reserved','bound','unknown','released')),
 PRIMARY KEY(source_handle_id,provider,native_session_id)
);
CREATE UNIQUE INDEX native_lifecycle_intent_preview ON native_lifecycle_intent(preview_id);
CREATE TABLE native_lifecycle_steer_policy (
 singleton INTEGER PRIMARY KEY NOT NULL CHECK(singleton=1), revision INTEGER NOT NULL CHECK(revision>=0),
 value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=2097152)
);
CREATE TABLE native_lifecycle_input_delivery (
 command_id TEXT PRIMARY KEY NOT NULL REFERENCES native_lifecycle_intent(command_id),
 value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=2097152)
);
CREATE TABLE native_lifecycle_image_reservation (
 command_id TEXT PRIMARY KEY NOT NULL REFERENCES native_lifecycle_intent(command_id),
 turn_digest TEXT NOT NULL, byte_size INTEGER NOT NULL CHECK(byte_size>=0 AND byte_size<=33554432),
 state TEXT NOT NULL CHECK(state IN ('reserved','written','acknowledged','not_consumed','unknown'))
);
CREATE INDEX native_lifecycle_image_turn ON native_lifecycle_image_reservation(turn_digest,state);
CREATE TABLE native_lifecycle_material (
 source_id TEXT PRIMARY KEY NOT NULL,owner_thread_id TEXT NOT NULL,conversation_id TEXT,scope_kind TEXT NOT NULL,
 client_id TEXT NOT NULL,original_id TEXT NOT NULL,value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=8192),
 relative_path TEXT NOT NULL UNIQUE,media_type TEXT NOT NULL,created_at TEXT NOT NULL
);
