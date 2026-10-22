//! 可信 Host 的 User 私有输入；不加入通用 Web 操作或模型工具，也不记录输入正文。
use super::*;
use crate::portable_configuration::{self as configuration,archive,archive_store,secrets};
use std::io::Read;

#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ArchiveInput{operation_id:String,path:PathBuf,source_digest:String,byte_size:u64,passphrase:String,manifest:Option<Value>}

impl Core{
    pub(super) async fn portable_configuration_private(&self,method:&str,value:Value,client:&crate::draft_client::DraftClient)->Result<Value>{
        let result=self.portable_configuration_private_inner(method,value,client).await;
        // 上游解析错误可能含原输入；私有入口只返回稳定原因，不回显秘密。
        result.map_err(|_|anyhow::anyhow!("configuration.private_input_rejected"))
    }
    async fn portable_configuration_private_inner(&self,method:&str,value:Value,client:&crate::draft_client::DraftClient)->Result<Value>{
        ensure!(serde_json::to_vec(&value)?.len()<=configuration::MAX_REQUEST,"configuration.request_too_large");
        if method=="configuration.files.finish"{
            #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Input{command_id:String}
            let input:Input=serde_json::from_value(value)?;
            let _lock=crate::workbench::MUTATIONS.lock().await;let database=self.database.lock().await;
            let original=configuration::migration::receipt(database.connection(),client.id(),&input.command_id)?;
            ensure!(original["state"]=="applied","configuration.original_database_result_required");
            // 只接续已提交的原文件计划，不能重新导入或重试已经冲突、失败的计划。
            configuration::files::drain(database.connection(),&self.data_dir,&input.command_id)?;
            let result=configuration::migration::receipt(database.connection(),client.id(),&input.command_id)?;
            emit(&self.output,"portableConfiguration.filesChanged",json!({"commandId":input.command_id,"readReceipt":true}));return Ok(result)
        }
        if method=="configuration.taskTemplates"{
            let _lock=crate::workbench::MUTATIONS.lock().await;let mut database=self.database.lock().await;
            let changed=matches!(value["operation"].as_str(),Some("put"|"remove"));
            let result=configuration::task_templates::access(&mut database,&self.data_dir,client,value)?;
            if changed{emit(&self.output,"portableConfiguration.templatesChanged",json!({"readLibrary":true}));}return Ok(result)
        }
        if method=="configuration.logging"{
            let _lock=crate::workbench::MUTATIONS.lock().await;
            return crate::diagnostic_preferences::access(&self.data_dir,value)
        }
        if method=="configuration.sync"{
            if value["operation"]=="receipt"{
                #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Input{operation:String,command_id:String}
                let input:Input=serde_json::from_value(value)?;ensure!(input.operation=="receipt","configuration.sync_operation_invalid");
                let _lock=crate::workbench::MUTATIONS.lock().await;
                return super::config_sync::original_receipt(&self.data_dir,&input.command_id)
            }
            ensure!(value["operation"].as_str().is_some_and(|operation|["get","set","upload","cancel","resume","archive","cache_preview","cache_remove","cache_status"].contains(&operation)),"configuration.sync_operation_invalid");
            return self.config_sync_access(value).await
        }
        if method=="configuration.export.prepare"{
            let input:configuration::export_artifact::Prepare=serde_json::from_value(value)?;
            let _lock=crate::workbench::MUTATIONS.lock().await;let database=self.database.lock().await;
            let frozen=configuration::export_artifact::freeze(database.connection(),&self.data_dir,client.id(),input)?;drop(database);
            return tokio::task::spawn_blocking(move||configuration::export_artifact::finish(frozen)).await.context("configuration.export_worker_unavailable")?
        }
        if matches!(method,"configuration.export.receipt"|"configuration.export.read"){
            let input:configuration::export_artifact::ReadExport=serde_json::from_value(value)?;
            return configuration::export_artifact::read(&self.data_dir,client.id(),input,method=="configuration.export.read")
        }
        if matches!(method,"configuration.desktopOwner.register"|"configuration.desktopOwner.pending"|"configuration.desktopOwner.acknowledge"){
            ensure!(client.is_desktop(),"configuration.desktop_owner_required");
            let _lock=crate::workbench::MUTATIONS.lock().await;let database=self.database.lock().await;
            if method=="configuration.desktopOwner.register"{
                #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Input{preferences_root:PathBuf}
                let input:Input=serde_json::from_value(value)?;
                crate::backup::owned_roots::register_desktop(&self.data_dir,&input.preferences_root)?;
                return Ok(json!({"registered":true}))
            }
            return if method=="configuration.desktopOwner.pending"{
                configuration::desktop_owner::pending(database.connection(),&self.data_dir,serde_json::from_value(value)?)
            }else{
                let command=value["commandId"].as_str().context("configuration.command_required")?.to_owned();
                let result=configuration::desktop_owner::acknowledge(database.connection(),&self.data_dir,serde_json::from_value(value)?)?;
                emit(&self.output,"portableConfiguration.filesChanged",json!({"commandId":command,"readReceipt":true}));Ok(result)
            }
        }
        if method=="configuration.secrets.prepare"{
            let mut database=self.database.lock().await;return secrets::prepare(&mut database,client.id(),&value)
        }
        if method=="configuration.providers.privateEdit"{
            let reference:configuration::PortableConnectionRefV1=serde_json::from_value(value)?;reference.validate()?;
            let database=self.database.lock().await;return configuration::providers::resolve(database.connection(),&reference,false)
        }
        if method=="configuration.archive.receipt"{
            #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Input{operation_id:String}
            let input:Input=serde_json::from_value(value)?;let database=self.database.lock().await;
            return Ok(json!({"archive":archive_store::receipt(database.connection(),client.id(),&input.operation_id)?}))
        }
        ensure!(matches!(method,"configuration.archive.registerLocal"|"configuration.archive.registerSelected"),"configuration.operation_unknown");
        let selected=method=="configuration.archive.registerSelected";
        ensure!(!selected||client.is_desktop(),"configuration.desktop_selection_required");
        let input:ArchiveInput=serde_json::from_value(value)?;
        uuid::Uuid::parse_str(&input.operation_id)?;
        ensure!(input.byte_size<=configuration::MAX_ARCHIVE as u64&&input.passphrase.len()<=16000,"configuration.archive_too_large");
        {
            let database=self.database.lock().await;
            if let Some(previous)=archive_store::receipt(database.connection(),client.id(),&input.operation_id)?{
                ensure!(previous["sourceDigest"]==input.source_digest,"configuration.archive_operation_changed");return Ok(previous)
            }
        }
        let parsed=tokio::task::spawn_blocking(move||->Result<_>{
            let mut file=if selected{
                ensure!(input.path.is_absolute(),"configuration.absolute_source_required");
                let metadata=std::fs::symlink_metadata(&input.path)?;
                ensure!(metadata.is_file()&&!metadata.file_type().is_symlink(),"configuration.source_kind_invalid");
                // Main 持有文件选择，Core 另核对整份内容 SHA；不接受 Renderer 任意路径。
                std::fs::File::open(&input.path)?
            }else{crate::platform::private_storage::open_private_read_file(&input.path)?};
            ensure!(file.metadata()?.len()==input.byte_size,"configuration.archive_changed");
            let mut bytes=Vec::new();file.by_ref().take(configuration::MAX_ARCHIVE as u64+1).read_to_end(&mut bytes)?;
            ensure!(bytes.len()as u64==input.byte_size,"configuration.archive_changed");
            let parsed=archive::parse(&bytes,&input.passphrase,input.manifest.as_ref())?;
            ensure!(parsed.source_digest==input.source_digest,"configuration.archive_changed");Ok((input.operation_id,parsed))
        }).await??;
        let database=self.database.lock().await;
        archive_store::register(database.connection(),&self.data_dir,client.id(),&parsed.0,parsed.1)
    }
}
