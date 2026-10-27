//! 删除前只收集由 Core 记录且归属于本 Chat 的原生根会话。
use super::origin;
use crate::{agent_profile::AdapterKind, db::Database};
use anyhow::{Context, Result, ensure};
use rusqlite::params;
use std::{collections::BTreeSet, path::PathBuf};

pub struct NativeArchiveTarget {
    pub session_id: String,
    pub home: PathBuf,
    pub cwd: PathBuf,
    pub executable: PathBuf,
    pub executable_fingerprint: String,
}

fn home_key(home: &str) -> Result<PathBuf> {
    std::fs::canonicalize(home).context("native_archive.home_unavailable")
}

pub fn collect(database: &Database, camp: &str) -> Result<Vec<NativeArchiveTarget>> {
    let connection = database.connection();
    let mut query = connection.prepare(
        "SELECT e.entity_id,e.execution_epoch,e.payload_json,r.effective_config_json,
                r.conversation_id,COALESCE(r.camp_id,t.camp_id)
         FROM event_log e LEFT JOIN agent_run r ON r.id=json_extract(e.payload_json,'$.agentRunId')
         LEFT JOIN camp_turn t ON t.id=r.camp_turn_id
         WHERE e.event_type='conversation.native_source.recorded' AND e.camp_id=?1
           AND e.entity_type='conversation' AND e.actor_type='system'
           AND json_extract(e.payload_json,'$.adapterKind')='codex-cli' ORDER BY e.rowid",
    )?;
    let mut targets = Vec::new();
    let mut seen = BTreeSet::new();
    for row in query.query_map([camp], |row| Ok((row.get::<_,String>(0)?,row.get::<_,i64>(1)?,row.get::<_,String>(2)?,row.get::<_,Option<String>>(3)?,row.get::<_,Option<String>>(4)?,row.get::<_,Option<String>>(5)?)))? {
        let (conversation,epoch,payload,config,run_conversation,run_camp) = row?;
        let record: origin::OriginRecord = serde_json::from_str(&payload)?;
        ensure!(run_conversation.as_deref()==Some(&conversation) && run_camp.as_deref()==Some(camp), "native_archive.source_unproven");
        let verified=origin::lookup(database,camp,&conversation,&record.agent_run_id,epoch,"codex-cli",&record.parent_native_session_id)?.context("native_archive.source_unproven")?;
        ensure!(verified==record, "native_archive.source_unproven");
        uuid::Uuid::parse_str(&record.parent_native_session_id).context("native_archive.invalid_session")?;
        let config: serde_json::Value=serde_json::from_str(&config.context("native_archive.program_unproven")?)?;
        let runtime: crate::agent_profile::FrozenAgentRuntimeConfig=serde_json::from_value(config["runtime"].clone())?;
        ensure!(runtime.adapter_kind==AdapterKind::CodexCli && runtime.installation_id==record.installation_id && runtime.binding_compatibility_digest==record.compatibility_digest, "native_archive.program_unproven");
        let key=(home_key(&record.home_path)?,record.parent_native_session_id.clone());
        if !seen.insert(key.clone()) {continue;}
        let mut others=connection.prepare("SELECT json_extract(e.payload_json,'$.homePath') FROM event_log e JOIN camp c ON c.id=e.camp_id WHERE e.event_type='conversation.native_source.recorded' AND e.actor_type='system' AND e.camp_id<>?1 AND c.deletion_operation_id IS NULL AND json_extract(e.payload_json,'$.adapterKind')='codex-cli' AND json_extract(e.payload_json,'$.parentNativeSessionId')=?2")?;
        for other in others.query_map(params![camp,record.parent_native_session_id],|row|row.get::<_,String>(0))? {
            ensure!(home_key(&other?)?!=key.0,"native_archive.shared_session");
        }
        let leased: bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM native_lifecycle_source_lease l LEFT JOIN native_lifecycle_continuation n ON n.command_id=l.command_id WHERE l.provider='codex' AND l.native_session_id=?1 AND l.state<>'released' AND (n.thread_id IS NULL OR n.thread_id<>?2))",params![record.parent_native_session_id,camp],|row|row.get(0))?;
        ensure!(!leased,"native_archive.shared_session");
        let cwd=record.working_directory.map(PathBuf::from).unwrap_or_else(||key.0.clone());
        targets.push(NativeArchiveTarget {session_id:record.parent_native_session_id,home:key.0,cwd,executable:PathBuf::from(runtime.executable_path),executable_fingerprint:runtime.executable_fingerprint});
    }
    // 老记录没有来源时拒绝猜测 Home；保留删除操作供核对与重试。
    let missing: bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM conversation c JOIN adapter_installation a ON a.id=c.native_adapter_installation_id WHERE c.camp_id=?1 AND c.native_session_id IS NOT NULL AND a.adapter_kind='codex-cli' AND NOT EXISTS(SELECT 1 FROM event_log e WHERE e.camp_id=c.camp_id AND e.entity_id=c.id AND e.event_type='conversation.native_source.recorded' AND json_extract(e.payload_json,'$.parentNativeSessionId')=c.native_session_id))",[camp],|row|row.get(0))?;
    ensure!(!missing,"native_archive.source_unproven");
    Ok(targets)
}
