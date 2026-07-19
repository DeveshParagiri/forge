//! Provider-neutral process and lifecycle host for Forge CLI subagents.
//!
//! Provider-specific command construction and JSONL translation live in
//! `external_subagents/providers/`. Adding another CLI requires an adapter and
//! one registry entry, without changing polling, cancellation, or TUI routing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, LazyLock, Mutex as StdMutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_client_protocol as acp;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, Notify};
use tokio_util::sync::CancellationToken;
use xai_grok_tools::implementations::grok_build::task::backend::SubagentBackend;
use xai_grok_tools::implementations::grok_build::task::types::{
    SubagentCancelOutcome, SubagentDescribeOutcome, SubagentRequest, SubagentResult,
    SubagentSnapshot, SubagentSnapshotStatus, SubagentTypeSummary, SubagentValidateTypeOutcome,
};
use xai_grok_tools::types::tool::ToolKind;
use xai_tool_runtime::ToolError;
use xai_tool_types::{SubagentCapabilityMode, SubagentIsolationMode};

use self::providers::{
    ExternalProvider, ExternalProviderSession, ProviderInvocation, ProviderState,
};
use super::subagent_ui::ExternalSubagentUi;

mod providers;

pub(crate) use providers::{CLAUDE_CODE_TYPE, CODEX_CLI_TYPE};

const REGISTRY_RUNNING: u8 = 0;
const REGISTRY_COMPLETED: u8 = 1;
const REGISTRY_FAILED: u8 = 2;
const REGISTRY_CANCELLED: u8 = 3;

struct RegistryTask {
    cancel: CancellationToken,
    status: AtomicU8,
}

static EXTERNAL_TASKS: LazyLock<StdMutex<HashMap<String, Arc<RegistryTask>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

/// Minimal delegation target for the generic ACP cancel endpoint. External
/// task ownership and cancellation behavior remain entirely inside Forge.
pub(crate) fn resolve_cancel(id: &str, native: SubagentCancelOutcome) -> SubagentCancelOutcome {
    if matches!(native, SubagentCancelOutcome::NotFound) {
        cancel_registered(id)
    } else {
        native
    }
}

fn cancel_registered(id: &str) -> SubagentCancelOutcome {
    let task = EXTERNAL_TASKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(id)
        .cloned();
    let Some(task) = task else {
        return SubagentCancelOutcome::NotFound;
    };
    match task.status.load(Ordering::Acquire) {
        REGISTRY_RUNNING => {
            task.cancel.cancel();
            SubagentCancelOutcome::Cancelled
        }
        REGISTRY_COMPLETED => SubagentCancelOutcome::AlreadyFinished {
            status: "completed".to_owned(),
        },
        REGISTRY_CANCELLED => SubagentCancelOutcome::AlreadyFinished {
            status: "cancelled".to_owned(),
        },
        _ => SubagentCancelOutcome::AlreadyFinished {
            status: "failed".to_owned(),
        },
    }
}

fn register_task(id: &str, cancel: CancellationToken) -> Arc<RegistryTask> {
    let task = Arc::new(RegistryTask {
        cancel,
        status: AtomicU8::new(REGISTRY_RUNNING),
    });
    EXTERNAL_TASKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(id.to_owned(), task.clone());
    task
}

/// Routes registered external harnesses to CLI adapters and preserves the
/// stock channel backend for every native or user-defined subagent.
pub(crate) struct CompositeSubagentBackend {
    native: Arc<dyn SubagentBackend>,
    external: ExternalSubagentBackend,
}

impl CompositeSubagentBackend {
    pub(crate) fn new(native: Arc<dyn SubagentBackend>, ui: Option<ExternalSubagentUi>) -> Self {
        Self {
            native,
            external: ExternalSubagentBackend::new(ui),
        }
    }

    fn is_external(name: &str) -> bool {
        providers::find(name).is_some()
    }
}

