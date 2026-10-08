use super::{GROK_PI_VERSION, PI_LOGO, cli::Args};
use anyhow::{Context, Result, bail};
use clap::Parser;
use pi_grok_adapter::durable::{DurableAgent, DurableRpc};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};
use tokio_util::sync::CancellationToken;
use xai_acp_lib::acp_channels;
use xai_grok_pager::{
    acp::{AcpConnection, ExternalLogoArt, ExternalUiProfile, ExternalWelcomeBrand},
    app::{ExternalRunReady, ExternalRunStartConfig, PagerArgs, run_external_deferred},
};
use xai_grok_shared::host_features::HostFeatureManifest;

const COMMANDS: &[&str] = &[
    "exit",
    "help",
    "hotkeys",
    "new",
    "resume",
    "model",
    "effort",
    "compact",
    "session-info",
    "copy",
    "find",
    "jump",
    "transcript",
    "export",
    "expand",
    "theme",
    "timestamps",
    "timeline",
    "multiline",
    "compact-mode",
    "vim-mode",
    "toggle-mouse-reporting",
    "doctor",
    "debug",
];

pub(super) const CAPABILITIES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../runtime/pi-durable-host/capabilities.json"
));

fn host_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("GROK_PI_DURABLE_HOST") {
        let path = PathBuf::from(path);
        if !path.is_file() {
            bail!("GROK_PI_DURABLE_HOST is not a file");
        }
        return std::path::absolute(path).context("Durable host path");
    }
    let packaged = std::env::current_exe()?
        .parent()
        .context("binary directory")?
        .join("lib/grok-pi/durable/host.mjs");
    if packaged.is_file() {
        return Ok(packaged);
    }
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../runtime/pi-durable-host/host.mjs");
    if source.is_file() {
        return std::fs::canonicalize(source).context("Durable source host");
    }
    bail!("Durable host is missing. Reinstall a package with Durable support, or use --no-durable.")
}

fn validate(args: &Args) -> Result<()> {
    if !args.extensions.is_empty() || !args.pi_args.is_empty() || !args.pi_prefix_args.is_empty() {
        bail!(
            "Durable does not execute classic Pi extensions or RPC passthrough arguments. Use --no-durable for those."
        );
    }
    if args.fork.is_some()
        || args.session_id.is_some()
        || args.session_dir.is_some()
        || args.no_session
        || args.models.is_some()
        || args.bash_max_wait_mins.is_some()
    {
        bail!(
            "Durable does not yet support --fork, --session-id, --session-dir, --no-session, --models or --bash-max-wait-mins."
        );
    }
    if args
        .session
        .as_ref()
        .is_some_and(|s| !s.starts_with("durable:"))
    {
        bail!("Pi JSONL sessions must be opened with --no-durable.");
    }
    Ok(())
}

pub(super) async fn run(args: Args, cwd: PathBuf) -> Result<()> {
    validate(&args)?;
    let host = host_path()?;
    let home = xai_grok_config::grok_home();
    let config = xai_grok_config::load_effective_config_disk_only().ok();
    let selected_tools = args.tools.clone().or_else(|| {
        config
            .as_ref()?
            .get("ui")?
            .get("pi_builtin_tools")
            .map(|prefs| {
                ["read", "write", "edit", "bash"]
                    .into_iter()
                    .filter(|name| {
                        prefs
                            .get(*name)
                            .and_then(toml::Value::as_bool)
                            .unwrap_or(true)
                    })
                    .chain(std::iter::once("subagent"))
                    .collect::<Vec<_>>()
                    .join(",")
            })
    });
    let options = json!({"home":home,"cwd":cwd,"backgroundOwner":args.durable_background,"continue":args.continue_last_session,"session":args.session,"name":args.name,"provider":args.provider,"model":args.model,"thinking":args.thinking,"systemPrompt":args.system_prompt,"appendSystemPrompts":args.append_system_prompts,"approve":args.approve && !args.no_approve,"noContextFiles":args.no_context_files,"noSkills":args.no_skills,"noTools":args.no_tools || args.no_builtin_tools,"tools":selected_tools,"excludeTools":args.exclude_tools});
    let mut pager = PagerArgs::parse_from(["grok-pi"]);
    pager.no_alt_screen = args.no_alt_screen;
    pager.minimal = args.minimal;
    pager.fullscreen = args.fullscreen;
    pager.no_auto_update = std::env::var_os("GROK_PI_NO_AUTO_UPDATE").is_some();
    let start = ExternalRunStartConfig {
        args: pager,
        session_cwd: Some(cwd.clone()),
        product_version: GROK_PI_VERSION.to_string(),
    };
    let owner = Rc::new(std::cell::RefCell::new(None::<DurableRpc>));
    let owner_ready = owner.clone();
    let resume_existing_session = args.continue_last_session || args.session.is_some();
    let ready = async move {
        let (rpc, events) = DurableRpc::spawn(&host, &cwd, options).await?;
        *owner_ready.borrow_mut() = Some(rpc.clone());
        let boot = rpc
            .request("hello", json!({}))
            .await
            .context("Durable startup failed (use --no-durable to bypass the saved setting)")?;
        if boot["protocol"] != "grok-pi-durable/1" || boot["sdkVersion"] != "1.1.0" {
            bail!("Incompatible Durable host protocol/SDK");
        }
        let (client, mut agent) = acp_channels();
        let adapter = Rc::new(DurableAgent::new(rpc, agent.tx.clone(), boot));
        let models = adapter.models();
        let session_id = adapter.session_id();
        let notifications = adapter.clone();
        tokio::task::spawn_local(async move {
            notifications.run_events(events).await;
        });
        tokio::task::spawn_local(async move {
            while let Some(message) = agent.rx.recv().await {
                message.route_to_agent(adapter.clone(), |future| {
                    tokio::task::spawn_local(future);
                });
            }
        });
        let commands = vec![
            agent_client_protocol::AvailableCommand::new(
                "tasks",
                "Inspect the official Durable task graph",
            ),
            agent_client_protocol::AvailableCommand::new(
                "durable-task",
                "Inspect one Durable task",
            ),
            agent_client_protocol::AvailableCommand::new(
                "durable-recover",
                "Continue or abort interrupted work after inspecting its effects",
            ),
        ];
        let connection = AcpConnection::external(
            client.tx,
            client.rx,
            models,
            commands,
            CancellationToken::new(),
            ExternalUiProfile {
                agent_name: "Pi Durable".into(),
                builtin_commands: COMMANDS.iter().map(|v| (*v).to_string()).collect(),
                logo: Some(ExternalLogoArt {
                    full: PI_LOGO,
                    small: PI_LOGO,
                }),
                welcome_brand: Some(ExternalWelcomeBrand {
                    title: "grok-pi",
                    subtitle: "Pi Durable · persistent tasks and recovery",
                    version: GROK_PI_VERSION,
                }),
                hide_new_worktree: true,
                changelog_url: Some(
                    "https://github.com/kyrosle/grok-pi-tui/blob/main/CHANGELOG.MD",
                ),
                host_features: HostFeatureManifest::default(),
            },
        );
        Ok(ExternalRunReady {
            connection,
            session_id,
            session_title: Some("Pi Durable".into()),
            resume_existing_session,
            emit_resume_hint: true,
            resume_session_dir: None,
        })
    };
    let result = run_external_deferred(start, ready).await;
    let rpc = owner.borrow_mut().take();
    if let Some(rpc) = rpc {
        rpc.stop_ui();
        let _ = rpc.request("close", json!({})).await;
    }
    result
}
