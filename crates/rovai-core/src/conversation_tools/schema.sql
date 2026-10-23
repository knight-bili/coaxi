CREATE TABLE conversation_tool_policy_revision (
 revision INTEGER PRIMARY KEY NOT NULL CHECK(revision>0),digest TEXT NOT NULL,
 value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=131072),created_at TEXT NOT NULL
);
CREATE TABLE conversation_tool_policy_head (
 singleton INTEGER PRIMARY KEY NOT NULL CHECK(singleton=1),revision INTEGER NOT NULL REFERENCES conversation_tool_policy_revision(revision)
);
CREATE TABLE conversation_tool_run (
 agent_run_id TEXT NOT NULL REFERENCES agent_run(id),execution_epoch INTEGER NOT NULL CHECK(execution_epoch>=0),
 agent_id TEXT NOT NULL REFERENCES agent_profile(id),thread_id TEXT NOT NULL,conversation_id TEXT,
 scope_kind TEXT NOT NULL CHECK(scope_kind IN ('public','single_chat','delegation_child')),
 policy_revision INTEGER NOT NULL CHECK(policy_revision>=0),policy_digest TEXT NOT NULL,catalog_digest TEXT NOT NULL,
 frozen_json TEXT NOT NULL CHECK(json_valid(frozen_json) AND length(frozen_json)<=262144),
 revoked_at TEXT,created_at TEXT NOT NULL,PRIMARY KEY(agent_run_id,execution_epoch)
);
CREATE TABLE conversation_tool_source (
 id TEXT PRIMARY KEY NOT NULL,owner_thread_id TEXT NOT NULL,source_kind TEXT NOT NULL CHECK(source_kind IN ('public','single_chat','delegation_child','imported')),
 conversation_id TEXT,import_id TEXT,import_revision TEXT,legacy_session_id INTEGER,
 locator_cipher BLOB NOT NULL,created_at TEXT NOT NULL,
 CHECK((source_kind='imported' AND import_id IS NOT NULL AND import_revision IS NOT NULL AND conversation_id IS NULL)
    OR (source_kind<>'imported' AND conversation_id IS NOT NULL AND import_id IS NULL AND import_revision IS NULL))
);
CREATE UNIQUE INDEX conversation_tool_import_source ON conversation_tool_source(owner_thread_id,import_id,import_revision)
 WHERE source_kind='imported';
CREATE TABLE conversation_tool_grant_preview (
 id TEXT PRIMARY KEY NOT NULL,client_id TEXT NOT NULL,scope_digest TEXT NOT NULL,source_id TEXT NOT NULL REFERENCES conversation_tool_source(id),
 digest TEXT NOT NULL,value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=131072),
 expires_at_ms INTEGER NOT NULL,created_at TEXT NOT NULL
);
CREATE TABLE conversation_tool_source_grant (
 id TEXT PRIMARY KEY NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),client_id TEXT NOT NULL,
 scope_digest TEXT NOT NULL,source_id TEXT NOT NULL REFERENCES conversation_tool_source(id),
 value_json TEXT NOT NULL CHECK(json_valid(value_json) AND length(value_json)<=131072),
 expires_at_ms INTEGER NOT NULL,revoked_at TEXT,created_at TEXT NOT NULL
);
CREATE INDEX conversation_tool_scope_grants ON conversation_tool_source_grant(scope_digest,id);
CREATE TABLE conversation_tool_request (
 id TEXT PRIMARY KEY NOT NULL,agent_run_id TEXT NOT NULL,execution_epoch INTEGER NOT NULL CHECK(execution_epoch>=0),
 tool_name TEXT NOT NULL CHECK(tool_name IN ('ask_user_question','create_automation','create_work_task')),
 input_digest TEXT NOT NULL,payload_cipher BLOB NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('prepared','awaiting_answer','answered','applied','rejected','cancelled','outcome_unknown')),
 revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0),result_cipher BLOB,
 result_kind TEXT CHECK(result_kind IN ('question','automation','mission','error')),
 created_at TEXT NOT NULL,expires_at_ms INTEGER NOT NULL,updated_at TEXT NOT NULL,
 UNIQUE(agent_run_id,execution_epoch,tool_name,id),
 FOREIGN KEY(agent_run_id,execution_epoch) REFERENCES conversation_tool_run(agent_run_id,execution_epoch)
);
CREATE UNIQUE INDEX conversation_tool_pending_question ON conversation_tool_request(agent_run_id,execution_epoch)
 WHERE tool_name='ask_user_question' AND state IN ('prepared','awaiting_answer','answered','outcome_unknown');
CREATE UNIQUE INDEX conversation_tool_unresolved_create ON conversation_tool_request(agent_run_id,execution_epoch,tool_name)
 WHERE tool_name IN ('create_automation','create_work_task') AND state IN ('prepared','outcome_unknown');
CREATE TABLE conversation_tool_question (
 request_id TEXT PRIMARY KEY NOT NULL REFERENCES conversation_tool_request(id),questions_cipher BLOB NOT NULL,
 target_cipher BLOB NOT NULL,target_digest TEXT NOT NULL,created_at TEXT NOT NULL
);
CREATE TABLE conversation_tool_answer (
 request_id TEXT PRIMARY KEY NOT NULL REFERENCES conversation_tool_request(id),command_id TEXT NOT NULL UNIQUE,
 client_id TEXT NOT NULL,input_digest TEXT NOT NULL,value_cipher BLOB NOT NULL,created_at TEXT NOT NULL
);
CREATE TABLE conversation_tool_delivery (
 request_id TEXT PRIMARY KEY NOT NULL REFERENCES conversation_tool_request(id),
 agent_run_id TEXT NOT NULL,execution_epoch INTEGER NOT NULL CHECK(execution_epoch>=0),
 bridge_instance_id TEXT NOT NULL,native_binding_id TEXT NOT NULL,native_binding_generation INTEGER NOT NULL CHECK(native_binding_generation>=0),
 native_locator_cipher BLOB NOT NULL,result_digest TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('pending','claimed','delivered','cancelled','outcome_unknown')),
 receipt_digest TEXT,created_at TEXT NOT NULL,updated_at TEXT NOT NULL,
 FOREIGN KEY(agent_run_id,execution_epoch) REFERENCES conversation_tool_run(agent_run_id,execution_epoch)
);
CREATE TABLE conversation_tool_automation_owner (
 automation_id TEXT PRIMARY KEY NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),
 action TEXT NOT NULL CHECK(action IN ('launch_session','enqueue_task')),timezone TEXT NOT NULL,
 source_request_id TEXT NOT NULL UNIQUE REFERENCES conversation_tool_request(id),created_at TEXT NOT NULL,updated_at TEXT NOT NULL
);
CREATE TABLE conversation_tool_mission (
 mission_id TEXT PRIMARY KEY NOT NULL REFERENCES mission(id) ON DELETE CASCADE,
 source_request_id TEXT NOT NULL REFERENCES conversation_tool_request(id),
 source_automation_id TEXT,created_at TEXT NOT NULL
);
CREATE TABLE conversation_tool_automation_fire (
 automation_id TEXT NOT NULL REFERENCES conversation_tool_automation_owner(automation_id),scheduled_at_utc TEXT NOT NULL,
 command_id TEXT NOT NULL UNIQUE,state TEXT NOT NULL CHECK(state IN ('prepared','applied','cancelled','outcome_unknown')),
 target_id TEXT,created_at TEXT NOT NULL,PRIMARY KEY(automation_id,scheduled_at_utc)
);
