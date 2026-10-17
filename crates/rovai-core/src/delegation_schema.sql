-- Identity graph only. Installed with the complete Schema160/Migration210, never by a query.
CREATE TABLE delegation (
    id TEXT PRIMARY KEY CHECK(length(id)>0),
    camp_id TEXT NOT NULL REFERENCES camp(id),
    owner_agent_id TEXT NOT NULL REFERENCES agent_profile(id),
    root_conversation_id TEXT NOT NULL REFERENCES conversation(id),
    parent_conversation_id TEXT NOT NULL REFERENCES conversation(id),
    parent_delegation_id TEXT REFERENCES delegation(id),
    child_conversation_id TEXT NOT NULL UNIQUE REFERENCES conversation(id),
    origin_parent_run_id TEXT NOT NULL REFERENCES agent_run(id),
    origin_parent_epoch INTEGER NOT NULL CHECK(typeof(origin_parent_epoch)='integer' AND origin_parent_epoch>=1),
    origin_binding_id TEXT NOT NULL CHECK(length(origin_binding_id)>0),
    origin_binding_generation INTEGER NOT NULL CHECK(typeof(origin_binding_generation)='integer' AND origin_binding_generation>=1),
    origin_tool_call_id TEXT NOT NULL CHECK(length(origin_tool_call_id)>0),
    depth INTEGER NOT NULL CHECK(typeof(depth)='integer' AND depth BETWEEN 1 AND 8),
    task_blob_id TEXT NOT NULL REFERENCES managed_blob(id),
    requested_runtime TEXT NOT NULL CHECK(requested_runtime IN (
        'antigravity','claude_code','cline','code_buddy','codex','command_code','copilot',
        'cursor','deepseek','gemini','grok','hermes','kimi_code','kiro','open_claw',
        'open_code','pi','qoder','qwen_code','trae_cn','zcode'
    ) OR (substr(requested_runtime,1,7)='custom:' AND length(requested_runtime)>7
          AND substr(requested_runtime,8) NOT GLOB '*[^A-Za-z0-9._-]*')),
    requested_working_dir TEXT CHECK(requested_working_dir IS NULL OR length(requested_working_dir)>0),
    resolved_workspace_json TEXT NOT NULL CHECK(json_valid(resolved_workspace_json) AND json_type(resolved_workspace_json)='object'),
    runtime_preset_json TEXT NOT NULL CHECK(json_valid(runtime_preset_json) AND json_type(runtime_preset_json)='object'),
    created_at TEXT NOT NULL,
    UNIQUE(origin_binding_id,origin_binding_generation,origin_parent_run_id,origin_parent_epoch,origin_tool_call_id),
    CHECK(child_conversation_id<>parent_conversation_id AND child_conversation_id<>root_conversation_id),
    CHECK(parent_delegation_id IS NULL OR parent_delegation_id<>id),
    FOREIGN KEY(id) REFERENCES delegation_control(delegation_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX delegation_parent_idx ON delegation(parent_conversation_id,created_at,id);
CREATE INDEX delegation_root_idx ON delegation(root_conversation_id,id);
CREATE INDEX delegation_parent_delegation_idx ON delegation(parent_delegation_id,id);
CREATE INDEX delegation_task_blob_idx ON delegation(task_blob_id);

CREATE TABLE delegation_attempt (
    delegation_id TEXT NOT NULL REFERENCES delegation(id) ON DELETE CASCADE,
    attempt_no INTEGER NOT NULL CHECK(typeof(attempt_no)='integer' AND attempt_no>=1),
    agent_run_id TEXT NOT NULL UNIQUE REFERENCES agent_run(id),
    trigger_parent_run_id TEXT NOT NULL REFERENCES agent_run(id),
    trigger_parent_epoch INTEGER NOT NULL CHECK(typeof(trigger_parent_epoch)='integer' AND trigger_parent_epoch>=1),
    trigger_tool_call_id TEXT NOT NULL CHECK(length(trigger_tool_call_id)>0),
    kind TEXT NOT NULL CHECK(kind IN ('initial','resume')),
    interruption_reason TEXT CHECK(interruption_reason IS NULL OR length(CAST(interruption_reason AS BLOB)) BETWEEN 1 AND 2048),
    created_at TEXT NOT NULL,
    PRIMARY KEY(delegation_id,attempt_no),
    UNIQUE(trigger_parent_run_id,trigger_parent_epoch,trigger_tool_call_id,kind),
    CHECK((attempt_no=1 AND kind='initial' AND interruption_reason IS NULL) OR (attempt_no>1 AND kind='resume'))
);

CREATE TABLE delegation_control (
    delegation_id TEXT PRIMARY KEY REFERENCES delegation(id) ON DELETE CASCADE,
    current_attempt_no INTEGER NOT NULL CHECK(typeof(current_attempt_no)='integer' AND current_attempt_no>=1),
    control_version INTEGER NOT NULL CHECK(typeof(control_version)='integer' AND control_version>=1),
    takeover_at TEXT,
    takeover_command_id TEXT CHECK(takeover_command_id IS NULL OR length(takeover_command_id)>0),
    takeover_message_sequence INTEGER CHECK(takeover_message_sequence IS NULL OR (typeof(takeover_message_sequence)='integer' AND takeover_message_sequence>=0)),
    CHECK((takeover_at IS NULL AND takeover_command_id IS NULL AND takeover_message_sequence IS NULL)
       OR (takeover_at IS NOT NULL AND takeover_command_id IS NOT NULL AND takeover_message_sequence IS NOT NULL)),
    FOREIGN KEY(delegation_id,current_attempt_no) REFERENCES delegation_attempt(delegation_id,attempt_no) DEFERRABLE INITIALLY DEFERRED
);

CREATE TRIGGER delegation_owner_insert BEFORE INSERT ON delegation BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM conversation parent
        JOIN conversation child ON child.id=NEW.child_conversation_id
        JOIN conversation root ON root.id=NEW.root_conversation_id
        JOIN camp_member member ON member.camp_id=parent.camp_id AND member.agent_id=parent.agent_id
        JOIN agent_run run ON run.id=NEW.origin_parent_run_id AND run.conversation_id=parent.id
        LEFT JOIN camp_turn parent_turn ON parent_turn.id=run.camp_turn_id
        JOIN delegation_settings settings ON settings.singleton=1
        WHERE parent.id=NEW.parent_conversation_id
          AND parent.camp_id=NEW.camp_id AND parent.agent_id=NEW.owner_agent_id
          AND parent.ended_at IS NULL AND member.status='active'
          AND child.camp_id=parent.camp_id AND child.agent_id=parent.agent_id
          AND child.kind='delegation_child' AND child.ended_at IS NULL
          AND root.camp_id=parent.camp_id AND root.agent_id=parent.agent_id
          AND root.kind IN ('camp_member','single_chat')
          AND run.status IN ('running','waiting') AND run.cancel_requested_at IS NULL
          AND run.execution_epoch=NEW.origin_parent_epoch
          AND COALESCE(run.camp_id,parent_turn.camp_id)=parent.camp_id
          AND parent.native_binding_id=NEW.origin_binding_id
          AND parent.native_binding_generation=NEW.origin_binding_generation
          AND settings.enabled=1 AND NEW.depth<=settings.depth_limit
          AND ((NEW.parent_delegation_id IS NULL AND NEW.depth=1 AND parent.id=root.id
                AND parent.kind IN ('camp_member','single_chat'))
               OR (NEW.parent_delegation_id IS NOT NULL AND parent.kind='delegation_child'
                   AND EXISTS(SELECT 1 FROM delegation ancestor
                       WHERE ancestor.id=NEW.parent_delegation_id
                         AND ancestor.child_conversation_id=parent.id
                         AND ancestor.root_conversation_id=root.id
                         AND ancestor.camp_id=NEW.camp_id AND ancestor.owner_agent_id=NEW.owner_agent_id
                         AND NEW.depth=ancestor.depth+1)))
    ) THEN RAISE(ABORT,'delegation parent, binding, depth or owner mismatch') END;
END;
CREATE TRIGGER delegation_identity_immutable BEFORE UPDATE ON delegation BEGIN
    SELECT RAISE(ABORT,'delegation identity is immutable');
END;

CREATE TRIGGER delegation_attempt_insert BEFORE INSERT ON delegation_attempt BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM delegation relation
        JOIN conversation child ON child.id=relation.child_conversation_id
        JOIN conversation parent ON parent.id=relation.parent_conversation_id
        JOIN camp_member member ON member.camp_id=relation.camp_id AND member.agent_id=relation.owner_agent_id
        JOIN agent_run child_run ON child_run.id=NEW.agent_run_id
        JOIN camp_turn turn ON turn.id=child_run.camp_turn_id
        JOIN agent_run parent_run ON parent_run.id=NEW.trigger_parent_run_id
        LEFT JOIN camp_turn parent_turn ON parent_turn.id=parent_run.camp_turn_id
        WHERE relation.id=NEW.delegation_id AND member.status='active'
          AND parent.ended_at IS NULL AND child.ended_at IS NULL
          AND parent_run.conversation_id=parent.id AND parent_run.execution_epoch=NEW.trigger_parent_epoch
          AND ((parent_run.status IN ('running','waiting') AND parent_run.cancel_requested_at IS NULL)
            OR (NEW.kind='resume' AND EXISTS(
                SELECT 1 FROM delegation_attempt basis JOIN event_log proof
                  ON proof.event_type='delegation.resumed' AND proof.actor_type='user'
                 AND proof.source_agent_run_id IS NULL AND proof.execution_epoch IS NULL
                 AND proof.camp_id=relation.camp_id AND proof.entity_type='delegation' AND proof.entity_id=relation.id
                 AND json_extract(proof.payload_json,'$.delegationId')=relation.id
                 AND json_type(proof.payload_json,'$.attemptNo')='integer'
                 AND json_extract(proof.payload_json,'$.attemptNo')=NEW.attempt_no
                 AND json_extract(proof.payload_json,'$.agentRunId')=child_run.id
                 AND NEW.trigger_tool_call_id='user-command:'||json_extract(proof.payload_json,'$.commandId')
                WHERE basis.delegation_id=relation.id AND basis.attempt_no=NEW.attempt_no-1
                  AND basis.trigger_parent_run_id=NEW.trigger_parent_run_id AND basis.trigger_parent_epoch=NEW.trigger_parent_epoch)))
          AND COALESCE(parent_run.camp_id,parent_turn.camp_id)=relation.camp_id
          AND child_run.conversation_id=child.id AND child_run.task_id IS NULL
          AND child_run.invocation_kind='delegation' AND child_run.operation_policy='delegation_child_v1'
          AND child_run.operation_policy_version=1 AND child_run.response_delivery='conversation_message'
          AND child_run.destination_conversation_id=child.id
          AND child_run.status='queued' AND child_run.execution_epoch=0
          AND turn.camp_id=relation.camp_id AND turn.kind='delegation'
          AND NEW.attempt_no=COALESCE((SELECT max(a.attempt_no)+1 FROM delegation_attempt a WHERE a.delegation_id=relation.id),1)
          AND ((NEW.kind='initial' AND NEW.trigger_parent_run_id=relation.origin_parent_run_id
                AND NEW.trigger_parent_epoch=relation.origin_parent_epoch
                AND NEW.trigger_tool_call_id=relation.origin_tool_call_id)
               OR (NEW.kind='resume' AND child.native_session_id IS NOT NULL
                   AND EXISTS(SELECT 1 FROM delegation_control control
                       JOIN delegation_attempt previous ON previous.delegation_id=control.delegation_id AND previous.attempt_no=NEW.attempt_no-1
                       JOIN agent_run old_run ON old_run.id=previous.agent_run_id
                       WHERE control.delegation_id=relation.id AND control.takeover_at IS NULL
                         AND control.current_attempt_no=NEW.attempt_no
                         AND old_run.status='cancelled' AND old_run.cancel_acknowledged_at IS NOT NULL
                         AND old_run.runtime_recovery_required=0
                         AND EXISTS(SELECT 1 FROM runtime_input_delivery input
                             WHERE input.agent_run_id=old_run.id AND input.execution_epoch=old_run.execution_epoch AND input.status='accepted'))))
    ) THEN RAISE(ABORT,'delegation attempt scope or lifecycle mismatch') END;
