use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use agent_client_protocol as acp;
use serde_json::{Value, json};
use xai_tool_types::SubagentCapabilityMode;

use crate::exaforge::subagent_ui::ChildEvent;

use super::{
    ExternalProvider, ExternalProviderSession, ProviderInvocation, ProviderState, ProviderUpdate,
};

pub(crate) const CODEX_CLI_TYPE: &str = "codex-cli";
const DEFAULT_MODEL: &str = "gpt-5.6-sol";
pub(crate) static CODEX_CLI: CodexCliProvider = CodexCliProvider;

pub(crate) struct CodexCliProvider;

impl ExternalProvider for CodexCliProvider {
    fn subagent_type(&self) -> &'static str {
        CODEX_CLI_TYPE
    }

    fn display_name(&self) -> &'static str {
        "Codex CLI"
    }

    fn program(&self) -> &'static str {
        "codex"
    }

    fn create_session(
        &self,
        invocation: ProviderInvocation<'_>,
        cwd: &Path,
    ) -> Box<dyn ExternalProviderSession> {
        Box::new(CodexAppServerSession::new(invocation, cwd))
    }
}

/// Codex `exec --json` reports item boundaries but not token deltas. The
/// app-server transport exposes the same thread model plus live message,
/// reasoning, and command-output notifications.
struct CodexAppServerSession {
    args: Vec<String>,
    state: ProviderState,
    prompt: String,
    cwd: PathBuf,
    capability: SubagentCapabilityMode,
    model: String,
    reasoning_effort: Option<String>,
    resume_session_id: Option<String>,
    streamed_messages: HashSet<String>,
    streamed_reasoning: HashSet<String>,
    command_output: HashMap<String, String>,
}

impl CodexAppServerSession {
    fn new(invocation: ProviderInvocation<'_>, cwd: &Path) -> Self {
        let model = invocation.model.unwrap_or(DEFAULT_MODEL).to_owned();
        Self {
            args: vec!["app-server".to_owned(), "--stdio".to_owned()],
            state: ProviderState::new(model.clone()),
            prompt: invocation.prompt.to_owned(),
            cwd: cwd.to_owned(),
            capability: invocation.capability,
            model,
            reasoning_effort: invocation.reasoning_effort.map(str::to_owned),
            resume_session_id: invocation.resume_session_id.map(str::to_owned),
            streamed_messages: HashSet::new(),
            streamed_reasoning: HashSet::new(),
            command_output: HashMap::new(),
        }
    }

