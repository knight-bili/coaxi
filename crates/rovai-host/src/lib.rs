mod backup;
mod config_snapshot;
mod desktop_channels;
mod diagnostic_preferences;
mod lifecycle;
mod program_recovery;
mod server;
mod server_channels;
mod server_logs;
mod server_updates;
mod signals;
mod transport_channels;
mod web_control;

pub use server::run as run_server;

use std::{
    io::{BufRead, Read},
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use rovai_core::{
    application::{CoreConfig, embedded},
    runtime_discovery::RuntimeSearchEnvironment,
};

#[derive(Parser)]
#[command(version, about = "Run the shared Rovai Core without Electron")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Publish the local owner CLI discovery record using native private storage.
    AutomationStorage {
        #[arg(long)]
        directory: PathBuf,
    },
    /// Inspect or explicitly switch an installer-owned program revision; never rewrite business data.
    ProgramRecovery(program_recovery::Arguments),
    /// Register only Desktop-owned state units for the current local data root.
    BackupPreferences {
        #[arg(long)]
        data_dir: PathBuf,
        #[arg(long)]
        preferences_root: PathBuf,
    },
    /// Offline encrypted backup/restore; passphrase is read from stdin only.
    Backup(backup::Arguments),
    /// Portable encrypted preference export and explicit offline import.
    ConfigSnapshot(config_snapshot::Arguments),
    /// Roll back an interrupted portable configuration import before UI stores load.
    ConfigRecover {
        #[arg(long)]
        data_dir: PathBuf,
    },
    /// Finish or roll back local recovery before Desktop caches are opened.
    RecoverLocalData {
        #[arg(long)]
        data_dir: PathBuf,
    },
    /// Start one Core owner, optionally with the shared Web module.
    Run(RunArgs),
    /// Generate a management token for an operator. Treat stdout as a secret.
    Token,
    /// Prepare only the explicitly selected private directory and print path arguments.
    Prepare {
        #[arg(long)]
        data_dir: PathBuf,
    },
}

#[derive(Args)]
struct RunArgs {
    /// Absolute Core data directory; never inferred from Desktop userData.
    #[arg(long)]
    data_dir: PathBuf,
    /// Absolute Skill Library directory belonging to this Host.
    #[arg(long)]
    skill_library_root: PathBuf,
    /// Absolute Runtime Camp files directory belonging to this Host.
    #[arg(long)]
    runtime_camp_files_root: PathBuf,
    /// Absolute MCP config path; prevents use of the daily global config.
    #[arg(long)]
    mcp_config_path: PathBuf,
    /// Allow initialization only when Core confirms that authority is absent.
    /// Without this flag, missing authority is a startup refusal.
    #[arg(long)]
    initialize: bool,
    /// Enable HTTP at this address; loopback is the safe default choice.
    #[arg(long, requires_all = ["web_ui", "web_token_stdin"])]
    web_listen: Option<SocketAddr>,
    /// Absolute directory containing the built shared WebUI.
    #[arg(long, requires = "web_listen")]
    web_ui: Option<PathBuf>,
    /// Optional exact origin for a reverse proxy; local interfaces are discovered automatically.
    #[arg(long, requires = "web_listen")]
    web_public_origin: Option<String>,
    /// Explicitly allow unencrypted LAN HTTP; use HTTPS/VPN on untrusted networks.
    #[arg(long, requires = "web_listen")]
    allow_insecure_lan: bool,
    /// Read one 64-character token from stdin. Never put it in args or environment.
    #[arg(long, requires = "web_listen")]
    web_token_stdin: bool,
}