END;
CREATE TRIGGER delegation_attempt_immutable BEFORE UPDATE ON delegation_attempt BEGIN
    SELECT RAISE(ABORT,'delegation attempt is immutable');
END;

CREATE TRIGGER delegation_control_insert BEFORE INSERT ON delegation_control
WHEN NEW.control_version<>1 OR NEW.current_attempt_no<>1 OR NEW.takeover_at IS NOT NULL
BEGIN SELECT RAISE(ABORT,'delegation control must start at the initial attempt'); END;
CREATE TRIGGER delegation_control_update BEFORE UPDATE ON delegation_control BEGIN
    SELECT CASE WHEN NEW.delegation_id IS NOT OLD.delegation_id
        OR NEW.control_version<>OLD.control_version+1 OR OLD.takeover_at IS NOT NULL
        OR NEW.current_attempt_no NOT IN (OLD.current_attempt_no,OLD.current_attempt_no+1)
        THEN RAISE(ABORT,'delegation control version or takeover fence') END;
    SELECT CASE WHEN NEW.takeover_at IS NOT NULL AND (
        NEW.current_attempt_no<>OLD.current_attempt_no
        OR NEW.takeover_message_sequence IS NOT (SELECT c.last_message_sequence FROM delegation d JOIN conversation c ON c.id=d.child_conversation_id WHERE d.id=NEW.delegation_id)
        OR NOT EXISTS(SELECT 1 FROM delegation_attempt a JOIN agent_run r ON r.id=a.agent_run_id
            WHERE a.delegation_id=NEW.delegation_id AND a.attempt_no=NEW.current_attempt_no AND r.status IN ('succeeded','failed','cancelled'))
    ) THEN RAISE(ABORT,'delegation takeover boundary or terminal fence') END;