#[async_trait::async_trait]
impl SubagentBackend for CompositeSubagentBackend {
    async fn spawn(&self, request: SubagentRequest) -> Result<SubagentResult, ToolError> {
        if Self::is_external(&request.subagent_type) {
            self.external.spawn(request).await
        } else {
            self.native.spawn(request).await
        }
    }

    async fn query(
        &self,
        id: &str,
        block: bool,
        timeout_ms: Option<u64>,
    ) -> Option<SubagentSnapshot> {
        if self.external.contains(id).await {
            self.external.query(id, block, timeout_ms).await
        } else {
            self.native.query(id, block, timeout_ms).await
        }
    }

    async fn cancel(&self, id: &str) -> SubagentCancelOutcome {
        if self.external.contains(id).await {
            self.external.cancel(id).await
        } else {
            self.native.cancel(id).await
        }
    }

    async fn validate_type(
        &self,
        subagent_type: &str,
        parent_session_id: &str,
    ) -> SubagentValidateTypeOutcome {
        if Self::is_external(subagent_type) {
            SubagentValidateTypeOutcome::Ok
        } else {
            match self
                .native
                .validate_type(subagent_type, parent_session_id)
                .await
            {
                SubagentValidateTypeOutcome::Unknown { mut available } => {
                    extend_available(&mut available);
                    SubagentValidateTypeOutcome::Unknown { available }
                }
                other => other,
            }
        }
    }

    async fn describe_subagent_type(
        &self,
        subagent_type: &str,
        harness_agent_type: Option<&str>,
        parent_session_id: &str,
    ) -> SubagentDescribeOutcome {
        if Self::is_external(subagent_type) {
            SubagentDescribeOutcome::Ok(external_type_summary())
        } else {
            match self
                .native
                .describe_subagent_type(subagent_type, harness_agent_type, parent_session_id)
                .await
            {
                SubagentDescribeOutcome::Unknown { mut available } => {
                    extend_available(&mut available);
                    SubagentDescribeOutcome::Unknown { available }
                }
                other => other,
            }
        }
    }
}

fn extend_available(available: &mut Vec<String>) {
    for name in providers::names() {
        if !available.iter().any(|candidate| candidate == name) {
            available.push(name.to_owned());
        }
    }
    available.sort();
}

fn external_type_summary() -> SubagentTypeSummary {
    let mut tool_names = HashMap::new();
    tool_names.insert(ToolKind::Read, "external read".to_owned());
    tool_names.insert(ToolKind::Search, "external search".to_owned());
    tool_names.insert(ToolKind::Edit, "external edit".to_owned());
    tool_names.insert(ToolKind::Write, "external write".to_owned());
    tool_names.insert(ToolKind::Execute, "external shell".to_owned());
    SubagentTypeSummary {
        tool_names,
        can_read: true,
        can_search: true,
        can_execute: true,
    }
}

#[derive(Clone)]
struct ExternalSubagentBackend {
    tasks: Arc<Mutex<HashMap<String, Arc<ExternalTask>>>>,
    ui: Option<ExternalSubagentUi>,
}

impl ExternalSubagentBackend {
    fn new(ui: Option<ExternalSubagentUi>) -> Self {
        Self {
            tasks: Arc::default(),
            ui,
        }
    }

    async fn contains(&self, id: &str) -> bool {
        self.tasks.lock().await.contains_key(id)
    }

    async fn task(&self, id: &str) -> Option<Arc<ExternalTask>> {
        self.tasks.lock().await.get(id).cloned()
    }

    async fn resume_session_id(&self, id: &str, expected_type: &str) -> Result<String, ToolError> {
        let task = self.task(id).await.ok_or_else(|| {
            ToolError::invalid_arguments(format!(
                "Cannot resume from external subagent '{id}': not found in this process"
            ))
        })?;
        let snapshot = task.snapshot.lock().await;
        if snapshot.subagent_type != expected_type {
            return Err(ToolError::invalid_arguments(format!(
                "Cannot resume {expected_type} from '{}': source type is '{}'",
                id, snapshot.subagent_type
            )));
        }
        drop(snapshot);
        task.external_session_id
            .lock()
            .await
            .clone()
            .ok_or_else(|| {
                ToolError::invalid_arguments(format!(
                    "Cannot resume external subagent '{id}': the CLI did not return a session ID"
                ))
            })
    }

