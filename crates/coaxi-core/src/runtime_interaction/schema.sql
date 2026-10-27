CREATE TABLE runtime_user_interaction (
    id TEXT PRIMARY KEY NOT NULL,
    camp_id TEXT NOT NULL REFERENCES camp(id) ON DELETE CASCADE,
    conversation_id TEXT NOT NULL REFERENCES conversation(id) ON DELETE CASCADE,
    agent_run_id TEXT NOT NULL REFERENCES agent_run(id) ON DELETE CASCADE,
    execution_epoch INTEGER NOT NULL CHECK(execution_epoch>=1),
    input_delivery_id TEXT NOT NULL REFERENCES runtime_input_delivery(id) ON DELETE CASCADE,
    core_generation TEXT NOT NULL CHECK(length(core_generation)>0),
    host_instance_id TEXT NOT NULL CHECK(length(host_instance_id)>0),
    native_sequence INTEGER NOT NULL CHECK(native_sequence>=0),
    native_request_id_json TEXT NOT NULL CHECK(json_valid(native_request_id_json) AND json_type(native_request_id_json) IN ('text','integer')),
    native_method TEXT NOT NULL CHECK(native_method IN ('cursor/ask_question','cursor/create_plan','_x.ai/ask_user_question','_x.ai/exit_plan_mode')),
    native_session_id TEXT NOT NULL CHECK(length(native_session_id)>0),
    native_binding_id TEXT NOT NULL CHECK(length(native_binding_id)>0),
    native_binding_generation INTEGER NOT NULL CHECK(native_binding_generation>=1),
    request_json TEXT NOT NULL CHECK(json_valid(request_json) AND COALESCE(json_extract(request_json,'$.schemaVersion')=1,0)),
    request_digest TEXT NOT NULL CHECK(length(request_digest)=64),
    kind TEXT NOT NULL CHECK(kind IN ('question','plan_approval')),
    status TEXT NOT NULL CHECK(status IN ('pending','resolved','dispatching','sent','closed','delivery_unknown')),
    version INTEGER NOT NULL CHECK(version>=1),
    answer_json TEXT CHECK(answer_json IS NULL OR json_valid(answer_json)),
    response_json TEXT CHECK(response_json IS NULL OR json_valid(response_json)),
    followup_feedback TEXT,
    followup_command_id TEXT NOT NULL UNIQUE,
    followup_delivery_id TEXT,
    resolved_command_id TEXT,
    resolved_at TEXT,
    dispatch_token TEXT UNIQUE,
    dispatch_started_at TEXT,
    finished_at TEXT,
    close_reason TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(core_generation,host_instance_id,agent_run_id,execution_epoch,native_sequence),
    CHECK(COALESCE(json_extract(request_json,'$.body.kind')=kind,0)),
    CHECK(COALESCE((native_method LIKE 'cursor/%' AND json_extract(request_json,'$.protocol')='cursor_acp')
       OR (native_method LIKE '_x.ai/%' AND json_extract(request_json,'$.protocol')='grok_acp'),0)),
    CHECK((native_method IN ('cursor/ask_question','_x.ai/ask_user_question') AND kind='question')
       OR (native_method IN ('cursor/create_plan','_x.ai/exit_plan_mode') AND kind='plan_approval')),
    CHECK((answer_json IS NULL AND response_json IS NULL AND resolved_command_id IS NULL AND resolved_at IS NULL)
       OR (answer_json IS NOT NULL AND response_json IS NOT NULL AND resolved_command_id IS NOT NULL AND resolved_at IS NOT NULL)),
    CHECK(status NOT IN ('resolved','dispatching','sent','delivery_unknown') OR answer_json IS NOT NULL),
    CHECK(status<>'pending' OR (answer_json IS NULL AND dispatch_token IS NULL)),
    CHECK(answer_json IS NOT NULL OR followup_feedback IS NULL),
    CHECK(status NOT IN ('pending','resolved','closed') OR dispatch_token IS NULL),
    CHECK(status NOT IN ('dispatching','sent','delivery_unknown') OR (dispatch_token IS NOT NULL AND dispatch_started_at IS NOT NULL)),
    CHECK(status NOT IN ('sent','closed','delivery_unknown') OR finished_at IS NOT NULL)
);
CREATE UNIQUE INDEX runtime_user_interaction_active_rpc ON runtime_user_interaction(core_generation,host_instance_id,native_request_id_json)
    WHERE status IN ('pending','resolved','dispatching');