    fn initialize_request() -> String {
        json_line(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": {"name": "exaforge", "version": env!("CARGO_PKG_VERSION")},
                "capabilities": {"experimentalApi": true}
            }
        }))
    }

    fn thread_request(&self) -> String {
        let sandbox = match self.capability {
            SubagentCapabilityMode::ReadOnly | SubagentCapabilityMode::Execute => "read-only",
            SubagentCapabilityMode::ReadWrite | SubagentCapabilityMode::All => "workspace-write",
        };
        let (method, mut params) = if let Some(thread_id) = &self.resume_session_id {
            ("thread/resume", json!({"threadId": thread_id}))
        } else {
            (
                "thread/start",
                json!({
                    "cwd": self.cwd,
                    "approvalPolicy": "never",
                    "sandbox": sandbox
                }),
            )
        };
        params["model"] = Value::String(self.model.clone());
        // Resume accepts cwd/sandbox overrides too; keep the requested
        // capability and directory stable across the provider thread.
        if self.resume_session_id.is_some() {
            params["cwd"] = Value::String(self.cwd.to_string_lossy().into_owned());
            params["sandbox"] = Value::String(sandbox.to_owned());
            params["approvalPolicy"] = Value::String("never".to_owned());
        }
        json_line(json!({
            "jsonrpc": "2.0", "id": 2, "method": method, "params": params
        }))
    }

    fn turn_request(&self, thread_id: &str) -> String {
        let mut params = json!({
            "threadId": thread_id,
            "input": [{"type": "text", "text": self.prompt}],
            "cwd": self.cwd,
            "approvalPolicy": "never"
        });
        params["model"] = Value::String(self.model.clone());
        if let Some(effort) = &self.reasoning_effort {
            params["effort"] = Value::String(effort.clone());
        }
        json_line(json!({
            "jsonrpc": "2.0", "id": 3, "method": "turn/start", "params": params
        }))
    }

    fn handle_response(&mut self, value: &Value) -> ProviderUpdate {
        match value.get("id").and_then(Value::as_u64) {
            Some(1) => {
                if let Some(error) = rpc_error(value) {
                    return terminal_error(format!("Codex initialize failed: {error}"));
                }
                ProviderUpdate {
                    events: Vec::new(),
                    outbound: vec![
                        json_line(json!({"jsonrpc": "2.0", "method": "initialized"})),
                        self.thread_request(),
                    ],
                    terminal: None,
                }
            }
            Some(2) => {
                if let Some(error) = rpc_error(value) {
                    return terminal_error(format!("Codex thread start/resume failed: {error}"));
                }
                let Some(thread_id) = value.pointer("/result/thread/id").and_then(Value::as_str)
                else {
                    return terminal_error(
                        "Codex thread response omitted its thread ID".to_owned(),
                    );
                };
                let thread_id = thread_id.to_owned();
                self.state.session_id = Some(thread_id.clone());
                if let Some(model) = value
                    .pointer("/result/thread/model")
                    .and_then(Value::as_str)
                {
                    self.state.record_resolved_model(model);
                }
                ProviderUpdate {
                    events: Vec::new(),
                    outbound: vec![self.turn_request(&thread_id)],
                    terminal: None,
                }
            }
            Some(3) => {
                if let Some(error) = rpc_error(value) {
                    terminal_error(format!("Codex turn start failed: {error}"))
                } else {
                    if let Some(model) = value.pointer("/result/turn/model").and_then(Value::as_str)
                    {
                        self.state.record_resolved_model(model);
                    }
                    ProviderUpdate::empty()
                }
            }
            _ => ProviderUpdate::empty(),
        }
    }

    fn handle_notification(&mut self, value: &Value) -> ProviderUpdate {
        let method = value.get("method").and_then(Value::as_str);
        let params = value.get("params").unwrap_or(&Value::Null);
        let mut events = Vec::new();
        match method {
            Some("thread/started") => {
                if let Some(thread_id) = params.pointer("/thread/id").and_then(Value::as_str) {
                    self.state.session_id = Some(thread_id.to_owned());
                }
                if let Some(model) = params.pointer("/thread/model").and_then(Value::as_str) {
                    self.state.record_resolved_model(model);
                }
            }
            Some("turn/started") => {
                if let Some(model) = params.pointer("/turn/model").and_then(Value::as_str) {
                    self.state.record_resolved_model(model);
                }
                events.push(ChildEvent::StreamStarted);
            }
            Some("item/agentMessage/delta") => {
                if let (Some(id), Some(delta)) = (
                    params.get("itemId").and_then(Value::as_str),
                    params.get("delta").and_then(Value::as_str),
                ) && !delta.is_empty()
                {
                    self.streamed_messages.insert(id.to_owned());
                    self.state.final_text.push_str(delta);
                    events.push(ChildEvent::AgentMessage(delta.to_owned()));
                }
            }
            Some("item/reasoning/textDelta" | "item/reasoning/summaryTextDelta") => {
                if let (Some(id), Some(delta)) = (
                    params.get("itemId").and_then(Value::as_str),
                    params.get("delta").and_then(Value::as_str),
                ) && !delta.is_empty()
                {
                    self.streamed_reasoning.insert(id.to_owned());
                    events.push(ChildEvent::Thought(delta.to_owned()));
                }
            }
            Some("item/commandExecution/outputDelta") => {
                if let (Some(id), Some(delta)) = (
                    params.get("itemId").and_then(Value::as_str),
                    params.get("delta").and_then(Value::as_str),
                ) && !delta.is_empty()
                {
                    self.command_output
                        .entry(id.to_owned())
                        .or_default()
                        .push_str(delta);
                    events.push(ChildEvent::ToolProgress {
                        id: id.to_owned(),
                        delta: delta.to_owned(),
                    });
                }
            }
            Some("item/started") => {
                if let Some(item) = params.get("item") {
                    match item.get("type").and_then(Value::as_str) {
                        // A Codex turn can contain multiple model loops around
                        // tools. Each reasoning/message item start is therefore
                        // a native stream boundary, like Grok's per-loop
                        // `streamStartMs` reset.
                        Some("reasoning" | "agentMessage") => {
                            events.push(ChildEvent::StreamStarted);
                        }
                        _ => {}
                    }
                    if let Some((id, title, kind, raw_input)) = tool(item) {
                        self.state.record_tool(&id, &title);
                        events.push(ChildEvent::ToolStarted {
                            id,
                            title,
                            kind,
                            raw_input,
                        });
                    }
                }
            }
            Some("item/completed") => {
                if let Some(item) = params.get("item") {
                    self.complete_item(item, &mut events);
                }
            }
            Some("error") => {
                if !params
                    .get("willRetry")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    self.state.error_count = self.state.error_count.saturating_add(1);
                }
            }
            Some("thread/tokenUsage/updated") => {
                if let Some(usage) = params.get("tokenUsage") {
                    let tokens_used = usage
                        .pointer("/last/totalTokens")
                        .or_else(|| usage.pointer("/total/totalTokens"))
                        .and_then(Value::as_u64)
                        .unwrap_or_default();
                    let context_window = usage.get("modelContextWindow").and_then(Value::as_u64);
                    self.state.record_usage(tokens_used, context_window);
                }
            }
            Some("turn/completed") => {
                self.state.turns = self.state.turns.saturating_add(1);
                let status = params.pointer("/turn/status").and_then(Value::as_str);
                if status != Some("completed") {
                    self.state.error_count = self.state.error_count.saturating_add(1);
                    let detail = params
                        .pointer("/turn/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or("Codex turn did not complete successfully");
                    return ProviderUpdate {
                        events,
                        outbound: Vec::new(),
                        terminal: Some(Err(detail.to_owned())),
                    };
                }
                return ProviderUpdate {
                    events,
                    outbound: Vec::new(),
                    terminal: Some(Ok(())),
                };
            }
            _ => {}
        }
        ProviderUpdate::events(events)
    }

    fn complete_item(&mut self, item: &Value, events: &mut Vec<ChildEvent>) {
        let item_type = item.get("type").and_then(Value::as_str);
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(item_type.unwrap_or("item"));
        match item_type {
            Some("agentMessage") => {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    self.state.final_text = text.to_owned();
                    if !self.streamed_messages.remove(id) {
                        events.push(ChildEvent::AgentMessage(text.to_owned()));
                    }
                }
            }
            Some("reasoning") => {
                if !self.streamed_reasoning.remove(id)
                    && let Some(text) = reasoning_text(item)
                {
                    events.push(ChildEvent::Thought(text));
                }
            }
            Some("commandExecution" | "fileChange" | "mcpToolCall" | "dynamicToolCall") => {
                if let Some((id, title, kind, raw_input)) = tool(item) {
                    if !self.state.knows_tool(&id) {
                        self.state.record_tool(&id, &title);
                        events.push(ChildEvent::ToolStarted {
                            id: id.clone(),
                            title: title.clone(),
                            kind,
                            raw_input,
                        });
                    }
                    let success = tool_success(item);
                    if !success {
                        self.state.error_count = self.state.error_count.saturating_add(1);
                    }
                    let output =
                        tool_output(item).or_else(|| self.command_output.get(&id).cloned());
                    let raw_output = (kind == acp::ToolKind::Execute)
                        .then(|| bash_output(item, output.as_deref().unwrap_or_default()));
                    self.command_output.remove(&id);
                    events.push(ChildEvent::ToolFinished {
                        id,
                        title: Some(title),
                        output,
                        raw_output,
                        success,
                    });
                }
            }
            _ => {}
        }
    }
}