    async fn run(&self, request: SubagentRequest) -> Result<SubagentResult, ToolError> {
        let resume_session_id = match request.resume_from.as_deref() {
            Some(source) => Some(
                self.resume_session_id(source, &request.subagent_type)
                    .await?,
            ),
            None => None,
        };
        let mut spec = ExternalCommandSpec::from_request(&request, resume_session_id.as_deref())?;
        let started = Instant::now();
        let task = Arc::new(ExternalTask {
            snapshot: Mutex::new(SubagentSnapshot {
                subagent_id: request.id.clone(),
                description: request.description.clone(),
                subagent_type: request.subagent_type.clone(),
                status: running_status(0, 0, 0, 0, 0, Vec::new(), 0),
                started_at_epoch_ms: now_epoch_ms(),
                duration_ms: 0,
                persona: None,
            }),
            result: Mutex::new(None),
            external_session_id: Mutex::new(None),
            cancel: CancellationToken::new(),
            completed: Notify::new(),
        });
        self.tasks
            .lock()
            .await
            .insert(request.id.clone(), task.clone());
        let registry_task = register_task(&request.id, task.cancel.clone());

        if let Some(ui) = &self.ui {
            let effective_model = spawned_model(&spec);
            persist_external_child_meta(
                ui.parent_session_id(),
                ui.parent_cwd(),
                &request,
                &spec.cwd,
                &effective_model,
            );
            ui.spawned(
                &request.id,
                &request.subagent_type,
                &request.description,
                &request.prompt,
                &spec.cwd,
                request.parent_prompt_id.clone(),
                request
                    .runtime_overrides
                    .capability_mode
                    .map(|mode| mode.as_str().to_owned()),
                Some(effective_model),
                request.resume_from.clone(),
                spec.provider.display_name(),
            );
            ui.workspace(&request.id, &spec.cwd).await;
            ui.progress(&request.id, 0, 0, 0, 0, 0, 0, Vec::new(), 0);
        }

        let result =
            run_external_process(&request, &mut spec, &task, started, self.ui.as_ref()).await;
        let result = match result {
            Ok(result) => result,
            Err(error) => SubagentResult {
                success: false,
                error: Some(error.to_string()),
                cancelled: task.cancel.is_cancelled(),
                subagent_id: request.id.clone(),
                child_session_id: request.id.clone(),
                duration_ms: started.elapsed().as_millis() as u64,
                ..Default::default()
            },
        };
        let (registry_status, ui_status) = if result.cancelled {
            (REGISTRY_CANCELLED, "cancelled")
        } else if result.success {
            (REGISTRY_COMPLETED, "completed")
        } else {
            (REGISTRY_FAILED, "failed")
        };
        registry_task
            .status
            .store(registry_status, Ordering::Release);

        let terminal_status = if result.cancelled {
            SubagentSnapshotStatus::Cancelled {
                reason: result.error.clone(),
            }
        } else if result.success {
            SubagentSnapshotStatus::Completed {
                output: result.output.to_string(),
                tool_calls: result.tool_calls,
                turns: result.turns,
                worktree_path: result.worktree_path.clone(),
            }
        } else {
            SubagentSnapshotStatus::Failed {
                error: result
                    .error
                    .clone()
                    .unwrap_or_else(|| "External subagent failed".to_owned()),
            }
        };
        {
            let mut snapshot = task.snapshot.lock().await;
            snapshot.duration_ms = result.duration_ms;
            snapshot.status = terminal_status;
        }
        *task.result.lock().await = Some(result.clone());
        task.completed.notify_waiters();
        if let Some(ui) = &self.ui {
            ui.finished(
                &request.id,
                ui_status,
                result.error.clone(),
                result.tool_calls,
                result.turns,
                result.duration_ms,
                result.tokens_used,
                result.success.then(|| result.output.to_string()),
            );
        }
        Ok(result)
    }
}

