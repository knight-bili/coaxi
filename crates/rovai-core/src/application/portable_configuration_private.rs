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
