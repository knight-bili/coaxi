CREATE TABLE ui_feedback_policy (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    revision INTEGER NOT NULL CHECK(revision>=0),
    value_json TEXT NOT NULL CHECK(json_valid(value_json))
);
INSERT INTO ui_feedback_policy VALUES(1,0,'{"schemaVersion":1,"revision":0,"members":[]}');
CREATE TABLE ui_feedback_preview (
    id TEXT PRIMARY KEY,
    request_json TEXT NOT NULL CHECK(json_valid(request_json)),
    result_json TEXT NOT NULL CHECK(json_valid(result_json)),
    binding_guard_json TEXT NOT NULL CHECK(json_valid(binding_guard_json)),
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX ui_feedback_preview_expiry ON ui_feedback_preview(expires_at);
CREATE TABLE ui_feedback_entry (
    id TEXT PRIMARY KEY,
    preview_id TEXT NOT NULL UNIQUE,
    thread_id TEXT NOT NULL,
    conversation_id TEXT,
    scope_kind TEXT NOT NULL CHECK(scope_kind IN ('public','single_chat','delegation_child')),
    agent_id TEXT NOT NULL,
    agent_run_id TEXT NOT NULL,
    execution_epoch INTEGER NOT NULL CHECK(execution_epoch>=0),
    text TEXT NOT NULL CHECK(length(text) BETWEEN 1 AND 4096),
    created_at TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','response_prepared','connection_written','unknown','requires_user_review','continued_as_message')),
    receipt_json TEXT NOT NULL CHECK(json_valid(receipt_json)),
    source_json TEXT NOT NULL CHECK(json_valid(source_json))
);
CREATE INDEX ui_feedback_pending ON ui_feedback_entry(agent_run_id,execution_epoch,status,created_at,id);
CREATE INDEX ui_feedback_scope ON ui_feedback_entry(thread_id,scope_kind,conversation_id,agent_id,created_at,id);
CREATE TABLE ui_feedback_command (
    command_id TEXT PRIMARY KEY,
    input_digest TEXT NOT NULL,
    feedback_id TEXT NOT NULL REFERENCES ui_feedback_entry(id),
    created_at TEXT NOT NULL
);
CREATE TABLE ui_feedback_attempt (
    id TEXT PRIMARY KEY,
    bridge_instance_id TEXT NOT NULL,
    binding_json TEXT NOT NULL CHECK(json_valid(binding_json)),
    binding_digest TEXT NOT NULL,
    rpc_id_json TEXT NOT NULL CHECK(json_valid(rpc_id_json)),
    arguments_digest TEXT NOT NULL,
    feedback_ids_json TEXT NOT NULL CHECK(json_valid(feedback_ids_json)),
    result_json TEXT NOT NULL CHECK(json_valid(result_json)),
    result_digest TEXT NOT NULL,
    write_status TEXT NOT NULL CHECK(write_status IN ('prepared','written','unknown','revoked')),
    created_at TEXT NOT NULL,
    written_at TEXT,
    UNIQUE(bridge_instance_id,rpc_id_json)
);
CREATE INDEX ui_feedback_attempt_run ON ui_feedback_attempt(bridge_instance_id,write_status);
CREATE TABLE ui_feedback_continuation (
    feedback_id TEXT PRIMARY KEY REFERENCES ui_feedback_entry(id),
    command_id TEXT NOT NULL UNIQUE,
    preview_id TEXT NOT NULL,
    pending_input_id TEXT,
    message_id TEXT,
    result_json TEXT NOT NULL CHECK(json_valid(result_json)),
    created_at TEXT NOT NULL
);
CREATE TRIGGER ui_feedback_pending_published AFTER UPDATE OF published_conversation_message_id ON single_chat_pending_input
WHEN NEW.published_conversation_message_id IS NOT NULL
BEGIN
    UPDATE ui_feedback_entry SET receipt_json=json_set(receipt_json,'$.continuedMessageId',NEW.published_conversation_message_id)
    WHERE id IN (SELECT feedback_id FROM ui_feedback_continuation WHERE pending_input_id=NEW.id) AND status='continued_as_message';
    UPDATE ui_feedback_continuation SET message_id=NEW.published_conversation_message_id WHERE pending_input_id=NEW.id;
END;
CREATE TRIGGER ui_feedback_run_closed AFTER UPDATE OF status,execution_epoch,cancel_requested_at ON agent_run
WHEN NEW.status!='running' OR NEW.cancel_requested_at IS NOT NULL OR NEW.execution_epoch!=OLD.execution_epoch
BEGIN
    UPDATE ui_feedback_entry SET
        receipt_json=json_set(receipt_json,'$.status',CASE WHEN status='pending' THEN 'requires_user_review' ELSE 'unknown' END),
        status=CASE WHEN status='pending' THEN 'requires_user_review' ELSE 'unknown' END
    WHERE agent_run_id=NEW.id AND status IN ('pending','response_prepared');
END;