struct ExternalTask {
    snapshot: Mutex<SubagentSnapshot>,
    result: Mutex<Option<SubagentResult>>,
    external_session_id: Mutex<Option<String>>,
    cancel: CancellationToken,
    completed: Notify,
}

#[async_trait::async_trait]
impl SubagentBackend for ExternalSubagentBackend {
    async fn spawn(&self, request: SubagentRequest) -> Result<SubagentResult, ToolError> {
        self.run(request).await
    }

    async fn query(
        &self,
        id: &str,
        block: bool,
        timeout_ms: Option<u64>,
    ) -> Option<SubagentSnapshot> {
        let task = self.task(id).await?;
        if block && task.result.lock().await.is_none() {
            let wait = task.completed.notified();
            let timeout = Duration::from_millis(timeout_ms.unwrap_or(30_000));
            let _ = tokio::time::timeout(timeout, wait).await;
        }
        Some(task.snapshot.lock().await.clone())
    }

    async fn cancel(&self, id: &str) -> SubagentCancelOutcome {
        let Some(task) = self.task(id).await else {
            return SubagentCancelOutcome::NotFound;
        };
        if let Some(result) = task.result.lock().await.as_ref() {
            return SubagentCancelOutcome::AlreadyFinished {
                status: result.status().to_owned(),
            };
        }
        task.cancel.cancel();
        SubagentCancelOutcome::Cancelled
    }

    async fn validate_type(
        &self,
        subagent_type: &str,
        _parent_session_id: &str,
    ) -> SubagentValidateTypeOutcome {
        if providers::find(subagent_type).is_some() {
            SubagentValidateTypeOutcome::Ok
        } else {
            SubagentValidateTypeOutcome::Unknown {
                available: providers::names().map(str::to_owned).collect(),
            }
        }
    }

    async fn describe_subagent_type(
        &self,
        subagent_type: &str,
        _harness_agent_type: Option<&str>,
        _parent_session_id: &str,
    ) -> SubagentDescribeOutcome {
        if providers::find(subagent_type).is_some() {
            SubagentDescribeOutcome::Ok(external_type_summary())
        } else {
            SubagentDescribeOutcome::Unknown {
                available: providers::names().map(str::to_owned).collect(),
            }
        }
    }
}

struct ExternalCommandSpec {
    provider: &'static dyn ExternalProvider,
    session: Box<dyn ExternalProviderSession>,
    cwd: PathBuf,
}

fn spawned_model(spec: &ExternalCommandSpec) -> String {
    spec.session.state().effective_model.clone()
}

impl ExternalCommandSpec {
    fn from_request(
        request: &SubagentRequest,
        resume_session_id: Option<&str>,
    ) -> Result<Self, ToolError> {
        if request.fork_context {
            return Err(ToolError::invalid_arguments(
                "External CLI subagents do not support fork_context",
            ));
        }
        if request.runtime_overrides.harness_agent_type.is_some() {
            return Err(ToolError::invalid_arguments(
                "External CLI subagents do not support harness_agent_type overrides",
            ));
        }
        if request.runtime_overrides.isolation == Some(SubagentIsolationMode::Worktree) {
            return Err(ToolError::invalid_arguments(
                "External CLI subagents do not yet support isolation=worktree; provide cwd or use isolation=none",
            ));
        }
        let cwd =
            request
                .cwd
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or(std::env::current_dir().map_err(|error| {
                    ToolError::custom("cwd_unavailable", format!("Cannot resolve cwd: {error}"))
                })?);
        if !cwd.is_dir() {
            return Err(ToolError::invalid_arguments(format!(
                "cwd '{}' is not a directory",
                cwd.display()
            )));
        }
        let provider = providers::find(&request.subagent_type).ok_or_else(|| {
            ToolError::invalid_arguments(format!(
                "Unsupported external subagent type: {}",
                request.subagent_type
            ))
        })?;
        let session = provider.create_session(
            ProviderInvocation {
                prompt: &request.prompt,
                capability: request
                    .runtime_overrides
                    .capability_mode
                    .unwrap_or(SubagentCapabilityMode::All),
                model: request.runtime_overrides.model.as_deref(),
                reasoning_effort: request.runtime_overrides.reasoning_effort.as_deref(),
                resume_session_id,
            },
            &cwd,
        );
        Ok(Self {
            provider,
            session,
            cwd,
        })
    }
}

