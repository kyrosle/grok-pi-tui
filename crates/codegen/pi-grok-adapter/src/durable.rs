//! Official Durable host → ACP. Execution, inboxes and storage stay in Pi.
use agent_client_protocol as acp;
use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::Path,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{mpsc, oneshot},
};
use xai_acp_lib::{AcpClientMessage, acp_send};

type Pending = Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>;

#[derive(Clone)]
pub struct DurableRpc {
    writer: mpsc::UnboundedSender<Value>,
    pending: Pending,
    next: Arc<AtomicU64>,
    closed: Arc<AtomicBool>,
    ui_closed: tokio_util::sync::CancellationToken,
}

impl DurableRpc {
    #[cfg(unix)]
    async fn background(
        host: &Path,
        cwd: &Path,
        mut options: Value,
    ) -> Result<(Self, mpsc::Receiver<Value>)> {
        let mut locate = options.clone();
        locate["locate"] = json!(true);
        let output = tokio::time::timeout(
            Duration::from_secs(20),
            Command::new("node")
                .arg(host)
                .arg(locate.to_string())
                .current_dir(cwd)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .context("Durable store lookup timed out")??;
        if !output.status.success() {
            bail!(
                "Durable store lookup failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let location: Value =
            serde_json::from_slice(&output.stdout).context("Durable store lookup protocol")?;
        let socket = location["socket"].as_str().context("Durable socket path")?;
        if let Ok(stream) = tokio::net::UnixStream::connect(socket).await {
            let (reader, writer) = stream.into_split();
            return Ok(Self::io(writer, reader));
        }
        options["storeId"] = location["storeId"].clone();
        let log_path = Path::new(
            location["directory"]
                .as_str()
                .context("Durable directory")?,
        )
        .join("owner.log");
        use std::os::unix::fs::OpenOptionsExt;
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(log_path)?;
        let mut child = Command::new("node")
            .arg(host)
            .arg(options.to_string())
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .process_group(0)
            .spawn()
            .context("Durable background owner")?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(stream) = tokio::net::UnixStream::connect(socket).await {
                let (reader, writer) = stream.into_split();
                tokio::spawn(async move {
                    let _ = child.wait().await;
                });
                return Ok(Self::io(writer, reader));
            }
            if let Some(status) = child.try_wait()? {
                bail!(
                    "Durable owner exited ({status}); inspect the store's owner.log. No fallback was started."
                );
            }
            if tokio::time::Instant::now() > deadline {
                let _ = child.kill().await;
                bail!("Durable owner startup timed out");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    #[cfg(not(unix))]
    async fn background(_: &Path, _: &Path, _: Value) -> Result<(Self, mpsc::Receiver<Value>)> {
        bail!("Background Durable owner currently requires Unix");
    }
    pub async fn spawn(
        host: &Path,
        cwd: &Path,
        options: Value,
    ) -> Result<(Self, mpsc::Receiver<Value>)> {
        if options["backgroundOwner"] == true {
            return Self::background(host, cwd, options).await;
        }
        let mut child = Command::new("node")
            .arg(host)
            .arg(options.to_string())
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context(
                "failed to start Durable host; Node >=22.19 and the packaged host are required",
            )?;
        let stdin = child.stdin.take().context("Durable stdin")?;
        let stdout = child.stdout.take().context("Durable stdout")?;
        let stderr = child.stderr.take().context("Durable stderr")?;
        let result = Self::io(stdin, stdout);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!(%line, "Durable host");
            }
        });
        tokio::spawn(async move {
            let _ = child.wait().await;
        });
        Ok(result)
    }

    fn io(
        mut stdin: impl tokio::io::AsyncWrite + Unpin + Send + 'static,
        stdout: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    ) -> (Self, mpsc::Receiver<Value>) {
        let (writer, mut writes) = mpsc::unbounded_channel::<Value>();
        let (events_tx, events) = mpsc::channel(100);
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        tokio::spawn(async move {
            while let Some(value) = writes.recv().await {
                let mut bytes = value.to_string().into_bytes();
                bytes.push(b'\n');
                if stdin.write_all(&bytes).await.is_err() {
                    break;
                }
            }
        });
        let closed = Arc::new(AtomicBool::new(false));
        let reader_closed = closed.clone();
        let read_pending = pending.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.len() > 16 * 1024 * 1024 {
                    break;
                }
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    break;
                };
                if value["kind"] == "fatal" {
                    let message = value["message"]
                        .as_str()
                        .unwrap_or("Durable startup failed")
                        .to_owned();
                    for (_, sender) in read_pending.lock().unwrap().drain() {
                        let _ = sender.send(Err(message.clone()));
                    }
                    break;
                }
                if value["kind"] == "response" {
                    let id = value["id"].as_str().unwrap_or_default();
                    if let Some(sender) = read_pending.lock().unwrap().remove(id) {
                        let result = value
                            .get("error")
                            .and_then(Value::as_str)
                            .map(|e| Err(e.to_owned()))
                            .unwrap_or_else(|| Ok(value["result"].clone()));
                        let _ = sender.send(result);
                    }
                } else if value["kind"] == "event" && events_tx.send(value).await.is_err() {
                    break;
                }
            }
            reader_closed.store(true, Ordering::Release);
            for (_, sender) in read_pending.lock().unwrap().drain() {
                let _ = sender.send(Err("Durable host disconnected; work may be recoverable. No Pi RPC fallback was started.".into()));
            }
        });
        (
            Self {
                writer,
                pending,
                next: Arc::new(AtomicU64::new(1)),
                closed,
                ui_closed: tokio_util::sync::CancellationToken::new(),
            },
            events,
        )
    }

    /// Keep RPC alive for pause/detach after Pager stops accepting UI receipts.
    pub fn stop_ui(&self) {
        self.ui_closed.cancel();
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value> {
        if self.closed.load(Ordering::Acquire) {
            bail!("Durable transport is disconnected");
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed).to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id.clone(), tx);
        if self
            .writer
            .send(json!({"id": id, "method": method, "params": params}))
            .is_err()
        {
            self.pending.lock().unwrap().remove(&id);
            bail!("Durable transport closed");
        }
        let result = if matches!(method, "wait" | "compact" | "abort") {
            rx.await.map_err(|_| anyhow!("Durable response lost"))?
        } else {
            match tokio::time::timeout(Duration::from_secs(30), rx).await {
                Ok(value) => value.map_err(|_| anyhow!("Durable response lost"))?,
                Err(_) => {
                    self.pending.lock().unwrap().remove(&id);
                    bail!("Durable {method} timed out; query status before retrying a submission");
                }
            }
        };
        result.map_err(anyhow::Error::msg)
    }
}

