//! Headless external-agent adapters for Exaforge subagents.
//!
//! Native subagents continue through Grok Build's channel coordinator. Only the
//! explicit `claude-code` and `codex-cli` types are routed to child processes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
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

pub(crate) const CLAUDE_CODE_TYPE: &str = "claude-code";
pub(crate) const CODEX_CLI_TYPE: &str = "codex-cli";
const EXTERNAL_TYPES: [&str; 2] = [CLAUDE_CODE_TYPE, CODEX_CLI_TYPE];

/// Route explicit external harness types to headless CLIs and preserve the
/// stock channel backend for every native/user-defined subagent.
pub(crate) struct CompositeSubagentBackend {
    native: Arc<dyn SubagentBackend>,
    external: ExternalSubagentBackend,
}

impl CompositeSubagentBackend {
    pub(crate) fn new(native: Arc<dyn SubagentBackend>) -> Self {
        Self {
            native,
            external: ExternalSubagentBackend::default(),
        }
    }

    fn is_external(name: &str) -> bool {
        EXTERNAL_TYPES.contains(&name)
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
                .describe_subagent_type(
                    subagent_type,
                    harness_agent_type,
                    parent_session_id,
                )
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
    for name in EXTERNAL_TYPES {
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
        can_edit: true,
        can_write: true,
    }
}

#[derive(Clone, Default)]
struct ExternalSubagentBackend {
    tasks: Arc<Mutex<HashMap<String, Arc<ExternalTask>>>>,
}

struct ExternalTask {
    snapshot: Mutex<SubagentSnapshot>,
    result: Mutex<Option<SubagentResult>>,
    external_session_id: Mutex<Option<String>>,
    cancel: CancellationToken,
    completed: Notify,
}

impl ExternalSubagentBackend {
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
        let session_id = task.external_session_id.lock().await.clone();
        session_id.ok_or_else(|| {
            ToolError::invalid_arguments(format!(
                "Cannot resume external subagent '{id}': the CLI did not return a session ID"
            ))
        })
    }

