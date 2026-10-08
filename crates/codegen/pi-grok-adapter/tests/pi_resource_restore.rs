//! Actual installed Pi resource admission; all declarations/history stay in a temp tree.
use agent_client_protocol::{self as acp, Agent};
use pi_grok_adapter::{PiAgent, PiBootstrap, PiRpc, SpawnConfig};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
use xai_acp_lib::{AcpClientMessage, acp_channels};

const INSPECTOR: &str = r#"
import {appendFileSync,writeFileSync} from 'node:fs';
import {SessionManager} from '@earendil-works/pi-coding-agent';
export default function(pi) {
 pi.on('session_shutdown',()=>appendFileSync(process.env.PI_RESOURCE_TRACE,'closed:'+process.pid+'\n'));
 pi.registerCommand('fixture-resource-inspect',{handler:async(args,ctx)=>{
  writeFileSync(JSON.parse(args).responsePath,JSON.stringify({pid:process.pid,sessionId:ctx.sessionManager.getSessionId(),leafId:ctx.sessionManager.getLeafId()}));
 }});
 pi.registerCommand('fixture-create-user-leaf',{handler:async(args,ctx)=>{
  const {responsePath,sessionDir}=JSON.parse(args);
  const manager=SessionManager.create(ctx.cwd,sessionDir);
  manager.appendModelChange(ctx.model.provider,ctx.model.id);
  manager.appendThinkingLevelChange('off');
  manager.appendMessage({role:'user',content:'fixture pending user',timestamp:Date.now()});
  writeFileSync(responsePath,JSON.stringify({sessionFile:manager.getSessionFile(),sessionId:manager.getSessionId(),leafId:manager.getLeafId()}));
 }});
}
"#;

