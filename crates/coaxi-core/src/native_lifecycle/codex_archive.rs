//! 原生归档是维护操作，不启动或恢复模型轮次。
use super::*;
use coaxi_core::runtime_subagent_session::native_archive::NativeArchiveTarget;

pub(crate) async fn archive(target: &NativeArchiveTarget) -> Result<()> {
    anyhow::ensure!(target.home.is_dir(), "native_archive.source_unavailable");
    anyhow::ensure!(coaxi_core::agent_runtime_adapter::executable_fingerprint(&target.executable)?==target.executable_fingerprint,"native_archive.program_changed");
    let mut values=coaxi_core::runtime_discovery::capture_host_runtime_environment();
    values.retain(|key,_|!key.eq_ignore_ascii_case("CODEX_HOME"));
    values.insert("CODEX_HOME".into(),target.home.to_string_lossy().into_owned());
    let environment=coaxi_core::member_environment::LaunchEnvironment{values:Some(values),overlay:None};
    let (incoming,mut receiver)=mpsc::unbounded_channel();
    let cwd=if target.cwd.is_dir(){&target.cwd}else{&target.home};
    let host=CodexHost::spawn_uninitialized(&target.executable,cwd,incoming,None,None,Some(environment)).await?;
    let drain=tokio::spawn(async move {while receiver.recv().await.is_some(){}});
    let result=timeout(Duration::from_secs(60),async {
        host.initialize().await?;
        // 显式列出来源并遍历分页，支持上次归档成功但回执丢失后的重试。
        let sources=json!(["cli","vscode","exec","appServer","subAgent","subAgentReview","subAgentCompact","subAgentThreadSpawn","subAgentOther","unknown"]);
        let mut cursor: Option<String>=None;
        let mut seen=HashSet::new();
        loop {
            let page=host.rpc("thread/list",json!({"archived":true,"limit":100,"cursor":cursor,"sourceKinds":sources,"modelProviders":[]})).await?;
            let items=page["data"].as_array().context("native_archive.invalid_listing")?;
            if items.iter().any(|item|item["id"]==target.session_id){return Ok(());}
            cursor=page["nextCursor"].as_str().map(str::to_owned);
            match &cursor {None=>break,Some(value)=>anyhow::ensure!(seen.insert(value.clone())&&seen.len()<=1000,"native_archive.invalid_cursor")}
        }
        host.rpc("thread/archive",json!({"threadId":target.session_id})).await?;
        Ok::<_,anyhow::Error>(())
    }).await.map_err(|_|anyhow::anyhow!("native_archive.outcome_unknown")).and_then(|result|result);
    host.shutdown_and_reap().await;
    drain.abort();
    result
}