    async fn run(&self, request: SubagentRequest) -> Result<SubagentResult, ToolError> {
        let started_at_epoch_ms = now_epoch_ms();
        let started = Instant::now();
        let task = Arc::new(ExternalTask {
            snapshot: Mutex::new(SubagentSnapshot {
                subagent_id: request.id.clone(),
                description: request.description.clone(),
                subagent_type: request.subagent_type.clone(),
                status: SubagentSnapshotStatus::Initializing,
                started_at_epoch_ms,
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

        let resume_session_id = match request.resume_from.as_deref() {
            Some(source) => Some(
                self.resume_session_id(source, &request.subagent_type)
                    .await?,
            ),
            None => None,
        };
        let spec = ExternalCommandSpec::from_request(&request, resume_session_id.as_deref())?;

        {
            let mut snapshot = task.snapshot.lock().await;
            snapshot.status = running_status(0, 0);
        }

        let result = run_external_process(&request, &spec, &task, started).await;
        let result = match result {
            Ok(result) => result,
            Err(error) => SubagentResult {
                success: false,
                error: Some(error.to_string()),
                cancelled: task.cancel.is_cancelled(),
                subagent_id: request.id.clone(),
                child_session_id: task
                    .external_session_id
                    .lock()
                    .await
                    .clone()
                    .unwrap_or_else(|| request.id.clone()),
                duration_ms: started.elapsed().as_millis() as u64,
                ..Default::default()
            },
        };
        let terminal_status = if result.cancelled {
            SubagentSnapshotStatus::Cancelled
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
        Ok(result)
    }
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
        if EXTERNAL_TYPES.contains(&subagent_type) {
            SubagentValidateTypeOutcome::Ok
        } else {
            SubagentValidateTypeOutcome::Unknown {
                available: EXTERNAL_TYPES.iter().map(|name| (*name).to_owned()).collect(),
            }
        }
    }

    async fn describe_subagent_type(
        &self,
        subagent_type: &str,
        _harness_agent_type: Option<&str>,
        _parent_session_id: &str,
    ) -> SubagentDescribeOutcome {
        if EXTERNAL_TYPES.contains(&subagent_type) {
            SubagentDescribeOutcome::Ok(external_type_summary())
        } else {
            SubagentDescribeOutcome::Unknown {
                available: EXTERNAL_TYPES.iter().map(|name| (*name).to_owned()).collect(),
            }
        }
    }
}

struct ExternalCommandSpec {
    program: &'static str,
    args: Vec<String>,
    cwd: PathBuf,
    kind: ExternalKind,
}

#[derive(Clone, Copy)]
enum ExternalKind {
    Claude,
    Codex,
}

impl ExternalCommandSpec {
    fn from_request(request: &SubagentRequest, resume_session_id: Option<&str>) -> Result<Self, ToolError> {
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
        let cwd = request
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
        let capability = request
            .runtime_overrides
            .capability_mode
            .unwrap_or(SubagentCapabilityMode::All);
        let model = request.runtime_overrides.model.as_deref();
        match request.subagent_type.as_str() {
            CLAUDE_CODE_TYPE => Ok(Self {
                program: "claude",
                args: claude_args(&request.prompt, capability, model, resume_session_id),
                cwd,
                kind: ExternalKind::Claude,
            }),
            CODEX_CLI_TYPE => Ok(Self {
                program: "codex",
                args: codex_args(&request.prompt, capability, model, resume_session_id),
                cwd,
                kind: ExternalKind::Codex,
            }),
            other => Err(ToolError::invalid_arguments(format!(
                "Unsupported external subagent type: {other}"
            ))),
        }
    }
}

fn claude_args(
    prompt: &str,
    capability: SubagentCapabilityMode,
    model: Option<&str>,
    resume_session_id: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "--print".to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
        "--forward-subagent-text".to_owned(),
        "--permission-mode".to_owned(),
        "dontAsk".to_owned(),
        "--allowedTools".to_owned(),
        claude_allowed_tools(capability).to_owned(),
    ];
    if let Some(model) = model {
        args.extend(["--model".to_owned(), model.to_owned()]);
    }
    if let Some(session_id) = resume_session_id {
        args.extend(["--resume".to_owned(), session_id.to_owned()]);
    }
    args.push(prompt.to_owned());
    args
}

fn claude_allowed_tools(capability: SubagentCapabilityMode) -> &'static str {
    match capability {
        SubagentCapabilityMode::ReadOnly => "Read,Glob,Grep,WebSearch,WebFetch",
        SubagentCapabilityMode::ReadWrite => {
            "Read,Glob,Grep,Edit,Write,NotebookEdit,WebSearch,WebFetch"
        }
        SubagentCapabilityMode::Execute => "Read,Glob,Grep,Bash,WebSearch,WebFetch",
        SubagentCapabilityMode::All => {
            "Read,Glob,Grep,Edit,Write,NotebookEdit,Bash,WebSearch,WebFetch"
        }
    }
}

fn codex_args(
    prompt: &str,
    capability: SubagentCapabilityMode,
    model: Option<&str>,
    resume_session_id: Option<&str>,
) -> Vec<String> {
    let sandbox = match capability {
        SubagentCapabilityMode::ReadOnly | SubagentCapabilityMode::Execute => "read-only",
        SubagentCapabilityMode::ReadWrite | SubagentCapabilityMode::All => "workspace-write",
    };
    let mut args = vec!["exec".to_owned()];
    if let Some(session_id) = resume_session_id {
        args.extend(["resume".to_owned(), session_id.to_owned()]);
    }
    args.extend([
        "--json".to_owned(),
        "--sandbox".to_owned(),
        sandbox.to_owned(),
        "--skip-git-repo-check".to_owned(),
    ]);
    if let Some(model) = model {
        args.extend(["--model".to_owned(), model.to_owned()]);
    }
    args.push(prompt.to_owned());
    args
}

async fn run_external_process(
    request: &SubagentRequest,
    spec: &ExternalCommandSpec,
    task: &Arc<ExternalTask>,
    started: Instant,
) -> Result<SubagentResult, ToolError> {
    let mut child = Command::new(spec.program)
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| {
            ToolError::custom(
                "external_cli_unavailable",
                format!(
                    "Cannot start {} for subagent '{}': {error}. Ensure the CLI is installed and authenticated.",
                    spec.program, request.subagent_type
                ),
            )
        })?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output).await;
        output
    });
    let mut lines = BufReader::new(stdout).lines();
    let mut parsed = ParsedOutput::default();

    loop {
        tokio::select! {
            _ = task.cancel.cancelled() => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                let stderr = stderr_task.await.unwrap_or_default();
                return Ok(SubagentResult {
                    success: false,
                    output: Arc::from(""),
                    error: (!stderr.trim().is_empty()).then(|| stderr.trim().to_owned()),
                    cancelled: true,
                    subagent_id: request.id.clone(),
                    child_session_id: request.id.clone(),
                    tool_calls: parsed.tool_calls,
                    turns: parsed.turns,
                    duration_ms: started.elapsed().as_millis() as u64,
                    ..Default::default()
                });
            }
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        parse_event(spec.kind, &line, &mut parsed);
                        if let Some(session_id) = parsed.session_id.clone() {
                            *task.external_session_id.lock().await = Some(session_id);
                        }
                        let mut snapshot = task.snapshot.lock().await;
                        snapshot.duration_ms = started.elapsed().as_millis() as u64;
                        snapshot.status = running_status(parsed.turns, parsed.tool_calls);
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let _ = child.kill().await;
                        return Err(ToolError::custom(
                            "external_stream_error",
                            format!("Failed reading {} JSON stream: {error}", spec.program),
                        ));
                    }
                }
            }
        }
    }

    let status = child.wait().await.map_err(|error| {
        ToolError::custom(
            "external_cli_wait_failed",
            format!("Failed waiting for {}: {error}", spec.program),
        )
    })?;
    let stderr = stderr_task.await.unwrap_or_default();
    let external_session_id = parsed
        .session_id
        .clone()
        .unwrap_or_else(|| request.id.clone());
    *task.external_session_id.lock().await = parsed.session_id.clone();
    if !status.success() {
        let detail = if stderr.trim().is_empty() {
            format!("{} exited with status {status}", spec.program)
        } else {
            stderr.trim().to_owned()
        };
        return Ok(SubagentResult {
            success: false,
            error: Some(detail),
            subagent_id: request.id.clone(),
            child_session_id: external_session_id,
            tool_calls: parsed.tool_calls,
            turns: parsed.turns,
            duration_ms: started.elapsed().as_millis() as u64,
            ..Default::default()
        });
    }
    if parsed.final_text.trim().is_empty() {
        parsed.final_text = "External subagent completed without a final text response.".to_owned();
    }
    Ok(SubagentResult {
        success: true,
        output: Arc::from(parsed.final_text),
        subagent_id: request.id.clone(),
        child_session_id: external_session_id,
        tool_calls: parsed.tool_calls,
        turns: parsed.turns.max(1),
        duration_ms: started.elapsed().as_millis() as u64,
        worktree_path: None,
        ..Default::default()
    })
}