END;

CREATE UNIQUE INDEX agent_run_active_delegation_conversation_unique ON agent_run(conversation_id)
    WHERE invocation_kind IN ('delegation','delegation_user') AND status IN ('queued','running','waiting');
CREATE TRIGGER delegation_manifest_route_insert BEFORE INSERT ON context_manifest
WHEN NEW.context_manifest_version=35 BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM agent_run r JOIN conversation c ON c.id=r.conversation_id
        JOIN native_session_bootstrap_evidence b ON b.id=NEW.bootstrap_evidence_id
          AND b.conversation_id=c.id AND b.native_binding_generation=NEW.native_binding_generation
        WHERE r.id=NEW.agent_run_id
          AND NEW.formatter_version=35 AND json_extract(r.skill_selection_snapshot_json,'$.schemaVersion')=3
          AND json_extract(r.effective_config_json,'$.delegationPolicy.schemaVersion')=1
          AND json_type(r.effective_config_json,'$.delegationPolicy.enabled') IN ('true','false')
          AND json_extract(r.effective_config_json,'$.delegationPolicy.depthLimit') BETWEEN 1 AND 8
          AND ((NEW.context_delivery_profile_version=12 AND NEW.run_facts_schema_version=9
                AND r.invocation_kind IN ('batch','direct','a2a','gather_completion')
                AND c.kind='camp_member'
                AND json_extract(r.effective_config_json,'$.delegationPolicy.role')='public_parent'
                AND (r.invocation_kind<>'batch' OR (
                    r.claim_previous_public_boundary_sequence IS NOT NULL
                    AND r.claim_has_additional_public_messages IS NOT NULL
                    AND EXISTS(SELECT 1 FROM agent_run_input i WHERE i.agent_run_id=r.id AND i.context_manifest_version=35)
                    AND NOT EXISTS(SELECT 1 FROM agent_run_input i WHERE i.agent_run_id=r.id AND i.context_manifest_version IS NOT 35))))
            OR (NEW.context_delivery_profile_version=13 AND NEW.run_facts_schema_version=6
                AND r.invocation_kind='single_chat' AND c.kind='single_chat'
                AND r.operation_policy='single_chat_v1' AND r.operation_policy_version=3
                AND json_extract(r.effective_config_json,'$.delegationPolicy.role')='private_parent')
            OR (NEW.context_delivery_profile_version=11 AND NEW.run_facts_schema_version=10
                AND r.invocation_kind IN ('delegation','delegation_user') AND c.kind='delegation_child'
                AND r.operation_policy='delegation_child_v1' AND r.operation_policy_version=1
                AND json_extract(r.effective_config_json,'$.delegationPolicy.role')='child'
                AND NEW.collaboration_state_included=0 AND NEW.workspace_fact_included=0
                AND NEW.recent_message_refs_json='[]' AND NEW.reference_closure_refs_json='[]'
                AND NEW.shared_message_evidence_json='[]' AND NEW.originating_public_user_message_ref_json IS NULL
                AND NEW.global_public_message_boundary=0 AND NEW.mission_details_version IS NULL))
    ) THEN RAISE(ABORT,'delegation Context format or private scope mismatch') END;
