use anyhow::{Context, Result, ensure};
use clap::{Args, ValueEnum};
use std::{io::Read, path::PathBuf};
#[derive(Args)]
pub struct Arguments {
    #[arg(value_enum)]
    operation: Operation,
    #[arg(long)]
    data_dir: PathBuf,
    #[arg(long)]
    file: PathBuf,
    #[arg(long)]
    preferences_root: Option<PathBuf>,
    #[arg(long)]
    expected_local_digest: Option<String>,
    #[arg(long)]
    summary_only: bool,
}
#[derive(Clone, ValueEnum)]
enum Operation {
    Export,
    Inspect,
    Apply,
}
pub fn run(args: Arguments) -> Result<()> {
    ensure!(
        args.data_dir.is_absolute() && args.file.is_absolute(),
        "config_sync.absolute_paths_required"
    );
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        passphrase: String,
    }
    let mut bytes = Vec::new();
    std::io::stdin().take(65537).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 65536, "config_sync.secret_input_limit");
    let input: Input = serde_json::from_slice(&bytes)?;
    let preferences = args
        .preferences_root
        .unwrap_or_else(|| args.data_dir.clone());
    ensure!(
        preferences.is_absolute(),
        "config_sync.absolute_preferences_root_required"
    );
    let preferences = std::fs::canonicalize(preferences)?;
    let cancel = tokio_util::sync::CancellationToken::new();
    let result = match args.operation {
        Operation::Export => rovai_core::backup::light_config::export(
            &args.data_dir,
            &preferences,
            &args.file,
            &input.passphrase,
            &cancel,
        )?,
        Operation::Inspect => rovai_core::backup::light_config::inspect(
            &args.data_dir,
            &preferences,
            &args.file,
            &input.passphrase,
            &cancel,
        )?,
        Operation::Apply => rovai_core::backup::light_config::apply(
            &args.data_dir,
            &preferences,
            &args.file,
            &input.passphrase,
            args.expected_local_digest
                .as_deref()
                .context("config_sync.expected_digest_required")?,
            &cancel,
        )?,
    };
    let mut result = result;
    if args.summary_only {
        if let Some(changes) = result["changes"].as_array_mut() {
            for change in changes {
                if change["domain"] == "messageLibrary" {
                    for side in ["before", "after"] {
                        if let Some(messages) = change[side]["quickMessages"].as_array_mut() {
                            for message in messages {
                                let bytes = message["text"]
                                    .as_str()
                                    .map(|value| value.len())
                                    .unwrap_or(0);
                                if let Some(object) = message.as_object_mut() {
                                    object.remove("text");
                                    object.insert("textBytes".into(), serde_json::json!(bytes));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("{result}");
    Ok(())
}