#[derive(Default)]
struct ParsedOutput {
    final_text: String,
    session_id: Option<String>,
    tool_calls: u32,
    turns: u32,
}

fn parse_event(kind: ExternalKind, line: &str, output: &mut ParsedOutput) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return;
    };
    match kind {
        ExternalKind::Claude => parse_claude_event(&value, output),
        ExternalKind::Codex => parse_codex_event(&value, output),
    }
}

fn parse_claude_event(value: &serde_json::Value, output: &mut ParsedOutput) {
    if let Some(session_id) = value.get("session_id").and_then(|v| v.as_str()) {
        output.session_id = Some(session_id.to_owned());
    }
    match value.get("type").and_then(|v| v.as_str()) {
        Some("assistant") => {
            output.turns = output.turns.saturating_add(1);
            if let Some(content) = value
                .pointer("/message/content")
                .and_then(|content| content.as_array())
            {
                output.tool_calls = output.tool_calls.saturating_add(
                    content
                        .iter()
                        .filter(|item| item.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
                        .count() as u32,
                );
            }
        }
        Some("result") => {
            if let Some(result) = value.get("result").and_then(|v| v.as_str()) {
                output.final_text = result.to_owned();
            }
        }
        _ => {}
    }
}

fn parse_codex_event(value: &serde_json::Value, output: &mut ParsedOutput) {
    match value.get("type").and_then(|v| v.as_str()) {
        Some("thread.started") => {
            if let Some(thread_id) = value.get("thread_id").and_then(|v| v.as_str()) {
                output.session_id = Some(thread_id.to_owned());
            }
        }
        Some("turn.completed") => output.turns = output.turns.saturating_add(1),
        Some("item.completed") => {
            let item_type = value.pointer("/item/type").and_then(|v| v.as_str());
            if item_type == Some("agent_message")
                && let Some(text) = value.pointer("/item/text").and_then(|v| v.as_str())
            {
                output.final_text = text.to_owned();
            }
            if matches!(
                item_type,
                Some("command_execution" | "file_change" | "mcp_tool_call")
            ) {
                output.tool_calls = output.tool_calls.saturating_add(1);
            }
        }
        _ => {}
    }
}

fn running_status(turn_count: u32, tool_call_count: u32) -> SubagentSnapshotStatus {
    SubagentSnapshotStatus::Running {
        turn_count,
        tool_call_count,
        tokens_used: 0,
        context_window_tokens: 0,
        context_usage_pct: 0,
        tools_used: Vec::new(),
        error_count: 0,
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
    fn claude_capabilities_never_use_dangerous_bypass() {
        let args = claude_args(
            "test",
            SubagentCapabilityMode::All,
            None,
            None,
        );
        assert!(!args.iter().any(|arg| arg.contains("dangerously")));
        assert!(args.iter().any(|arg| arg == "dontAsk"));
    }

    #[test]
    fn codex_capabilities_use_bounded_sandboxes() {
        let all = codex_args("test", SubagentCapabilityMode::All, None, None);
        assert!(all.iter().any(|arg| arg == "workspace-write"));
        assert!(!all.iter().any(|arg| arg.contains("dangerously")));
        let read_only = codex_args("test", SubagentCapabilityMode::ReadOnly, None, None);
        assert!(read_only.iter().any(|arg| arg == "read-only"));
    }

    #[test]
    fn parses_claude_result_and_session() {
        let mut output = ParsedOutput::default();
        parse_event(
            ExternalKind::Claude,
            r#"{"type":"result","session_id":"claude-session","result":"done"}"#,
            &mut output,
        );
        assert_eq!(output.session_id.as_deref(), Some("claude-session"));
        assert_eq!(output.final_text, "done");
    }

    #[test]
    fn parses_codex_result_and_session() {
        let mut output = ParsedOutput::default();
        parse_event(
            ExternalKind::Codex,
            r#"{"type":"thread.started","thread_id":"codex-session"}"#,
            &mut output,
        );
        parse_event(
            ExternalKind::Codex,
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"done"}}"#,
            &mut output,
        );
        assert_eq!(output.session_id.as_deref(), Some("codex-session"));
        assert_eq!(output.final_text, "done");
    }
}