impl ExternalProviderSession for CodexAppServerSession {
    fn program(&self) -> &'static str {
        "codex"
    }

    fn args(&self) -> &[String] {
        &self.args
    }

    fn initial_input(&mut self) -> Vec<String> {
        vec![Self::initialize_request()]
    }

    fn handle_line(&mut self, line: &str) -> ProviderUpdate {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return ProviderUpdate::empty();
        };
        if value.get("id").is_some() {
            self.handle_response(&value)
        } else {
            self.handle_notification(&value)
        }
    }

    fn state(&self) -> &ProviderState {
        &self.state
    }
}

fn json_line(value: Value) -> String {
    value.to_string()
}

fn terminal_error(error: String) -> ProviderUpdate {
    ProviderUpdate {
        events: Vec::new(),
        outbound: Vec::new(),
        terminal: Some(Err(error)),
    }
}

fn rpc_error(value: &Value) -> Option<String> {
    let error = value.get("error")?;
    error
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| Some(error.to_string()))
}

fn tool(item: &Value) -> Option<(String, String, acp::ToolKind, Option<Value>)> {
    let item_type = item.get("type")?.as_str()?;
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(item_type)
        .to_owned();
    match item_type {
        "commandExecution" => {
            let command = item
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("command");
            Some((
                id,
                command.to_owned(),
                acp::ToolKind::Execute,
                Some(json!({"command": command})),
            ))
        }
        "fileChange" => Some((
            id,
            "Apply file changes".to_owned(),
            acp::ToolKind::Edit,
            item.get("changes").cloned(),
        )),
        "mcpToolCall" => {
            let server = item.get("server").and_then(Value::as_str).unwrap_or("mcp");
            let name = item.get("tool").and_then(Value::as_str).unwrap_or("tool");
            Some((
                id,
                format!("{server}: {name}"),
                acp::ToolKind::Other,
                item.get("arguments").cloned(),
            ))
        }
        "dynamicToolCall" => {
            let name = item.get("tool").and_then(Value::as_str).unwrap_or("tool");
            Some((
                id,
                name.to_owned(),
                acp::ToolKind::Other,
                item.get("arguments").cloned(),
            ))
        }
        _ => None,
    }
}