pub fn run_host() -> Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    // Desktop keeps its existing explicit path/config arguments and parent pipe.
    // The compatibility executable remains available to old integrations.
    let desktop = arguments.first().is_some_and(|argument| {
        argument.starts_with("--") && !matches!(argument.as_str(), "--help" | "--version")
    });
    if arguments
        .first()
        .is_some_and(|argument| argument == "--prepare-windows-data-root")
    {
        return rovai_core::application::run_stdio();
    }
    let mut web = None;
    let config = if desktop {
        CoreConfig::from_legacy_args(arguments)?
    } else {
        match Cli::parse().command {
            Command::AutomationStorage { directory } => {
                anyhow::ensure!(
                    directory.is_absolute()
                        && !directory.components().any(|part| matches!(
                            part,
                            std::path::Component::CurDir | std::path::Component::ParentDir
                        )),
                    "automation_context_absolute_directory_required"
                );
                let mut bytes = Vec::new();
                std::io::stdin()
                    .take(1024 * 1024 + 1)
                    .read_to_end(&mut bytes)?;
                rovai_core::platform::write_user_automation_context(&directory, &bytes)?;
                println!("{}", serde_json::json!({"saved":true}));
                return Ok(());
            }
            Command::BackupPreferences {
                data_dir,
                preferences_root,
            } => {
                anyhow::ensure!(
                    data_dir.is_absolute() && preferences_root.is_absolute(),
                    "backup.absolute_paths_required"
                );
                rovai_core::backup::owned_roots::register_desktop(&data_dir, &preferences_root)?;
                println!("{}", serde_json::json!({"registered":true}));
                return Ok(());
            }
            Command::RecoverLocalData { data_dir } => {
                anyhow::ensure!(data_dir.is_absolute(), "backup.absolute_paths_required");
                println!("{}", rovai_core::backup::recover_before_desktop(&data_dir)?);
                return Ok(());
            }
            Command::ConfigRecover { data_dir } => {
                anyhow::ensure!(
                    data_dir.is_absolute(),
                    "config_sync.absolute_paths_required"
                );
                let recovered = rovai_core::backup::light_config::recover(&data_dir)?;
                println!("{}", serde_json::json!({"recovered":recovered}));
                return Ok(());
            }
            Command::ProgramRecovery(args) => return program_recovery::run(args),
            Command::ConfigSnapshot(args) => return config_snapshot::run(args),
            Command::Backup(args) => return backup::run(args),
            Command::Token => {
                println!("{}", rovai_web::new_token()?);
                return Ok(());
            }
            Command::Prepare { data_dir } => {
                anyhow::ensure!(
                    data_dir.is_absolute()
                        && !data_dir.components().any(|part| matches!(
                            part,
                            std::path::Component::CurDir | std::path::Component::ParentDir
                        )),
                    "prepare requires a normalized absolute path"
                );
                anyhow::ensure!(
                    !data_dir.try_exists()?,
                    "prepare requires a new directory; it does not repair or change existing data"
                );
                #[cfg(unix)]
                let data = {
                    use std::os::unix::fs::DirBuilderExt;
                    std::fs::DirBuilder::new()
                        .mode(0o700)
                        .create(&data_dir)
                        .context(
                            "could not create private data directory; its parent must exist",
                        )?;
                    std::fs::canonicalize(&data_dir)?
                };
                #[cfg(not(unix))]
                let data = rovai_core::platform::prepare_private_directory(&data_dir)?;
                let runtime_files = if cfg!(windows) {
                    data.join("runtime-files")
                } else {
                    let home = std::fs::canonicalize(
                        dirs::home_dir().context("current Home is unavailable")?,
                    )?;
                    home.join(".rovai/instances")
                        .join(rovai_core::camp_attachment_view::instance_key(&data)?)
                        .join("runtime-files")
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "dataDir": data,
                        "skillLibraryRoot": data.join("skills"),
                        "mcpConfigPath": data.join("mcp.json"),
                        "runtimeCampFilesRoot": runtime_files,
                        "runArguments": ["--data-dir", data.to_str(), "--skill-library-root", data.join("skills").to_str(), "--mcp-config-path", data.join("mcp.json").to_str(), "--runtime-camp-files-root", runtime_files.to_str()]
                    }))?
                );
                return Ok(());
            }
            Command::Run(args) => {
                if let Some(listen) = args.web_listen {
                    let mut token = String::new();
                    std::io::stdin()
                        .lock()
                        .take(66)
                        .read_line(&mut token)
                        .context("could not read Web management token from stdin")?;
                    let token = token.trim_end_matches(['\r', '\n']).to_owned();
                    web = Some((
                        rovai_web::WebConfig {
                            listen,
                            public_origin: args.web_public_origin,
                            allow_insecure_lan: args.allow_insecure_lan,
                            ui_directory: args.web_ui.context("Web UI directory is required")?,
                        },
                        token,
                    ));
                }
                CoreConfig {
                    data_dir: args.data_dir,
                    skill_library_root: args.skill_library_root,
                    runtime_camp_files_root: args.runtime_camp_files_root,
                    mcp_config_path: Some(args.mcp_config_path),
                    require_existing_authority: !args.initialize,
                    automation_scheduler_control: None,
                    removed_skill_project_roots: Default::default(),
                }
            }
        }
    };
    run(config, web, desktop, None, None)
}

