//! Codeg project-boot CLI recipes, with Core-owned launch plans and receipts.
use super::*;
use super::workbench::{Scope,relative};
use anyhow::ensure;
use serde::{Deserialize,Serialize};
use std::{fs,sync::LazyLock};
use tokio_util::sync::CancellationToken;
#[derive(Deserialize,Serialize,Clone)] #[serde(rename_all="camelCase",deny_unknown_fields)]
struct Options{scope:Scope,kind:String,name:String,#[serde(default)]parent:String,package_manager:String,template:Option<String>,preset:Option<String>,example:Option<String>,resolution:Option<String>}
#[derive(Deserialize,Serialize)] #[serde(rename_all="camelCase")]
struct Plan{id:String,options:Options,root:PathBuf,program:PathBuf,program_digest:String,arguments:Vec<String>}
#[derive(Deserialize)] #[serde(rename_all="camelCase",deny_unknown_fields)]
struct Request{scope:Scope,operation:String,options:Option<Options>,plan_id:Option<String>,command_id:Option<String>,#[serde(default)]confirmed:bool}
struct Active{root:PathBuf,cancel:CancellationToken,log:Arc<tokio::sync::Mutex<Vec<u8>>>}
static ACTIVE:LazyLock<tokio::sync::Mutex<HashMap<String,Active>>>=LazyLock::new(Default::default);
static ADMISSION:tokio::sync::Mutex<()>=tokio::sync::Mutex::const_new(());
fn resolve(name:&str)->Result<PathBuf>{
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()){
        if !directory.is_absolute(){continue}
        let suffixes=if cfg!(windows){vec![".exe",".cmd",".bat"]}else{vec![""]};
        for suffix in suffixes{let path=directory.join(format!("{name}{suffix}"));if path.is_file(){return Ok(path)}}
    }anyhow::bail!("launcher.package_manager_unavailable: {name}")
}
fn recipe(options:&Options)->Result<(PathBuf,Vec<String>)>{
    let name=&options.name;ensure!(!name.is_empty()&&name.len()<=128&&!name.starts_with('-')&&!name.contains(['/', '\\', ':','\0','\r','\n'])&&name!="."&&name!=".."&&!name.ends_with(['.',' ']),"launcher.invalid_name");
    let (program,prefix)=match options.package_manager.as_str(){"npm"=>("npx",vec!["--yes"]),"pnpm"=>("pnpm",vec!["dlx"]),"yarn"=>("yarn",vec!["dlx"]),"bun"=>("bunx",vec![]),_=>anyhow::bail!("launcher.invalid_package_manager")};
    let mut arguments=prefix.into_iter().map(String::from).collect::<Vec<_>>();
    match options.kind.as_str(){
        "shadcn"=>{let template=options.template.as_deref().unwrap_or("start");ensure!(["next","vite","start","react-router","laravel","astro"].contains(&template),"launcher.invalid_template");let preset=options.preset.as_deref().context("launcher.preset_required")?;ensure!(!preset.is_empty()&&preset.len()<=1024&&preset.bytes().all(|byte|byte.is_ascii_alphanumeric()||b"_-".contains(&byte)),"launcher.invalid_preset");arguments.extend(["shadcn@latest","init","-n",name,"-t",template,"-p",preset,"-y"].into_iter().map(String::from));},
        "hyperframes"=>{let example=options.example.as_deref().unwrap_or("blank");ensure!(!example.is_empty()&&example.len()<=128&&!example.starts_with('-')&&!example.contains(['\0','\r','\n']),"launcher.invalid_example");arguments.extend(["hyperframes@latest","init",name,"--example",example].into_iter().map(String::from));if let Some(resolution)=options.resolution.as_deref().filter(|value|!value.is_empty()){ensure!(["1920x1080","1080x1920","1080x1080","1280x720","720x1280","landscape","portrait","square","landscape-4k","portrait-4k"].contains(&resolution),"launcher.invalid_resolution");arguments.extend(["--resolution".into(),resolution.into()]);}arguments.push("--non-interactive".into());},
        _=>anyhow::bail!("launcher.invalid_kind")
    }Ok((resolve(program)?,arguments))
}
impl Core{
    pub(super) async fn project_launcher(&self,value:Value)->Result<Value>{
        let request:Request=serde_json::from_value(value)?;let root=self.workbench_root(&request.scope).await?;
        if request.operation=="detect"{let mut tools=Vec::new();for name in ["npm","pnpm","yarn","bun"]{let path=resolve(name);tools.push(json!({"name":name,"installed":path.is_ok()}));}return Ok(json!({"tools":tools}))}
        let plans=self.data_dir.join("project-launcher-plans");crate::platform::private_storage::prepare_private_directory(&plans)?;
        if request.operation=="preview"{let options=request.options.context("launcher.options_required")?;ensure!(options.scope.thread_id==request.scope.thread_id&&options.scope.mission_id==request.scope.mission_id,"launcher.scope_changed");let parent=relative(&root,&options.parent,false)?;ensure!(parent.is_dir()&&!parent.join(&options.name).exists(),"launcher.destination_exists");let (program,arguments)=recipe(&options)?;let id=uuid::Uuid::new_v4().to_string();let plan=Plan{id:id.clone(),options,root,program_digest:crate::backup::file_digest(&program)?.1,program,arguments};crate::platform::private_storage::atomic_write_private_json(&plans.join(format!("{id}.json")),&plan)?;return Ok(json!({"planId":id,"arguments":plan.arguments,"name":plan.options.name,"parent":plan.options.parent,"kind":plan.options.kind}))}
        let id=request.command_id.context("launcher.command_id_required")?;uuid::Uuid::parse_str(&id)?;let receipts=self.data_dir.join("project-launcher-operations");crate::platform::private_storage::prepare_private_directory(&receipts)?;let receipt=receipts.join(format!("{id}.json"));
        if matches!(request.operation.as_str(),"status"|"cancel"){
            let active=ACTIVE.lock().await;if let Some(task)=active.get(&id){ensure!(task.root==root,"launcher.scope_changed");if request.operation=="cancel"{task.cancel.cancel()}return Ok(json!({"commandId":id,"state":"running","log":String::from_utf8_lossy(&task.log.lock().await).into_owned()}))}
            if !receipt.exists(){return Ok(json!({"commandId":id,"state":"not_found"}))}let mut record:Value=serde_json::from_slice(&fs::read(receipt)?)?;ensure!(record["root"]==json!(root),"launcher.scope_changed");if record["state"]=="running"{record["state"]=json!("unknown")}record.as_object_mut().unwrap().remove("root");return Ok(record)
        }
        ensure!(request.operation=="start"&&request.confirmed,"launcher.confirmation_required");let plan_id=request.plan_id.context("launcher.plan_required")?;uuid::Uuid::parse_str(&plan_id)?;
        let _admission=ADMISSION.lock().await;
        if receipt.exists(){let mut record:Value=serde_json::from_slice(&fs::read(&receipt)?)?;ensure!(record["root"]==json!(root)&&record["planId"]==plan_id,"launcher.command_identity_changed");record.as_object_mut().unwrap().remove("root");return Ok(record)}
        let plan:Plan=serde_json::from_slice(&fs::read(plans.join(format!("{plan_id}.json")))?)?;ensure!(plan.id==plan_id&&plan.root==root&&crate::backup::file_digest(&plan.program)?.1==plan.program_digest,"launcher.plan_changed");
        let parent=relative(&root,&plan.options.parent,false)?;let destination=parent.join(&plan.options.name);ensure!(!destination.exists(),"launcher.destination_exists");
        let stage=parent.join(format!(".rovai-project-{id}"));crate::platform::private_storage::prepare_private_directory(&stage)?;
        let record=json!({"root":root,"commandId":id,"planId":plan_id,"state":"running","path":destination,"partialPath":stage,"startedAt":Utc::now().to_rfc3339()});crate::platform::private_storage::atomic_write_private_json(&receipt,&record)?;
        let cancel=CancellationToken::new();let log=Arc::new(tokio::sync::Mutex::new(Vec::new()));ACTIVE.lock().await.insert(id.clone(),Active{root:root.clone(),cancel:cancel.clone(),log:log.clone()});
        let started=json!({"commandId":id,"state":"running"});
        tokio::spawn(async move{
            let result=async{
                let mut command=crate::office_preview::tokio_command(&plan.program);command.current_dir(&stage).args(&plan.arguments);for key in ["APPDATA","XDG_CACHE_HOME","XDG_CONFIG_HOME","USER","USERNAME","SHELL","ComSpec","COMSPEC"]{if let Some(value)=std::env::var_os(key){command.env(key,value);}}
                crate::managed_operation::run_bounded_command(command,&format!("project-launcher-{id}"),Duration::from_secs(1200),cancel,log.clone()).await?;
                let generated=stage.join(&plan.options.name);let metadata=fs::symlink_metadata(&generated)?;ensure!(metadata.is_dir()&&!metadata.file_type().is_symlink(),"launcher.generated_directory_missing");ensure!(!destination.exists(),"launcher.destination_exists");fs::rename(&generated,&destination)?;let _=fs::remove_dir(&stage);Ok::<(),anyhow::Error>(())
            }.await;
            let mut record=record;record["completedAt"]=json!(Utc::now().to_rfc3339());record["log"]=json!(String::from_utf8_lossy(&log.lock().await).into_owned());match result{Ok(())=>{record["state"]=json!("completed");record["partialPath"]=Value::Null},Err(error)=>{record["state"]=json!("failed");record["error"]=json!(error.to_string())}}let _=crate::platform::private_storage::atomic_write_private_json(&receipt,&record);ACTIVE.lock().await.remove(&id);
        });
        Ok(started)
    }
}
