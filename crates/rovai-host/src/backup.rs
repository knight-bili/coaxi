use anyhow::{Result,Context,ensure};
use clap::{Args,ValueEnum};
use std::{io::Read,path::PathBuf};
#[derive(Args)]
pub struct Arguments{
    #[arg(value_enum)]operation:Operation,
    #[arg(long)]data_dir:PathBuf,
    #[arg(long)]file:Option<PathBuf>,
}
#[derive(Clone,ValueEnum)]
enum Operation{Create,Inspect,Stage,Apply,Rollback}
pub fn run(args:Arguments)->Result<()>{
    ensure!(args.data_dir.is_absolute(),"backup.absolute_data_directory_required");
    if matches!(args.operation,Operation::Apply){let result=rovai_core::backup::apply_pending(&args.data_dir)?;println!("{}",serde_json::json!({"applied":result.is_some(),"manifest":result}));return Ok(())}
    if matches!(args.operation,Operation::Rollback){rovai_core::backup::rollback(&args.data_dir)?;println!("{}",serde_json::json!({"rolledBack":true}));return Ok(())}
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