fn run(
    config: CoreConfig,
    web: Option<(rovai_web::WebConfig, String)>,
    desktop: bool,
    console: Option<&server::Console>,
    updates: Option<server_updates::ServerUpdates>,
) -> Result<()> {
    use tracing_subscriber::{Layer, prelude::*};
    let preferences = diagnostic_preferences::Filter::new(config.data_dir.clone());
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_target(true)
                .with_writer(std::io::stderr)
                .with_filter(tracing_subscriber::filter::filter_fn(move |metadata| {
                    if desktop {
                        *metadata.level() <= tracing::Level::INFO
                    } else {
                        preferences.enabled(metadata)
                    }
                })),
        )
        .init();

    // Preserve the Desktop adapter's ordering: discovery captures its search
    // environment before Tokio starts and passes it to child launches explicitly.
    let environment = Arc::new(RuntimeSearchEnvironment::capture_initial());
    environment.activate_for_runtime_commands();
    let data_dir = config.data_dir.clone();
    let (core, runner) = embedded(config, environment)?;
    let control = web_control::WebControl::new(
        core.clone(),
        desktop,
        data_dir,
        updates
            .clone()
            .map(|u| Arc::new(u) as Arc<dyn rovai_web::UpdateHost>),
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to create Host Tokio runtime")?;
    let result = runtime.block_on(async {
        if desktop {
            let startup_core = core.clone(); let startup_control = control.clone();
            tokio::spawn(async move { if startup_core.wait_ready().await.is_ok() { startup_control.start_transport_adapters().await; } });
            let result = runner.with_desktop_stdio(control.clone()).run().await;
            control.shutdown().await;
            return result;
        }
        // Register listeners before starting Core, including during admission.
        let signals = signals::StopSignals::register()?;
        let result = lifecycle::run(
            core,
            runner,
            async {
                let result = tokio::select! {
                    result = signals.wait() => result,
                    () = async { if let Some(updates) = &updates { updates.requested().await } else { std::future::pending::<()>().await } } => Ok(Duration::from_secs(10)),
                };
                control.shutdown().await;
                result
            },
            async {
                control.start_transport_adapters().await;
                if let Some((config, token)) = web {
                    let status = control.start(config, &token).await?;
                    tracing::info!(
                        origin = status["origin"].as_str().unwrap_or_default(),
                        "Host Web listener started"
                    );
                    if let Some(console) = console {
                        console.ready(&status, &token);
                    }
                    if let Some(updates) = &updates { updates.start_automatic_checks(); }
                }
                Ok(())
            },
        )
        .await;
        control.shutdown().await;
        result
    });
    // Host owns this runtime. On a hard stop it must also end all remaining Core
    // task owners; it does not leave detached work running in another service.
    runtime.shutdown_timeout(Duration::from_millis(250));
    result
}