CREATE INDEX runtime_user_interaction_conversation ON runtime_user_interaction(conversation_id,created_at,id);
CREATE INDEX runtime_user_interaction_run ON runtime_user_interaction(agent_run_id,execution_epoch,status);
CREATE TRIGGER runtime_user_interaction_scope_insert BEFORE INSERT ON runtime_user_interaction
WHEN NEW.status<>'pending' OR NEW.version<>1 OR NEW.answer_json IS NOT NULL OR NEW.dispatch_token IS NOT NULL
 OR NEW.followup_delivery_id IS NOT NULL OR NOT EXISTS (
    SELECT 1 FROM agent_run r JOIN conversation c ON c.id=r.conversation_id
    JOIN runtime_input_delivery input ON input.id=NEW.input_delivery_id AND input.agent_run_id=r.id AND input.execution_epoch=r.execution_epoch
    JOIN camp ON camp.id=c.camp_id
    WHERE r.id=NEW.agent_run_id AND r.execution_epoch=NEW.execution_epoch
      AND camp.deletion_operation_id IS NULL AND camp.activation_state='active'
      AND c.id=NEW.conversation_id AND c.camp_id=NEW.camp_id AND c.ended_at IS NULL
      AND r.status IN ('running','waiting') AND r.cancel_requested_at IS NULL AND r.runtime_recovery_required=0
      AND c.native_session_id=NEW.native_session_id AND c.native_binding_id=NEW.native_binding_id
      AND c.native_binding_generation=NEW.native_binding_generation
      AND input.native_binding_id=NEW.native_binding_id AND input.native_binding_generation=NEW.native_binding_generation
      AND (input.status='accepted' OR (input.status='prepared' AND input.dispatch_started_at IS NOT NULL))
      AND ((r.runtime_adapter_kind='cursor-agent' AND NEW.native_method LIKE 'cursor/%')
        OR (r.runtime_adapter_kind='grok-build' AND NEW.native_method LIKE '_x.ai/%'))
)
BEGIN SELECT RAISE(ABORT,'Native interaction scope is not live'); END;
CREATE TRIGGER runtime_user_interaction_immutable BEFORE UPDATE ON runtime_user_interaction
WHEN NEW.id IS NOT OLD.id OR NEW.camp_id IS NOT OLD.camp_id OR NEW.conversation_id IS NOT OLD.conversation_id
 OR NEW.agent_run_id IS NOT OLD.agent_run_id OR NEW.execution_epoch IS NOT OLD.execution_epoch
 OR NEW.input_delivery_id IS NOT OLD.input_delivery_id OR NEW.core_generation IS NOT OLD.core_generation
 OR NEW.host_instance_id IS NOT OLD.host_instance_id OR NEW.native_sequence IS NOT OLD.native_sequence
 OR NEW.native_request_id_json IS NOT OLD.native_request_id_json OR NEW.native_method IS NOT OLD.native_method
 OR NEW.native_session_id IS NOT OLD.native_session_id OR NEW.native_binding_id IS NOT OLD.native_binding_id
 OR NEW.native_binding_generation IS NOT OLD.native_binding_generation
 OR NEW.request_json IS NOT OLD.request_json OR NEW.request_digest IS NOT OLD.request_digest OR NEW.kind IS NOT OLD.kind
 OR NEW.followup_command_id IS NOT OLD.followup_command_id OR NEW.created_at IS NOT OLD.created_at
 OR (OLD.answer_json IS NOT NULL AND (NEW.answer_json IS NOT OLD.answer_json OR NEW.response_json IS NOT OLD.response_json
     OR NEW.resolved_command_id IS NOT OLD.resolved_command_id OR NEW.resolved_at IS NOT OLD.resolved_at
     OR NEW.followup_feedback IS NOT OLD.followup_feedback))
 OR (OLD.dispatch_token IS NOT NULL AND (NEW.dispatch_token IS NOT OLD.dispatch_token OR NEW.dispatch_started_at IS NOT OLD.dispatch_started_at))
 OR (OLD.followup_delivery_id IS NOT NULL AND NEW.followup_delivery_id IS NOT OLD.followup_delivery_id)
BEGIN SELECT RAISE(ABORT,'Native interaction identity and decision are frozen'); END;
CREATE TRIGGER runtime_user_interaction_state_update BEFORE UPDATE ON runtime_user_interaction
WHEN NEW.version<>OLD.version+1
 OR (OLD.answer_json IS NULL AND NEW.answer_json IS NOT NULL AND NEW.status<>'resolved')
 OR NOT (
    (OLD.status='pending' AND NEW.status IN ('resolved','closed'))
 OR (OLD.status='resolved' AND NEW.status IN ('resolved','dispatching','closed'))
 OR (OLD.status='dispatching' AND NEW.status IN ('sent','delivery_unknown'))
)
BEGIN SELECT RAISE(ABORT,'Native interaction state cannot be replayed or reopened'); END;
CREATE TRIGGER runtime_user_interaction_run_closed AFTER UPDATE OF status,execution_epoch,cancel_requested_at,runtime_recovery_required ON agent_run
WHEN NEW.status NOT IN ('running','waiting') OR NEW.execution_epoch<>OLD.execution_epoch
 OR NEW.cancel_requested_at IS NOT NULL OR NEW.runtime_recovery_required=1
BEGIN
    UPDATE runtime_user_interaction SET status=CASE WHEN status='dispatching' THEN 'delivery_unknown' ELSE 'closed' END,
        close_reason='run_no_longer_live',finished_at=NEW.updated_at,updated_at=NEW.updated_at,version=version+1
    WHERE agent_run_id=NEW.id AND status IN ('pending','resolved','dispatching');
END;
CREATE TRIGGER runtime_user_interaction_binding_closed AFTER UPDATE OF ended_at,native_session_id,native_binding_id,native_binding_generation ON conversation
WHEN NEW.ended_at IS NOT NULL OR NEW.native_session_id IS NOT OLD.native_session_id
 OR NEW.native_binding_id IS NOT OLD.native_binding_id OR NEW.native_binding_generation IS NOT OLD.native_binding_generation
BEGIN
    UPDATE runtime_user_interaction SET status=CASE WHEN status='dispatching' THEN 'delivery_unknown' ELSE 'closed' END,
        close_reason='native_binding_closed',finished_at=NEW.updated_at,updated_at=NEW.updated_at,version=version+1
    WHERE conversation_id=NEW.id AND status IN ('pending','resolved','dispatching');
END;