END;
CREATE TRIGGER delegation_run_route_insert BEFORE INSERT ON agent_run
WHEN NEW.invocation_kind IN ('delegation','delegation_user')
  OR EXISTS(SELECT 1 FROM conversation WHERE id=NEW.conversation_id AND kind='delegation_child') BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM conversation c JOIN camp_turn t ON t.id=NEW.camp_turn_id
        JOIN camp_member member ON member.camp_id=c.camp_id AND member.agent_id=c.agent_id
        WHERE c.id=NEW.conversation_id AND c.kind='delegation_child' AND c.ended_at IS NULL
          AND t.camp_id=c.camp_id AND t.kind='delegation' AND member.status='active'
          AND t.trigger_type='conversation_message' AND t.trigger_id=NEW.trigger_conversation_message_id
          AND NEW.invocation_kind IN ('delegation','delegation_user')
          AND NEW.operation_policy='delegation_child_v1' AND NEW.operation_policy_version=1
          AND NEW.task_id IS NULL AND NEW.response_delivery='conversation_message'
          AND NEW.destination_conversation_id=c.id
          AND NEW.trigger_conversation_message_id IS NOT NULL
          AND EXISTS(SELECT 1 FROM conversation_message message WHERE message.id=NEW.trigger_conversation_message_id AND message.conversation_id=c.id)
          AND (NEW.invocation_kind='delegation' OR EXISTS(SELECT 1 FROM delegation d JOIN delegation_control control ON control.delegation_id=d.id WHERE d.child_conversation_id=c.id AND control.takeover_at IS NOT NULL))
    ) THEN RAISE(ABORT,'delegation Run must target its private child') END;
