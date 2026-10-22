CREATE TABLE portable_provider_revision (
 id TEXT NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),digest TEXT NOT NULL,
 value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=2097152),content_cipher BLOB NOT NULL,created_at TEXT NOT NULL,
 PRIMARY KEY(id,revision),UNIQUE(id,revision,digest)
);
CREATE TABLE portable_provider_head (
 id TEXT PRIMARY KEY NOT NULL,revision INTEGER NOT NULL,removed INTEGER NOT NULL CHECK(removed IN (0,1)),
 FOREIGN KEY(id,revision) REFERENCES portable_provider_revision(id,revision)
);
CREATE TABLE portable_secret_revision (
 id TEXT NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),client_id TEXT NOT NULL,
 purpose TEXT NOT NULL CHECK(purpose IN ('provider_key','member_environment')),context TEXT NOT NULL,
 cipher BLOB NOT NULL,operation_id TEXT NOT NULL,request_hmac TEXT NOT NULL,
 attached INTEGER NOT NULL DEFAULT 0 CHECK(attached IN (0,1)),expires_at_ms INTEGER NOT NULL,created_at TEXT NOT NULL,
 PRIMARY KEY(id,revision),UNIQUE(client_id,operation_id)
);
CREATE TABLE portable_member_connection (
 agent_id TEXT PRIMARY KEY NOT NULL REFERENCES agent_profile(id),revision INTEGER NOT NULL CHECK(revision>0),
 reference_json TEXT NOT NULL CHECK(json_valid(reference_json)),
 environment_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(environment_json) AND length(environment_json)<=1048576),
 updated_at TEXT NOT NULL
);
CREATE TABLE configuration_import_archive (
 id TEXT PRIMARY KEY NOT NULL,client_id TEXT NOT NULL,source_digest TEXT NOT NULL,
 source_kind TEXT NOT NULL CHECK(source_kind IN ('fusion_v1','fusion_v2','codeg_v1')),
 relative_path TEXT NOT NULL UNIQUE,metadata_json TEXT NOT NULL CHECK(json_valid(metadata_json) AND length(metadata_json)<=2097152),
 created_at TEXT NOT NULL
);
CREATE TABLE configuration_import_preview (
 id TEXT PRIMARY KEY NOT NULL,client_id TEXT NOT NULL,archive_id TEXT NOT NULL REFERENCES configuration_import_archive(id),
 kind TEXT NOT NULL CHECK(kind IN ('apply','undo')),original_command_id TEXT,
 digest TEXT NOT NULL,value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=2097152),
 local_basis_json TEXT NOT NULL CHECK(json_valid(local_basis_json) AND length(local_basis_json)<=2097152),
 expires_at_ms INTEGER NOT NULL,created_at TEXT NOT NULL
);
CREATE TABLE configuration_import_receipt (
 command_id TEXT PRIMARY KEY NOT NULL,client_id TEXT NOT NULL,core_instance_id TEXT NOT NULL,
 preview_id TEXT NOT NULL UNIQUE REFERENCES configuration_import_preview(id),request_digest TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('prepared','applied','rejected','unknown')),
 value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=2097152),
 created_at TEXT NOT NULL,updated_at TEXT NOT NULL
);
CREATE TABLE configuration_import_before (
 command_id TEXT NOT NULL REFERENCES configuration_import_receipt(command_id),domain TEXT NOT NULL,target_id TEXT NOT NULL,
 before_json TEXT NOT NULL CHECK(json_valid(before_json) AND length(before_json)<=2097152),
 after_revision TEXT NOT NULL,created_at TEXT NOT NULL,PRIMARY KEY(command_id,domain,target_id)
);
CREATE TABLE configuration_file_outbox (
 command_id TEXT NOT NULL REFERENCES configuration_import_receipt(command_id),domain TEXT NOT NULL,target_id TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('pending','applied','conflict','failed')),expected_digest TEXT NOT NULL,
 value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=2097152),error_code TEXT,
 created_at TEXT NOT NULL,updated_at TEXT NOT NULL,PRIMARY KEY(command_id,domain,target_id)
);
CREATE INDEX configuration_import_archive_client ON configuration_import_archive(client_id,created_at,id);
CREATE INDEX configuration_import_preview_client ON configuration_import_preview(client_id,expires_at_ms);
CREATE INDEX configuration_import_receipt_client ON configuration_import_receipt(client_id,created_at,command_id);
CREATE INDEX configuration_file_outbox_pending ON configuration_file_outbox(state,created_at);