struct State {
    boot: Value,
    watching: bool,
    entries: std::collections::HashSet<String>,
    partial: HashMap<usize, String>,
    outputs: HashMap<String, String>,
    prompt_id: Option<String>,
    epoch: String,
    sequence: u64,
    settled: std::collections::HashSet<String>,
    waiters: HashMap<String, oneshot::Sender<()>>,
}

pub struct DurableAgent {
    rpc: DurableRpc,
    client_tx: mpsc::UnboundedSender<AcpClientMessage>,
    state: RefCell<State>,
}

fn id(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
fn error(value: impl std::fmt::Display) -> acp::Error {
    acp::Error::internal_error().data(value.to_string())
}
fn response(value: Value) -> Result<acp::ExtResponse, acp::Error> {
    Ok(acp::ExtResponse::new(
        serde_json::value::to_raw_value(&value)
            .map_err(error)?
            .into(),
    ))
}

impl DurableAgent {
    pub fn new(
        rpc: DurableRpc,
        client_tx: mpsc::UnboundedSender<AcpClientMessage>,
        boot: Value,
    ) -> Self {
        Self {
            rpc,
            client_tx,
            state: RefCell::new(State {
                boot,
                watching: false,
                entries: Default::default(),
                partial: Default::default(),
                outputs: Default::default(),
                prompt_id: None,
                epoch: String::new(),
                sequence: 0,
                settled: Default::default(),
                waiters: Default::default(),
            }),
        }
    }
    pub fn session_id(&self) -> String {
        self.state.borrow().boot["session"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }
    pub fn models(&self) -> Option<acp::SessionModelState> {
        let state = self.state.borrow();
        let models = crate::model::parse_models(&state.boot);
        let agent = &state.boot["snapshot"]["agent"];
        let current = models.iter().find(|m| {
            m.provider == agent["model"]["provider"].as_str().unwrap_or_default()
                && m.id == agent["model"]["modelId"].as_str().unwrap_or_default()
        });
        let (catalog, selected) = crate::pi_adapter::build_model_catalog(
            &models,
            current,
            agent["thinkingLevel"].as_str().unwrap_or("off"),
        );
        Some(acp::SessionModelState::new(
            selected.or_else(|| catalog.first().map(|(key, _)| key.clone()))?,
            catalog.into_values().collect(),
        ))
    }
    async fn ext(&self, method: &str, value: Value) {
        if let Ok(raw) = serde_json::value::to_raw_value(&value) {
            tokio::select! {
                _ = self.rpc.ui_closed.cancelled() => {},
                _ = acp_send(acp::ExtNotification::new(method, raw.into()), &self.client_tx) => {},
            }
        }
    }
    async fn update(&self, update: acp::SessionUpdate, replay: bool) {
        let timestamp_ms = match &update {
            acp::SessionUpdate::ToolCallUpdate(tool) => tool
                .fields
                .raw_output
                .as_ref()
                .and_then(|raw| raw.get("timestamp"))
                .and_then(Value::as_i64),
            _ => None,
        };
        self.update_at(update, replay, timestamp_ms).await;
    }
    async fn update_at(&self, update: acp::SessionUpdate, replay: bool, timestamp_ms: Option<i64>) {
        let mut meta = acp::Meta::new();
        if replay {
            meta.insert("isReplay".into(), json!(true));
        }
        if let Some(ms) = timestamp_ms.or_else(|| (!replay).then(crate::pi_adapter::utc_now_ms)) {
            meta.insert("agentTimestampMs".into(), json!(ms));
        }
        if let Some(prompt) = &self.state.borrow().prompt_id {
            meta.insert("promptId".into(), json!(prompt));
        }
        let notification =
            acp::SessionNotification::new(self.session_id(), update).meta(Some(meta));
        tokio::select! {
            _ = self.rpc.ui_closed.cancelled() => {},
            _ = acp_send(notification, &self.client_tx) => {},
        }
    }
    async fn ensure_watch(&self) -> Result<()> {
        if self.state.borrow().watching {
            return Ok(());
        }
        self.state.borrow_mut().watching = true;
        if let Err(err) = self.rpc.request("watch", json!({})).await {
            self.state.borrow_mut().watching = false;
            return Err(err);
        }
        if self.state.borrow().boot["recovery"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
        {
            self.ext("pi/ui/status", json!({"key":"durable-recovery","text":"Durable paused: inspect interrupted tools, then /durable-recover continue or /durable-recover abort"})).await;
        }
        Ok(())
    }
    async fn tool_start(
        &self,
        call_id: &str,
        name: &str,
        args: Value,
        replay: bool,
        timestamp_ms: Option<i64>,
    ) {
        let diff = crate::tool_projection::edit_diff_content(name, Some(&args), None);
        let mut call = acp::ToolCall::new(call_id.to_owned(), name.to_owned())
            .kind(crate::tool_projection::tool_kind(name))
            .status(acp::ToolCallStatus::InProgress)
            .raw_input(crate::tool_projection::normalize_tool_raw_input(
                name,
                Some(args),
            ));
        if let Some(content) = diff {
            call = call.content(content);
        }
        self.update_at(acp::SessionUpdate::ToolCall(call), replay, timestamp_ms)
            .await;
    }
    async fn message(&self, message: &Value, replay: bool) {
        let role = message["role"].as_str().unwrap_or_default();
        if !replay && role == "assistant" {
            self.ext("pi/ui/program_status", json!({"sessionId":self.session_id(),"state":if message["stopReason"] == "error" {"error"} else {"working"}})).await;
        }
        if let Some(text) = message["content"].as_str() {
            self.text(role, text, false, replay).await;
            return;
        }
        if let Some(blocks) = message["content"].as_array() {
            for block in blocks {
                match block["type"].as_str().unwrap_or_default() {
                    "text" => {
                        self.text(
                            role,
                            block["text"].as_str().unwrap_or_default(),
                            false,
                            replay,
                        )
                        .await
                    }
                    "thinking" => {
                        self.text(
                            role,
                            block["thinking"].as_str().unwrap_or_default(),
                            true,
                            replay,
                        )
                        .await
                    }
                    "toolCall" => {
                        self.tool_start(
                            &id(&block["id"]),
                            block["name"].as_str().unwrap_or("tool"),
                            block["arguments"].clone(),
                            replay,
                            message["timestamp"].as_i64(),
                        )
                        .await
                    }
                    _ => {}
                }
            }
        }
    }
    async fn text(&self, role: &str, text: &str, thinking: bool, replay: bool) {
        if text.is_empty() {
            return;
        }
        let chunk = acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(text)));
        let update = if role == "user" {
            acp::SessionUpdate::UserMessageChunk(chunk)
        } else if thinking {
            acp::SessionUpdate::AgentThoughtChunk(chunk)
        } else {
            acp::SessionUpdate::AgentMessageChunk(chunk)
        };
        self.update(update, replay).await;
    }
    async fn entry(&self, entry: &Value, replay: bool) {
        let key = id(&entry["id"]);
        if !self.state.borrow_mut().entries.insert(key) && entry["kind"] != "pi.tool-result" {
            return;
        }
        if let Some(messages) = entry["model"].as_array() {
            for message in messages {
                if message["role"] == "toolResult" {
                    self.update(
                        acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                            id(&message["toolCallId"]),
                            acp::ToolCallUpdateFields::new()
                                .status(if message["isError"] == true {
                                    acp::ToolCallStatus::Failed
                                } else {
                                    acp::ToolCallStatus::Completed
                                })
                                .content(crate::tool_projection::tool_content(message))
                                .raw_output(message.clone()),
                        )),
                        replay,
                    )
                    .await;
                } else {
                    self.message(message, replay).await;
                }
            }
        }
    }
    async fn publish_boot(&self, value: Value) {
        self.state.borrow_mut().boot = value;
        if let Some(models) = self.models() {
            self.ext(
                "x.ai/models/update",
                serde_json::to_value(models).unwrap_or_default(),
            )
            .await;
        }
    }
    fn reset_view(&self) {
        let mut state = self.state.borrow_mut();
        state.watching = false;
        state.entries.clear();
        state.partial.clear();
        state.outputs.clear();
        state.prompt_id = None;
        state.settled.clear();
        state.waiters.clear();
    }
    pub async fn run_events(&self, mut events: mpsc::Receiver<Value>) {
        while let Some(frame) = events.recv().await {
            let epoch = frame["epoch"].as_str().unwrap_or_default();
            let seq = frame["sequence"].as_u64().unwrap_or(0);
            {
                let mut state = self.state.borrow_mut();
                if state.epoch == epoch && seq <= state.sequence {
                    continue;
                }
                state.epoch = epoch.into();
                state.sequence = seq;
            }
            let event = &frame["event"];
            if frame["session"].as_str() != Some(self.session_id().as_str()) {
                continue;
            }
            match event["type"].as_str().unwrap_or_default() {
                "settled" => {
                    let key = id(&event["id"]);
                    let mut state = self.state.borrow_mut();
                    state.settled.insert(key.clone());
                    if let Some(sender) = state.waiters.remove(&key) {
                        let _ = sender.send(());
                    }
                }
                "run_start" => {
                    if let Some(prompt) = event["promptIds"][0].as_str() {
                        self.state.borrow_mut().prompt_id = Some(prompt.to_owned());
                        self.ext("x.ai/queue/changed", json!({"sessionId":self.session_id(),"entries":[],"runningPromptId":prompt,"runningKind":"prompt"})).await;
                    }
                }
                "run_end" => {
                    self.state.borrow_mut().prompt_id = None;
                }
                "snapshot" => {
                    self.state.borrow_mut().boot["snapshot"] = event.clone();
                    if let Some(entries) = event["entries"].as_array() {
                        for entry in entries {
                            self.entry(entry, true).await;
                        }
                    }
                    if let Some(blocks) = event["generation"]["message"]["content"].as_array() {
                        for (index, block) in blocks.iter().enumerate() {
                            self.partial_block(index, block).await;
                        }
                    }
                    if let Some(tools) = event["tools"].as_array() {
                        for tool in tools {
                            self.tool_start(
                                &id(&tool["callId"]),
                                tool["name"].as_str().unwrap_or("tool"),
                                tool["arguments"].clone(),
                                true,
                                None,
                            )
                            .await;
                        }
                    }
                }
                "message_start" => {
                    self.state.borrow_mut().partial.clear();
                }
                "message_update" => {
                    if let Some(changes) = event["changes"].as_array() {
                        for change in changes {
                            let index = change["contentIndex"].as_u64().unwrap_or(0) as usize;
                            match change["type"].as_str().unwrap_or_default() {
                                "text_delta" | "thinking_delta" => {
                                    let delta = change["delta"].as_str().unwrap_or_default();
                                    self.state
                                        .borrow_mut()
                                        .partial
                                        .entry(index)
                                        .or_default()
                                        .push_str(delta);
                                    self.text(
                                        "assistant",
                                        delta,
                                        change["type"] == "thinking_delta",
                                        false,
                                    )
                                    .await;
                                }
                                "block" | "text_start" | "thinking_start" => {
                                    self.partial_block(index, &change["block"]).await
                                }
                                "message" => {
                                    if let Some(blocks) = change["message"]["content"].as_array() {
                                        for (i, block) in blocks.iter().enumerate() {
                                            self.partial_block(i, block).await;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "message_end" => {
                    if event["entry"]["kind"] == "pi.assistant" {
                        self.ext("pi/ui/program_status", json!({"sessionId":self.session_id(),"state":if event["entry"]["model"][0]["stopReason"] == "error" {"error"} else {"working"}})).await;
                        if let Some(blocks) = event["entry"]["model"][0]["content"].as_array() {
                            for (index, block) in blocks.iter().enumerate() {
                                self.partial_block(index, block).await;
                            }
                        }
                        self.state
                            .borrow_mut()
                            .entries
                            .insert(id(&event["entry"]["id"]));
                    } else if event["entry"]["kind"] == "pi.tool-result" {
                        self.entry(&event["entry"], false).await;
                    }
                }
                "entry_appended" => {
                    self.entry(&event["entry"], false).await;
                }
                "tool_execution_start" => {
                    self.tool_start(
                        &id(&event["toolCallId"]),
                        event["toolName"].as_str().unwrap_or("tool"),
                        event["args"].clone(),
                        false,
                        None,
                    )
                    .await
                }
                "tool_execution_update" => {
                    let key = id(&event["toolCallId"]);
                    let output = apply_output(
                        self.state
                            .borrow_mut()
                            .outputs
                            .entry(key.clone())
                            .or_default(),
                        &event["output"],
                    );
                    self.update(
                        acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                            key,
                            acp::ToolCallUpdateFields::new().content(
                                crate::tool_projection::tool_content(
                                    &json!({"content":[{"type":"text","text":output}]}),
                                ),
                            ),
                        )),
                        false,
                    )
                    .await;
                }
                "tool_execution_end" => {
                    if event["entry"].is_object() {
                        self.entry(&event["entry"], false).await;
                    } else {
                        self.update(
                            acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                                id(&event["toolCallId"]),
                                acp::ToolCallUpdateFields::new()
                                    .status(acp::ToolCallStatus::Failed),
                            )),
                            false,
                        )
                        .await;
                    }
                }
                "view_state" => {
                    self.state.borrow_mut().boot["snapshot"]["agent"] =
                        event["docs"]["pi.agent"].clone();
                    let entries = event["inbox"].as_array().map(|items| items.iter().filter(|item| item["mode"] != "write").enumerate().map(|(position,item)| json!({"id":item["requestId"].as_str().map(str::to_owned).unwrap_or_else(|| id(&item["id"])),"version":1,"kind":"prompt","text":item["content"],"position":position})).collect::<Vec<_>>()).unwrap_or_default();
                    self.ext("x.ai/queue/changed", json!({"sessionId":self.session_id(),"entries":entries,"runningPromptId":self.state.borrow().prompt_id})).await;
                    self.ext(
                        "pi/ui/durable/state",
                        json!({"sessionId":self.session_id(),"docs":event["docs"]}),
                    )
                    .await;
                }
                "task_graph" => {
                    self.ext(
                        "pi/ui/durable/tasks",
                        json!({"sessionId":self.session_id(),"graph":event["graph"]}),
                    )
                    .await
                }
                "agent_changed" => {
                    self.state.borrow_mut().boot["snapshot"]["agent"] = event["agent"].clone();
                    if let Some(models) = self.models() {
                        self.ext(
                            "x.ai/models/update",
                            serde_json::to_value(models).unwrap_or_default(),
                        )
                        .await;
                    }
                }
                "task_failed" => {
                    self.ext(
                        "pi/ui/notify",
                        json!({"type":"warning","message":event["message"]}),
                    )
                    .await
                }
                _ => {}
            }
        }
        self.state.borrow_mut().waiters.clear();
        self.ext("pi/ui/notify", json!({"type":"error","message":"Durable host disconnected. Restart with --durable --continue to recover."})).await;
    }
    async fn partial_block(&self, index: usize, block: &Value) {
        let thinking = block["type"] == "thinking";
        let text = block[if thinking { "thinking" } else { "text" }]
            .as_str()
            .unwrap_or_default();
        let delta = {
            let mut state = self.state.borrow_mut();
            let old = state.partial.entry(index).or_default();
            let suffix = text.strip_prefix(old.as_str()).unwrap_or(text).to_owned();
            *old = text.to_owned();
            suffix
        };
        self.text("assistant", &delta, thinking, false).await;
    }
}

fn apply_output(current: &mut String, change: &Value) -> String {
    if let Some(value) = change["set"].as_str() {
        *current = value.to_owned();
    } else {
        let trim = change["trimStart"].as_u64().unwrap_or(0) as usize;
        // SDK trims JS string units. Keep Unicode boundaries when projecting UTF-8.
        let mut units = 0;
        let mut bytes = 0;
        for ch in current.chars() {
            if units >= trim {
                break;
            }
            units += ch.len_utf16();
            bytes += ch.len_utf8();
        }
        current.drain(..bytes);
        if let Some(value) = change["append"].as_str() {
            current.push_str(value);
        }
    }
    current.clone()
}

#[async_trait::async_trait(?Send)]
impl acp::Agent for DurableAgent {
    async fn initialize(
        &self,
        _: acp::InitializeRequest,
    ) -> Result<acp::InitializeResponse, acp::Error> {
        Ok(acp::InitializeResponse::new(acp::ProtocolVersion::V1)
            .agent_capabilities(acp::AgentCapabilities::new().load_session(true)))
    }
    async fn authenticate(
        &self,
        _: acp::AuthenticateRequest,
    ) -> Result<acp::AuthenticateResponse, acp::Error> {
        Err(error(
            "Configure credentials with Pi /login; Durable login UI is not yet supported",
        ))
    }
    async fn new_session(
        &self,
        _: acp::NewSessionRequest,
    ) -> Result<acp::NewSessionResponse, acp::Error> {
        let boot = self.rpc.request("new", json!({})).await.map_err(error)?;
        self.reset_view();
        self.publish_boot(boot).await;
        Ok(acp::NewSessionResponse::new(self.session_id()).models(self.models()))
    }
    async fn load_session(
        &self,
        args: acp::LoadSessionRequest,
    ) -> Result<acp::LoadSessionResponse, acp::Error> {
        let target = args.session_id.0.to_string();
        if target != self.session_id() {
            let boot = self
                .rpc
                .request("attach", json!({"session":target}))
                .await
                .map_err(error)?;
            self.reset_view();
            self.publish_boot(boot).await;
        }
        // The host defers watch until the adapter knows the new locator, so
        // its initial snapshot cannot be discarded as another session's frame.
        self.ensure_watch().await.map_err(error)?;
        Ok(acp::LoadSessionResponse::new().models(self.models()))
    }
    async fn prompt(&self, args: acp::PromptRequest) -> Result<acp::PromptResponse, acp::Error> {
        let (text, images) = crate::prompt_bridge::prompt_to_pi(&args.prompt);
        if !images.is_empty() {
            return Err(error("Images are not supported in this Durable mode"));
        }
        let prompt_id = args
            .meta
            .as_ref()
            .and_then(|m| m.get("promptId"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Some(action) = text.trim().strip_prefix("/durable-recover ") {
            self.rpc
                .request("recover", json!({"action":action.trim()}))
                .await
                .map_err(error)?;
            self.ext(
                "pi/ui/status",
                json!({"key":"durable-recovery","text":null}),
            )
            .await;
            self.ext(
                "pi/ui/notify",
                json!({"type":"info","message":"Durable recovery decision recorded."}),
            )
            .await;
            return Ok(crate::prompt_bridge::prompt_response(
                acp::StopReason::EndTurn,
                prompt_id.as_deref(),
            ));
        }
        if let Some(task_id) = text.trim().strip_prefix("/durable-task") {
            if !task_id.trim().is_empty() {
                let task = self
                    .rpc
                    .request("task", json!({"id":task_id.trim()}))
                    .await
                    .map_err(error)?;
                self.update(acp::SessionUpdate::ToolCall(acp::ToolCall::new(format!("inspect-{}",uuid::Uuid::now_v7()), "Durable task inspection").kind(acp::ToolKind::Other).status(acp::ToolCallStatus::Completed).content(crate::tool_projection::tool_content(&json!({"content":[{"type":"text","text":serde_json::to_string_pretty(&task).unwrap_or_default()}]}))).raw_output(task)), false).await;
            }
            return Ok(crate::prompt_bridge::prompt_response(
                acp::StopReason::EndTurn,
                prompt_id.as_deref(),
            ));
        }
        if text.trim() == "/tasks" {
            self.ext("pi/ui/durable/tasks", json!({"sessionId":self.session_id(),"graph":self.rpc.request("tasks", json!({})).await.map_err(error)?,"open":true})).await;
            return Ok(crate::prompt_bridge::prompt_response(
                acp::StopReason::EndTurn,
                prompt_id.as_deref(),
            ));
        }
        self.ensure_watch().await.map_err(error)?;
        if self.state.borrow().prompt_id.is_none() {
            self.state.borrow_mut().prompt_id = prompt_id.clone();
        }
        let request_id = prompt_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
        let behavior = args
            .meta
            .as_ref()
            .and_then(|m| m.get("streamingBehavior"))
            .and_then(Value::as_str)
            .unwrap_or("steer");
        self.ext(
            "pi/ui/program_status",
            json!({"sessionId":self.session_id(),"state":"working"}),
        )
        .await;
        let submission = self
            .rpc
            .request(
                "submit",
                json!({"text":text,"requestId":request_id,"whenBusy":behavior}),
            )
            .await
            .map_err(error)?;
        let key = id(&submission["id"]);
        let (tx, rx) = oneshot::channel();
        let needs_barrier = {
            let mut state = self.state.borrow_mut();
            if state.settled.contains(&key)
                || matches!(submission["status"].as_str(), Some("done" | "unanswered"))
            {
                false
            } else {
                state.waiters.insert(key.clone(), tx);
                true
            }
        };
        let settled = self
            .rpc
            .request("wait", json!({"id":submission["id"]}))
            .await
            .map_err(error)?;
        if needs_barrier {
            rx.await
                .map_err(|_| error("Durable UI event stream disconnected"))?;
        }
        self.state.borrow_mut().settled.remove(&key);
        Ok(crate::prompt_bridge::prompt_response(
            if settled["reason"] == "aborted" {
                acp::StopReason::Cancelled
            } else {
                acp::StopReason::EndTurn
            },
            prompt_id.as_deref(),
        ))
    }
    async fn cancel(&self, _: acp::CancelNotification) -> Result<(), acp::Error> {
        self.rpc.request("abort", json!({})).await.map_err(error)?;
        Ok(())
    }
    async fn set_session_mode(
        &self,
        _: acp::SetSessionModeRequest,
    ) -> Result<acp::SetSessionModeResponse, acp::Error> {
        Err(error("Plan mode is not adapted to Durable"))
    }
    async fn set_session_model(
        &self,
        args: acp::SetSessionModelRequest,
    ) -> Result<acp::SetSessionModelResponse, acp::Error> {
        let models = crate::model::parse_models(&self.state.borrow().boot);
        let model = models
            .iter()
            .find(|model| crate::pi_adapter::model_key(model) == args.model_id.0.as_ref())
            .ok_or_else(|| error("Unknown model"))?;
        let mut params = json!({"model":{"provider":model.provider,"modelId":model.id}});
        if let Some(effort) = args
            .meta
            .as_ref()
            .and_then(|m| m.get("reasoningEffort"))
            .and_then(Value::as_str)
        {
            params["thinkingLevel"] = json!(
                model
                    .pi_level_for_acp_effort(effort)
                    .ok_or_else(|| error("Unsupported thinking level"))?
            );
        }
        self.publish_boot(self.rpc.request("configure", params).await.map_err(error)?)
            .await;
        Ok(acp::SetSessionModelResponse::new())
    }
    async fn ext_method(&self, args: acp::ExtRequest) -> Result<acp::ExtResponse, acp::Error> {
        let params: Value = serde_json::from_str(args.params.get()).map_err(error)?;
        match args.method.as_ref() {
            "pi/session/list" => { let sessions = self.rpc.request("sessions", json!({})).await.map_err(error)?; self.ext("pi/ui/session_catalog", json!({"scope":"current","sessions":sessions})).await; response(json!({})) },
            "x.ai/interject" => {
                let (text, images) = if let Some(blocks) = params.get("content").cloned().and_then(|value| serde_json::from_value::<Vec<acp::ContentBlock>>(value).ok()) { crate::prompt_bridge::prompt_to_pi(&blocks) } else { (params["text"].as_str().unwrap_or_default().to_owned(), Vec::new()) };
                if !images.is_empty() { return Err(error("Images are not supported by Durable")); }
                let request_id = params.get("interjectionId").or_else(|| params.get("promptId")).and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
                response(self.rpc.request("submit", json!({"text":text,"requestId":request_id,"whenBusy":"steer"})).await.map_err(error)?)
            }
            "x.ai/compact_conversation" => response(self.rpc.request("compact", json!({"instructions":params.get("customInstructions").or_else(|| params.get("instructions"))})).await.map_err(error)?),
            "pi/durable/recover" => response(self.rpc.request("recover", params).await.map_err(error)?),
            "pi/durable/withdraw" => response(self.rpc.request("withdraw", params).await.map_err(error)?),
            "x.ai/queue/remove" => response(self.rpc.request("withdraw", params).await.map_err(error)?),
            "x.ai/queue/clear" => {
                let inbox = self.rpc.request("snapshot", json!({})).await.map_err(error)?;
                if let Some(items) = inbox["inbox"].as_array() { for item in items { self.rpc.request("withdraw", json!({"id":item["id"]})).await.map_err(error)?; } }
                response(json!({}))
            }
            "pi/durable/tasks" => response(self.rpc.request("tasks", params).await.map_err(error)?),
            "x.ai/session/info" => {
                let data = self.rpc.request("sessionInfo", json!({})).await.map_err(error)?;
                let models = crate::model::parse_models(&self.state.borrow().boot);
                let state = self.state.borrow(); let selected = &state.boot["snapshot"]["agent"]["model"];
                let model = models.iter().find(|model| model.provider == selected["provider"].as_str().unwrap_or_default() && model.id == selected["modelId"].as_str().unwrap_or_default());
                response(crate::context_projection::build_session_info_response(&data["stats"], Some(&data["messages"]), &self.session_id(), data["cwd"].as_str().unwrap_or_default(), model, None, None, data["stats"]["sessionFile"].as_str(), Some("Pi Durable")))
            }
            _ => Err(acp::Error::method_not_found().data(format!("{} is not supported by Durable", args.method))),
        }
    }
    async fn ext_notification(&self, _: acp::ExtNotification) -> Result<(), acp::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn durable_output_uses_js_units_and_replacements() {
        let mut value = "你😀ok".to_owned();
        assert_eq!(
            apply_output(&mut value, &json!({"trimStart":3,"append":"!"})),
            "ok!"
        );
        assert_eq!(apply_output(&mut value, &json!({"set":"new"})), "new");
    }
}