END;
CREATE TRIGGER delegation_run_route_update BEFORE UPDATE OF conversation_id,camp_turn_id,task_id,invocation_kind,response_delivery,operation_policy,operation_policy_version,destination_conversation_id ON agent_run
WHEN (OLD.invocation_kind IN ('delegation','delegation_user') OR NEW.invocation_kind IN ('delegation','delegation_user')
      OR EXISTS(SELECT 1 FROM conversation WHERE id IN (OLD.conversation_id,NEW.conversation_id) AND kind='delegation_child'))
  AND (NEW.conversation_id IS NOT OLD.conversation_id OR NEW.camp_turn_id IS NOT OLD.camp_turn_id
       OR NEW.task_id IS NOT OLD.task_id OR NEW.invocation_kind IS NOT OLD.invocation_kind
       OR NEW.response_delivery IS NOT OLD.response_delivery OR NEW.operation_policy IS NOT OLD.operation_policy
       OR NEW.operation_policy_version IS NOT OLD.operation_policy_version OR NEW.destination_conversation_id IS NOT OLD.destination_conversation_id)
BEGIN SELECT RAISE(ABORT,'delegation Run route is immutable'); END;

CREATE TRIGGER delegation_conversation_identity_update BEFORE UPDATE OF id,camp_id,agent_id,kind ON conversation
WHEN (OLD.kind='delegation_child' OR NEW.kind='delegation_child'
      OR EXISTS(SELECT 1 FROM delegation d WHERE OLD.id IN (d.root_conversation_id,d.parent_conversation_id,d.child_conversation_id)))
  AND (NEW.id IS NOT OLD.id OR NEW.camp_id IS NOT OLD.camp_id OR NEW.agent_id IS NOT OLD.agent_id OR NEW.kind IS NOT OLD.kind)