async fn run_external_process(
    request: &SubagentRequest,
    spec: &mut ExternalCommandSpec,
    task: &Arc<ExternalTask>,
    started: Instant,
    ui: Option<&ExternalSubagentUi>,
) -> Result<SubagentResult, ToolError> {
    let program = spec.session.program();
    let args = spec.session.args().to_vec();
    let initial_input = spec.session.initial_input();
    let interactive = !initial_input.is_empty();
    let mut command = Command::new(program);
    command
        .args(&args)
        .current_dir(&spec.cwd)
        .stdin(if interactive {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Own the complete CLI tree. npm/sh wrappers can otherwise exit before a
    // long-lived app-server descendant, leaving it re-parented and unkillable
    // through the Tokio child handle.
    xai_tty_utils::new_process_group(&mut command);
    let mut process_group = xai_tty_utils::ProcessGroup::new().map_err(|error| {
        ToolError::custom(
            "external_cli_unavailable",
            format!("Cannot create a process group for {program}: {error}"),
        )
    })?;
    let mut child = command
        .spawn()
        .map_err(|error| {
            ToolError::custom(
                "external_cli_unavailable",
                format!(
                    "Cannot start {program} for subagent '{}': {error}. Ensure the CLI is installed and authenticated.",
                    request.subagent_type
                ),
            )
        })?;
    if let Err(error) = process_group.attach(&child) {
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Err(ToolError::custom(
            "external_cli_unavailable",
            format!("Cannot attach {program} to its process group: {error}"),
        ));
    }
    let mut stdin = child.stdin.take();
    if let Some(writer) = stdin.as_mut()
        && let Err(error) = write_provider_messages(writer, initial_input).await
    {
        kill_external_process_group(&process_group, &mut child).await;
        return Err(error);
    }
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output).await;
        output
    });
    let mut lines = BufReader::new(stdout).lines();
    let mut protocol_terminal: Option<Result<(), String>> = None;

    loop {
        tokio::select! {
            _ = task.cancel.cancelled() => {
                kill_external_process_group(&process_group, &mut child).await;
                let _ = child.wait().await;
                let stderr = stderr_task.await.unwrap_or_default();
                let state = spec.session.state();
                return Ok(SubagentResult {
                    success: false,
                    output: Arc::from(""),
                    error: (!stderr.trim().is_empty()).then(|| stderr.trim().to_owned()),
                    cancelled: true,
                    subagent_id: request.id.clone(),
                    child_session_id: request.id.clone(),
                    tool_calls: state.tool_calls,
                    turns: state.turns,
                    duration_ms: started.elapsed().as_millis() as u64,
                    tokens_used: state.tokens_used,
                    ..Default::default()
                });
            }
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        let update = spec.session.handle_line(&line);
                        if !update.outbound.is_empty() {
                            let write_result = match stdin.as_mut() {
                                Some(writer) => write_provider_messages(writer, update.outbound).await,
                                None => Err(ToolError::custom(
                                    "external_protocol_error",
                                    format!("{program} requested an interactive protocol write without piped stdin"),
                                )),
                            };
                            if let Err(error) = write_result {
                                kill_external_process_group(&process_group, &mut child).await;
                                let _ = stderr_task.await;
                                return Err(error);
                            }
                        }
                        let state = spec.session.state();
                        if let Some(session_id) = state.session_id.clone() {
                            *task.external_session_id.lock().await = Some(session_id);
                        }
                        let tools_used = sorted_tools(state);
                        if let Some(ui) = ui {
                            for event in update.events {
                                ui.child_event(&request.id, event);
                            }
                            ui.progress(
                                &request.id,
                                started.elapsed().as_millis() as u64,
                                state.turns,
                                state.tool_calls,
                                state.tokens_used,
                                state.context_window_tokens,
                                state.context_usage_pct(),
                                tools_used.clone(),
                                state.error_count,
                            );
                            if state.tokens_used > 0 {
                                ui.context(&request.id, state.tokens_used);
                            }
                        }
                        let mut snapshot = task.snapshot.lock().await;
                        snapshot.duration_ms = started.elapsed().as_millis() as u64;
                        snapshot.status = running_status(
                            state.turns,
                            state.tool_calls,
                            state.tokens_used,
                            state.context_window_tokens,
                            state.context_usage_pct(),
                            tools_used,
                            state.error_count,
                        );
                        if let Some(terminal) = update.terminal {
                            protocol_terminal = Some(terminal);
                            // Do not keep the stdout reader borrowed while
                            // terminating the long-lived app-server tree.
                            drop(lines);
                            kill_external_process_group(&process_group, &mut child).await;
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        kill_external_process_group(&process_group, &mut child).await;
                        return Err(ToolError::custom(
                            "external_stream_error",
                            format!("Failed reading {program} JSON stream: {error}"),
                        ));
                    }
                }
            }
        }
    }

    // App-server is intentionally long-lived; protocol terminal events already
    // stopped its complete process group inside the read loop.
    let status = match child.wait().await {
        Ok(status) => status,
        Err(error) => {
            let _ = process_group.kill();
            let _ = child.kill().await;
            return Err(ToolError::custom(
                "external_cli_wait_failed",
                format!("Failed waiting for {program}: {error}"),
            ));
        }
    };
    let stderr = stderr_task.await.unwrap_or_default();
    let state = spec.session.state();
    *task.external_session_id.lock().await = state.session_id.clone();
    if let Some(Err(error)) = protocol_terminal {
        return Ok(failed_external_result(request, state, started, error));
    }
    if protocol_terminal.is_none() && !status.success() {
        let detail = if stderr.trim().is_empty() {
            format!("{program} exited with status {status}")
        } else {
            stderr.trim().to_owned()
        };
        return Ok(failed_external_result(request, state, started, detail));
    }
    let output = if state.final_text.trim().is_empty() {
        "External subagent completed without a final text response.".to_owned()
    } else {
        state.final_text.clone()
    };
    Ok(SubagentResult {
        success: true,
        output: Arc::from(output),
        subagent_id: request.id.clone(),
        child_session_id: request.id.clone(),
        tool_calls: state.tool_calls,
        turns: state.turns.max(1),
        duration_ms: started.elapsed().as_millis() as u64,
        tokens_used: state.tokens_used,
        worktree_path: None,
        ..Default::default()
    })
}