fn tool_success(item: &Value) -> bool {
    if item.get("error").is_some_and(|error| !error.is_null()) {
        return false;
    }
    !matches!(
        item.get("status").and_then(Value::as_str),
        Some("failed" | "declined")
    ) && item
        .get("exitCode")
        .and_then(Value::as_i64)
        .is_none_or(|code| code == 0)
}

fn tool_output(item: &Value) -> Option<String> {
    for key in ["aggregatedOutput", "output", "result", "error"] {
        if let Some(value) = item.get(key)
            && let Some(text) = json_text(value)
        {
            return Some(text);
        }
    }
    item.get("changes").and_then(json_text)
}

fn reasoning_text(item: &Value) -> Option<String> {
    let values = item
        .get("summary")
        .and_then(Value::as_array)
        .or_else(|| item.get("content").and_then(Value::as_array))?;
    let text = values
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

fn json_text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| (!value.is_null()).then(|| value.to_string()))
}

fn bash_output(item: &Value, output: &str) -> Value {
    let command = item
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let exit_code = item
        .get("exitCode")
        .and_then(Value::as_i64)
        .unwrap_or(if tool_success(item) { 0 } else { -1 });
    json!({
        "type": "Bash",
        "output": output.as_bytes(),
        "output_for_prompt": output,
        "exit_code": exit_code,
        "command": command,
        "truncated": false,
        "signal": null,
        "timed_out": false,
        "description": null,
        "current_dir": item.get("cwd").and_then(Value::as_str).unwrap_or_default(),
        "output_file": "",
        "total_bytes": output.len(),
        "was_bare_echo": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation<'a>() -> ProviderInvocation<'a> {
        ProviderInvocation {
            prompt: "inspect repository",
            capability: SubagentCapabilityMode::All,
            model: None,
            reasoning_effort: Some("xhigh"),
            resume_session_id: None,
        }
    }

    fn session() -> CodexAppServerSession {
        CodexAppServerSession::new(invocation(), Path::new("/tmp"))
    }

    #[test]
    fn defaults_to_policy_model_and_sends_it_to_thread_and_turn() {
        let mut session = session();
        assert_eq!(session.state.effective_model, "gpt-5.6-sol");
        let thread = session.handle_line(r#"{"id":1,"result":{}}"#);
        assert!(thread.outbound[1].contains("\"model\":\"gpt-5.6-sol\""));
        let turn = session.handle_line(r#"{"id":2,"result":{"thread":{"id":"codex-session"}}}"#);
        assert!(turn.outbound[0].contains("\"model\":\"gpt-5.6-sol\""));
    }

    #[test]
    fn explicit_model_override_wins_and_wire_model_can_refine_state() {
        let mut invocation = invocation();
        invocation.model = Some("gpt-5.5-codex");
        let mut session = CodexAppServerSession::new(invocation, Path::new("/tmp"));
        assert_eq!(session.state.effective_model, "gpt-5.5-codex");
        let thread = session.handle_line(
            r#"{"id":2,"result":{"thread":{"id":"codex-session","model":"gpt-5.5-codex-resolved"}}}"#,
        );
        assert!(thread.outbound[0].contains("\"model\":\"gpt-5.5-codex\""));
        assert_eq!(session.state.effective_model, "gpt-5.5-codex-resolved");
    }

    #[test]
    fn app_server_handshake_preserves_sandbox_and_effort() {
        let mut session = session();
        assert_eq!(session.args(), ["app-server", "--stdio"]);
        assert!(session.initial_input()[0].contains("\"initialize\""));
        let thread = session.handle_line(r#"{"id":1,"result":{}}"#);
        assert_eq!(thread.outbound.len(), 2);
        assert!(thread.outbound[1].contains("workspace-write"));
        let turn = session.handle_line(r#"{"id":2,"result":{"thread":{"id":"codex-session"}}}"#);
        assert!(turn.outbound[0].contains("\"effort\":\"xhigh\""));
        assert_eq!(session.state.session_id.as_deref(), Some("codex-session"));
    }

    #[test]
    fn streams_message_reasoning_and_command_output() {
        let mut session = session();
        let started = session.handle_line(r#"{"method":"turn/started","params":{}}"#);
        assert!(matches!(started.events[0], ChildEvent::StreamStarted));
        let message = session.handle_line(
            r#"{"method":"item/agentMessage/delta","params":{"itemId":"msg-1","delta":"hello"}}"#,
        );
        assert!(matches!(&message.events[0], ChildEvent::AgentMessage(text) if text == "hello"));
        let thought = session.handle_line(
            r#"{"method":"item/reasoning/summaryTextDelta","params":{"itemId":"why-1","delta":"checking"}}"#,
        );
        assert!(matches!(&thought.events[0], ChildEvent::Thought(text) if text == "checking"));
        let output = session.handle_line(
            r#"{"method":"item/commandExecution/outputDelta","params":{"itemId":"cmd-1","delta":"ok\n"}}"#,
        );
        assert!(
            matches!(&output.events[0], ChildEvent::ToolProgress { id, delta } if id == "cmd-1" && delta == "ok\n")
        );
    }

    #[test]
    fn item_starts_create_per_loop_native_stream_boundaries() {
        let mut session = session();
        let reasoning = session.handle_line(
            r#"{"method":"item/started","params":{"item":{"id":"why-1","type":"reasoning","summary":[],"content":[]}}}"#,
        );
        assert!(matches!(reasoning.events[0], ChildEvent::StreamStarted));
        let message = session.handle_line(
            r#"{"method":"item/started","params":{"item":{"id":"msg-1","type":"agentMessage","text":"","phase":"commentary"}}}"#,
        );
        assert!(matches!(message.events[0], ChildEvent::StreamStarted));
    }

    #[test]
    fn completed_items_do_not_repeat_streamed_text() {
        let mut session = session();
        session.handle_line(
            r#"{"method":"item/agentMessage/delta","params":{"itemId":"msg-1","delta":"hello"}}"#,
        );
        let completed = session.handle_line(
            r#"{"method":"item/completed","params":{"item":{"type":"agentMessage","id":"msg-1","text":"hello"}}}"#,
        );
        assert!(completed.events.is_empty());
        assert_eq!(session.state.final_text, "hello");
    }

    #[test]
    fn live_wire_usage_and_turn_completion_are_terminal() {
        let mut session = session();
        let usage = session.handle_line(
            r#"{"method":"thread/tokenUsage/updated","params":{"threadId":"thread-1","turnId":"turn-1","tokenUsage":{"total":{"totalTokens":20442,"inputTokens":20437,"cachedInputTokens":9984,"outputTokens":5,"reasoningOutputTokens":0},"last":{"totalTokens":20442,"inputTokens":20437,"cachedInputTokens":9984,"outputTokens":5,"reasoningOutputTokens":0},"modelContextWindow":258400}}}"#,
        );
        assert!(usage.terminal.is_none());
        assert_eq!(session.state.tokens_used, 20_442);
        assert_eq!(session.state.context_window_tokens, 258_400);
        assert_eq!(session.state.context_usage_pct(), 7);

        let completed = session.handle_line(
            r#"{"method":"turn/completed","params":{"threadId":"thread-1","turn":{"id":"turn-1","items":[],"itemsView":"notLoaded","status":"completed","error":null,"startedAt":1784454133,"completedAt":1784454139,"durationMs":5427}}}"#,
        );
        assert!(matches!(completed.terminal, Some(Ok(()))));
        assert_eq!(session.state.turns, 1);
    }

    #[test]
    fn translates_true_tool_lifecycle() {
        let mut session = session();
        let started = session.handle_line(
            r#"{"method":"item/started","params":{"item":{"id":"cmd-1","type":"commandExecution","command":"cargo test","commandActions":[],"cwd":"/tmp","status":"inProgress"}}}"#,
        );
        assert!(
            matches!(&started.events[0], ChildEvent::ToolStarted { id, kind: acp::ToolKind::Execute, .. } if id == "cmd-1")
        );
        let completed = session.handle_line(
            r#"{"method":"item/completed","params":{"item":{"id":"cmd-1","type":"commandExecution","command":"cargo test","commandActions":[],"cwd":"/tmp","status":"completed","exitCode":0,"aggregatedOutput":"ok"}}}"#,
        );
        assert!(
            matches!(&completed.events[0], ChildEvent::ToolFinished { id, success: true, raw_output: Some(_), .. } if id == "cmd-1")
        );
        assert_eq!(session.state.tool_calls, 1);
    }
}