BEGIN SELECT RAISE(ABORT,'delegation Conversation identity is immutable'); END;
CREATE TRIGGER delegation_parent_run_identity_update BEFORE UPDATE OF conversation_id,camp_turn_id,camp_id ON agent_run
WHEN (NEW.conversation_id IS NOT OLD.conversation_id OR NEW.camp_turn_id IS NOT OLD.camp_turn_id OR NEW.camp_id IS NOT OLD.camp_id)
  AND (EXISTS(SELECT 1 FROM delegation d WHERE d.origin_parent_run_id=OLD.id)
       OR EXISTS(SELECT 1 FROM delegation_attempt a WHERE a.trigger_parent_run_id=OLD.id))
BEGIN SELECT RAISE(ABORT,'delegation parent Run identity is immutable'); END;
CREATE TRIGGER delegation_turn_scope_update BEFORE UPDATE OF camp_id,kind,trigger_type,trigger_id ON camp_turn
WHEN OLD.kind='delegation' AND (NEW.camp_id IS NOT OLD.camp_id OR NEW.kind IS NOT OLD.kind
    OR NEW.trigger_type IS NOT OLD.trigger_type OR NEW.trigger_id IS NOT OLD.trigger_id)
BEGIN SELECT RAISE(ABORT,'delegation Turn identity is immutable'); END;

CREATE TABLE delegation_workspace_permission (
    delegation_id TEXT PRIMARY KEY REFERENCES delegation(id) ON DELETE CASCADE,
    action_id TEXT NOT NULL UNIQUE REFERENCES action_execution(id),
    initial_config_digest TEXT NOT NULL CHECK(length(initial_config_digest)>0),
    created_at TEXT NOT NULL
);
CREATE TRIGGER delegation_workspace_permission_insert BEFORE INSERT ON delegation_workspace_permission BEGIN
    SELECT CASE WHEN NOT EXISTS(
        SELECT 1 FROM delegation d JOIN delegation_attempt a ON a.delegation_id=d.id AND a.attempt_no=1
        JOIN agent_run r ON r.id=a.agent_run_id JOIN action_execution x ON x.id=NEW.action_id
        JOIN approval p ON p.action_id=x.id
        WHERE d.id=NEW.delegation_id AND x.agent_run_id=r.id
          AND r.status='queued' AND r.execution_epoch=0 AND r.cancel_requested_at IS NULL
          AND NEW.initial_config_digest=json_extract(r.effective_config_json,'$.configDigest')
          AND x.action_kind='runtime_permission_grant' AND x.policy_version='delegation-workspace-v1'
          AND x.execution_authority='core' AND x.control_mode='mediated' AND x.policy_decision='ask'
          AND x.source_agent_run_execution_epoch=0 AND x.status='prepared'
          AND x.native_action_id IS NULL AND x.native_request_method IS NULL AND x.native_request_id_json IS NULL
          AND json_extract(x.canonical_input_json,'$.cwd')=json_extract(d.resolved_workspace_json,'$.executionRoot')
          AND json_extract(x.canonical_input_json,'$.permissions.workspace')=json(d.resolved_workspace_json)
          AND p.status='pending' AND p.action_digest=x.action_digest
          AND p.request_policy_version=x.policy_version AND p.requested_for_user_id='local_user'
    ) THEN RAISE(ABORT,'delegation workspace permission scope mismatch') END;
END;
CREATE TRIGGER delegation_workspace_permission_immutable BEFORE UPDATE ON delegation_workspace_permission
BEGIN SELECT RAISE(ABORT,'delegation workspace permission is immutable'); END;
CREATE TRIGGER delegation_workspace_permission_delete BEFORE DELETE ON delegation_workspace_permission
WHEN EXISTS(SELECT 1 FROM delegation WHERE id=OLD.delegation_id)
BEGIN SELECT RAISE(ABORT,'delegation workspace permission belongs to its retained delegation'); END;