async fn kill_external_process_group(
    process_group: &xai_tty_utils::ProcessGroup,
    child: &mut tokio::process::Child,
) {
    let _ = process_group.terminate();
    // Give wrappers and descendants a brief chance to close cleanly.
    if tokio::time::timeout(Duration::from_millis(500), child.wait())
        .await
        .is_ok()
    {
        // The direct child may have exited before its app-server descendant.
        // Always close the validated group after reaping the leader.
        let _ = process_group.kill();
        return;
    }
    let _ = process_group.kill();
    let _ = child.kill().await;
    let _ = child.wait().await;
}

async fn write_provider_messages(
    stdin: &mut tokio::process::ChildStdin,
    messages: Vec<String>,
) -> Result<(), ToolError> {
    for message in messages {
        stdin.write_all(message.as_bytes()).await.map_err(|error| {
            ToolError::custom(
                "external_protocol_error",
                format!("Failed writing provider protocol message: {error}"),
            )
        })?;
        stdin.write_all(b"\n").await.map_err(|error| {
            ToolError::custom(
                "external_protocol_error",
                format!("Failed terminating provider protocol message: {error}"),
            )
        })?;
    }
    stdin.flush().await.map_err(|error| {
        ToolError::custom(
            "external_protocol_error",
            format!("Failed flushing provider protocol messages: {error}"),
        )
    })
}

