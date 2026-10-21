use anyhow::{Result,Context,ensure};
use clap::{Args,ValueEnum};
use std::{io::Read,path::PathBuf};
#[derive(Args)]
pub struct Arguments{
    #[arg(value_enum)]operation:Operation,
    #[arg(long)]data_dir:PathBuf,
    #[arg(long)]file:Option<PathBuf>,
    #[arg(long)]operation_id:Option<String>,
    #[arg(long)]snapshot_id:Option<String>,
}
#[derive(Clone,ValueEnum)]
enum Operation{Create,Inspect,Stage,Apply,Rollback,WebdavList,WebdavProbe,WebdavPush,WebdavPull}
pub fn run(args:Arguments)->Result<()>{
    ensure!(args.data_dir.is_absolute(),"backup.absolute_data_directory_required");
    if matches!(args.operation,Operation::Apply){let result=rovai_core::backup::apply_pending(&args.data_dir)?;println!("{}",serde_json::json!({"applied":result.is_some(),"manifest":result}));return Ok(())}
    if matches!(args.operation,Operation::Rollback){rovai_core::backup::rollback(&args.data_dir)?;println!("{}",serde_json::json!({"rolledBack":true}));return Ok(())}
    if matches!(args.operation,Operation::WebdavList|Operation::WebdavProbe|Operation::WebdavPush|Operation::WebdavPull){
        let settings:serde_json::Value=serde_json::from_slice(&std::fs::read(args.data_dir.join("webdav-settings-v1.json"))?)?;
        let config:rovai_core::backup::webdav::Config=serde_json::from_value(settings["config"].clone())?;
        let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        return runtime.block_on(async{
            let client=rovai_core::backup::webdav::WebDav::new(config)?;
            let result=match args.operation{
                Operation::WebdavList=>serde_json::json!({"entries":client.list().await?}),
                Operation::WebdavProbe=>client.probe().await?,
                Operation::WebdavPush=>{let file=args.file.context("webdav.file_required")?;ensure!(file.is_absolute(),"webdav.absolute_file_required");let id=args.operation_id.context("webdav.operation_id_required")?;serde_json::json!({"snapshot":client.push(&file,&id,None).await?})},
                Operation::WebdavPull=>{let file=args.file.context("webdav.file_required")?;ensure!(file.is_absolute()&&!std::fs::canonicalize(file.parent().context("webdav.parent_required")?)?.starts_with(std::fs::canonicalize(&args.data_dir)?),"webdav.destination_invalid");let id=args.snapshot_id.context("webdav.snapshot_id_required")?;serde_json::json!({"snapshot":client.pull(&id,&file,None).await?})},
                _=>unreachable!(),
            };println!("{}",result);Ok(())
        });
    }
    #[derive(serde::Deserialize)]#[serde(deny_unknown_fields)]struct Input{passphrase:String}
    let mut input=Vec::new();std::io::stdin().take(65537).read_to_end(&mut input)?;ensure!(input.len()<=65536,"backup.secret_input_limit");let input:Input=serde_json::from_slice(&input)?;
    let file=args.file.context("backup.file_required")?;ensure!(file.is_absolute(),"backup.absolute_file_required");
    let cancel=tokio_util::sync::CancellationToken::new();
    let manifest=match args.operation{
        Operation::Create=>rovai_core::backup::create(&args.data_dir,&file,&input.passphrase,&cancel)?,
        Operation::Inspect=>rovai_core::backup::inspect(&args.data_dir,&file,&input.passphrase,&cancel)?,
        Operation::Stage=>rovai_core::backup::stage(&args.data_dir,&file,&input.passphrase,&cancel)?,
        _=>unreachable!(),
    };
    println!("{}",serde_json::json!({"manifest":manifest}));Ok(())
}