struct Fixture {
    directory: tempfile::TempDir,
    agent: Rc<PiAgent>,
    rpc: PiRpc,
    environment: Vec<(String, String)>,
    cli: String,
    package: PathBuf,
    sink: tokio::task::JoinHandle<()>,
    events: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn start(memory: bool) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap()
            .to_owned();
        let bridge_source = std::fs::read_to_string(
            repository.join("crates/codegen/xai-grok-pager-bin/src/bin/grok_pi/tree_bridge.rs"),
        )
        .unwrap();
        let bridge = directory.path().join("bridge.ts");
        std::fs::write(
            &bridge,
            bridge_source
                .split_once("r#\"")
                .unwrap()
                .1
                .split_once("\"#;")
                .unwrap()
                .0,
        )
        .unwrap();
        let provider = directory.path().join("provider.ts");
        let source = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pi_lifecycle.ts"),
        )
        .unwrap();
        std::fs::write(
            &provider,
            source.replace("reasoning: false", "reasoning: true"),
        )
        .unwrap();
        let inspector = directory.path().join("inspector.ts");
        std::fs::write(&inspector, INSPECTOR).unwrap();
        let package = directory.path().join("local-package");
        std::fs::create_dir(&package).unwrap();
        std::fs::write(
            package.join("package.json"),
            json!({"name":"pi-resource-fresh","version":"1.0.0","pi":{"extensions":["index.ts"]}})
                .to_string(),
        )
        .unwrap();
        std::fs::write(package.join("index.ts"),r#"import {appendFileSync} from 'node:fs'; export default function(pi){appendFileSync(process.env.PI_RESOURCE_TRACE,'factory:'+process.pid+'\n');pi.registerCommand('fixture-new-resource',{handler:async()=>{}});}"#).unwrap();
        let mut arguments: Vec<String> = [
            "--mode",
            "rpc",
            "--offline",
            "--no-approve",
            "-ne",
            "-ns",
            "-np",
            "-nc",
            "--no-themes",
            "--model",
            "pi-lifecycle/local",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        if memory {
            arguments.push("--no-session".into());
        } else {
            arguments.extend([
                "--session-dir".into(),
                directory.path().join("sessions").display().to_string(),
            ]);
        }
        for path in [&bridge, &provider, &inspector] {
            arguments.extend(["--extension".into(), path.display().to_string()]);
        }
        let base = arguments.clone();
        let environment: Vec<(String, String)> = vec![
            (
                "PI_CODING_AGENT_DIR".into(),
                directory.path().join("pi").display().to_string(),
            ),
            ("PI_OFFLINE".into(), "1".into()),
            ("PI_TELEMETRY".into(), "0".into()),
            (
                "PI_RESOURCE_TRACE".into(),
                directory.path().join("scope.log").display().to_string(),
            ),
        ]
        .into_iter()
        .chain(std::env::vars().filter_map(|(key, _)| {
            (key.ends_with("_API_KEY") || key.ends_with("_TOKEN") || key.starts_with("AWS_"))
                .then_some((key, String::new()))
        }))
        .collect();
        let cli = std::env::var("PI_BIN").unwrap_or_else(|_| "pi".into());
        let process = PiRpc::spawn(SpawnConfig {
            program: cli.clone(),
            prefix_args: vec![],
            cwd: directory.path().into(),
            pi_args: arguments,
            env: environment.clone(),
        })
        .await
        .unwrap();
        let rpc = process.rpc.clone();
        let bootstrap = PiBootstrap::load(&rpc).await.unwrap();
        let (mut client, channel) = acp_channels();
        let planner = Rc::new(move |snapshot: &Value| -> anyhow::Result<Vec<String>> {
            let mut arguments = base.clone();
            for resource in snapshot["resolvedPaths"]["extensions"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("resolvedPaths missing"))?
            {
                if resource["enabled"] == true {
                    arguments.extend([
                        "--extension".into(),
                        resource["path"].as_str().unwrap().to_owned(),
                    ]);
                }
            }
            Ok(arguments)
        });
        let agent = Rc::new(
            PiAgent::new(
                rpc.clone(),
                channel.tx,
                bootstrap,
                directory.path().join("sessions"),
                None,
                None,
                None,
                None,
                None,
                false,
            )
            .unwrap()
            .with_resource_admission_planner(planner),
        );
        let sink = tokio::task::spawn_local(async move {
            while let Some(message) = client.rx.recv().await {
                match message {
                    AcpClientMessage::SessionNotification(args) => {
                        let _ = args.response_tx.send(Ok(()));
                    }
                    AcpClientMessage::ExtNotification(args) => {
                        let _ = args.response_tx.send(Ok(()));
                    }
                    other => panic!("unexpected request: {other:?}"),
                }
            }
        });
        let event_agent = agent.clone();
        let events = tokio::task::spawn_local(async move {
            event_agent.run_events(process.events).await;
        });
        Self {
            directory,
            agent,
            rpc,
            environment,
            cli,
            package,
            sink,
            events,
        }
    }
    async fn prompt(&self, text: &str) {
        let state = self.rpc.request(json!({"type":"get_state"})).await.unwrap();
        tokio::time::timeout(
            Duration::from_secs(10),
            self.agent.prompt(acp::PromptRequest::new(
                state["sessionId"].as_str().unwrap().to_owned(),
                vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
            )),
        )
        .await
        .unwrap()
        .unwrap();
    }
    async fn inspect(&self) -> Value {
        let path = self.directory.path().join("inspect.json");
        self.rpc.request(json!({"type":"prompt","message":format!("/fixture-resource-inspect {}",json!({"responsePath":path}))})).await.unwrap();
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }
    async fn install(&self) {
        let output = tokio::process::Command::new(&self.cli)
            .args(["install", self.package.to_str().unwrap(), "--no-approve"])
            .current_dir(self.directory.path())
            .envs(self.environment.iter().cloned())
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "official local install failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    async fn package_action(&self, operation: &str, source: &str) -> Value {
        let params = json!({"operation":operation,"scope":"user","source":source});
        let response = self
            .agent
            .ext_method(acp::ExtRequest::new(
                "pi/packages/action",
                serde_json::value::to_raw_value(&params).unwrap().into(),
            ))
            .await
            .unwrap();
        serde_json::from_str::<Value>(response.0.get()).unwrap()["result"].clone()
    }
    async fn user_leaf(&self) {
        let response_path = self.directory.path().join("user-leaf.json");
        let parameters = json!({"responsePath":response_path,"sessionDir":self.directory.path().join("sessions")});
        self.rpc.request(json!({"type":"prompt","message":format!("/fixture-create-user-leaf {parameters}")})).await.unwrap();
        let created: Value =
            serde_json::from_slice(&std::fs::read(response_path).unwrap()).unwrap();
        let result = self
            .agent
            .switch_session(
                Path::new(created["sessionFile"].as_str().unwrap()),
                created["sessionId"].as_str().unwrap(),
            )
            .await
            .unwrap();
        assert!(!result.cancelled);
        let current = self
            .rpc
            .request(json!({"type":"get_entries"}))
            .await
            .unwrap();
        assert_eq!(current["leafId"], created["leafId"]);
        let leaf = current["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == current["leafId"])
            .unwrap();
        assert_eq!(
            leaf["message"]["role"], "user",
            "public SessionManager must produce a genuine active user leaf"
        );
    }
    async fn reload(&self) -> Result<Value, acp::Error> {
        let response = self
            .agent
            .ext_method(acp::ExtRequest::new(
                "pi/session/reload",
                serde_json::value::to_raw_value(&json!({})).unwrap().into(),
            ))
            .await?;
        let body: Value = serde_json::from_str(response.0.get()).unwrap();
        Ok(body.get("result").unwrap_or(&body).clone())
    }
    async fn close(self) {
        self.rpc.kill().await;
        self.events.abort();
        self.sink.abort();
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires installed Pi 1.0; isolated synthetic provider and local package, no network"]
async fn actual_resource_admission_restores_persistent_session_leaf_and_selected_model() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let fixture = Fixture::start(false).await;
            fixture
                .rpc
                .request(json!({"type":"set_thinking_level","level":"low"}))
                .await
                .unwrap();
            fixture.prompt("fixture-model-run first").await;
            fixture.prompt("fixture-model-run second").await;
            let entries = fixture
                .rpc
                .request(json!({"type":"get_entries"}))
                .await
                .unwrap();
            let target = entries["entries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["message"]["role"] == "assistant")
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned();
            fixture
                .rpc
                .request(json!({"type":"prompt","message":format!("/__pi_navigate_tree {target}")}))
                .await
                .unwrap();
            let before = fixture
                .rpc
                .request(json!({"type":"get_state"}))
                .await
                .unwrap();
            let identity = fixture.inspect().await;
            assert_eq!(identity["leafId"], target);
            assert!(Path::new(before["sessionFile"].as_str().unwrap()).is_file());
            assert_ne!(
                fixture.reload().await.unwrap()["restarted"],
                true,
                "ordinary edit reload keeps the same child"
            );
            assert_eq!(fixture.inspect().await["pid"], identity["pid"]);
            std::fs::write(fixture.directory.path().join("scope.log"), "").unwrap();
            fixture.install().await;
            let result = fixture.reload().await.unwrap();
            assert_eq!(result["restarted"], true);
            let after = fixture
                .rpc
                .request(json!({"type":"get_state"}))
                .await
                .unwrap();
            let restored = fixture.inspect().await;
            assert_ne!(restored["pid"], identity["pid"]);
            assert_eq!(restored["leafId"], target);
            for key in ["sessionId", "sessionFile", "thinkingLevel"] {
                assert_eq!(after[key], before[key], "preserve {key}");
            }
            assert_eq!(after["model"]["provider"], before["model"]["provider"]);
            assert_eq!(after["model"]["id"], before["model"]["id"]);
            let commands = fixture
                .rpc
                .request(json!({"type":"get_commands"}))
                .await
                .unwrap();
            assert!(
                commands["commands"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|command| command["name"] == "fixture-new-resource")
            );
            let log = std::fs::read_to_string(fixture.directory.path().join("scope.log")).unwrap();
            let closed = format!("closed:{}", identity["pid"].as_u64().unwrap());
            let factory = format!("factory:{}", restored["pid"].as_u64().unwrap());
            assert!(
                log.find(&closed).unwrap() < log.find(&factory).unwrap(),
                "old scope must close before the new factory runs: {log}"
            );
            fixture.close().await;
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires installed Pi 1.0; in-memory/user-leaf boundary remains isolated"]
async fn actual_resource_admission_defers_memory_history_and_user_message_leaf() {
    tokio::task::LocalSet::new()
        .run_until(async {
            for memory in [true, false] {
                let fixture = Fixture::start(memory).await;
                fixture.prompt("fixture-model-run").await;
                if !memory {
                    fixture.user_leaf().await;
                }
                let before = fixture
                    .rpc
                    .request(json!({"type":"get_messages"}))
                    .await
                    .unwrap();
                let identity = fixture.inspect().await;
                fixture.install().await;
                let error = fixture.reload().await.unwrap_err();
                let detail = serde_json::to_string(&error).unwrap();
                assert!(
                    detail.contains(if memory {
                        "in-memory"
                    } else {
                        "user-message leaf"
                    }),
                    "specific defer boundary: {detail}"
                );
                assert_eq!(fixture.inspect().await, identity);
                assert_eq!(
                    fixture
                        .rpc
                        .request(json!({"type":"get_messages"}))
                        .await
                        .unwrap(),
                    before
                );
                fixture.close().await;
            }
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires installed Pi 1.0; official local install/remove through production adapter"]
async fn actual_local_package_action_resolves_declared_scope_and_preserves_session() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let fixture = Fixture::start(false).await;
            fixture.prompt("fixture-model-run").await;
            let before = fixture
                .rpc
                .request(json!({"type":"get_state"}))
                .await
                .unwrap();
            let leaf = fixture.inspect().await["leafId"].clone();
            let installed = fixture
                .package_action("install", fixture.package.to_str().unwrap())
                .await;
            assert_eq!(installed["status"], "registry_refreshed", "{installed}");
            assert!(
                installed["snapshot"]["commands"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|command| command["name"] == "fixture-new-resource")
            );
            let source = installed["snapshot"]["packages"]
                .as_array()
                .unwrap()
                .iter()
                .find(|package| package["scope"] == "user")
                .unwrap()["source"]
                .as_str()
                .unwrap()
                .to_owned();
            assert_eq!(
                source, "../local-package",
                "fixture must exercise agentDir-relative declarations"
            );
            let removed = fixture.package_action("remove", &source).await;
            assert_eq!(removed["status"], "registry_refreshed", "{removed}");
            assert!(
                removed["snapshot"]["packages"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(
                !removed["snapshot"]["commands"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|command| command["name"] == "fixture-new-resource")
            );
            assert!(
                fixture.package.join("index.ts").is_file(),
                "local removal must retain user sources"
            );
            let after = fixture
                .rpc
                .request(json!({"type":"get_state"}))
                .await
                .unwrap();
            for key in ["sessionId", "sessionFile", "thinkingLevel", "model"] {
                assert_eq!(after[key], before[key], "preserve {key}");
            }
            assert_eq!(fixture.inspect().await["leafId"], leaf);
            fixture.close().await;
        })
        .await;
}