fn persist_external_child_meta(
    parent_session_id: &str,
    parent_cwd: &Path,
    request: &SubagentRequest,
    cwd: &Path,
    effective_model: &str,
) {
    let parent_info = crate::session::info::Info {
        id: acp::SessionId::new(parent_session_id.to_owned()),
        cwd: parent_cwd.to_string_lossy().into_owned(),
    };
    let dir = crate::session::persistence::session_dir(&parent_info)
        .join("subagents")
        .join(&request.id);
    if let Err(error) = std::fs::create_dir_all(&dir) {
        tracing::warn!(%error, subagent_id = request.id, "failed to create external subagent metadata directory");
        return;
    }
    let meta = serde_json::json!({
        "prompt": request.prompt,
        "child_cwd": cwd,
        "worktree_path": null,
        "effective_model_id": effective_model
    });
    if let Err(error) = std::fs::write(
        dir.join("meta.json"),
        serde_json::to_vec_pretty(&meta).unwrap_or_default(),
    ) {
        tracing::warn!(%error, subagent_id = request.id, "failed to persist external subagent metadata");
    }
}

fn failed_external_result(
    request: &SubagentRequest,
    state: &ProviderState,
    started: Instant,
    error: String,
) -> SubagentResult {
    SubagentResult {
        success: false,
        error: Some(error),
        subagent_id: request.id.clone(),
        child_session_id: request.id.clone(),
        tool_calls: state.tool_calls,
        turns: state.turns,
        duration_ms: started.elapsed().as_millis() as u64,
        tokens_used: state.tokens_used,
        ..Default::default()
    }
}
fn sorted_tools(state: &ProviderState) -> Vec<String> {
    let mut tools = state.tools_used.iter().cloned().collect::<Vec<_>>();
    tools.sort();
    tools
}

fn running_status(
    turn_count: u32,
    tool_call_count: u32,
    tokens_used: u64,
    context_window_tokens: u64,
    context_usage_pct: u8,
    tools_used: Vec<String>,
    error_count: u32,
) -> SubagentSnapshotStatus {
    SubagentSnapshotStatus::Running {
        turn_count,
        tool_call_count,
        tokens_used,
        context_window_tokens,
        context_usage_pct,
        tools_used,
        error_count,
    }
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_cancel_is_forge_owned() {
        let id = format!("registry-test-{}", now_epoch_ms());
        let token = CancellationToken::new();
        let registered = register_task(&id, token.clone());
        assert!(matches!(
            cancel_registered(&id),
            SubagentCancelOutcome::Cancelled
        ));
        assert!(token.is_cancelled());
        registered
            .status
            .store(REGISTRY_COMPLETED, Ordering::Release);
        assert!(matches!(
            cancel_registered(&id),
            SubagentCancelOutcome::AlreadyFinished { status } if status == "completed"
        ));
        EXTERNAL_TASKS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&id);
    }

    #[test]
    fn available_types_come_from_provider_registry() {
        let mut available = vec!["general-purpose".to_owned()];
        extend_available(&mut available);
        assert!(available.iter().any(|name| name == CLAUDE_CODE_TYPE));
        assert!(available.iter().any(|name| name == CODEX_CLI_TYPE));
    }
}
