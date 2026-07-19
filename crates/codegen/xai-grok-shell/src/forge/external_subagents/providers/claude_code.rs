use std::collections::HashSet;
use std::path::Path;

use agent_client_protocol as acp;
use serde_json::Value;
use xai_tool_types::SubagentCapabilityMode;

use crate::forge::subagent_ui::ChildEvent;

use super::{
    ExternalProvider, ExternalProviderSession, ProviderInvocation, ProviderState, ProviderUpdate,
};

pub(crate) const CLAUDE_CODE_TYPE: &str = "claude-code";
/// Full provider model ID observed in Claude Code's `modelUsage` response.
/// The CLI receives this exact ID; the adapter derives a concise TUI label.
const DEFAULT_MODEL: &str = "claude-opus-4-8";
const GLOBAL_OPUS_MODEL_CLASS: &str = "openrouter-opus";
pub(crate) static CLAUDE_CODE: ClaudeCodeProvider = ClaudeCodeProvider;

pub(crate) struct ClaudeCodeProvider;

impl ExternalProvider for ClaudeCodeProvider {
    fn subagent_type(&self) -> &'static str {
        CLAUDE_CODE_TYPE
    }

    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    fn program(&self) -> &'static str {
        "claude"
    }

    fn create_session(
        &self,
        invocation: ProviderInvocation<'_>,
        _cwd: &Path,
    ) -> Box<dyn ExternalProviderSession> {
        let provider_model = provider_model(invocation.model);
        Box::new(ClaudeCodeSession {
            args: build_args(invocation, &provider_model),
            state: ProviderState::new(display_model(&provider_model)),
            partial_messages: HashSet::new(),
            partial_blocks: HashSet::new(),
        })
    }
}

struct ClaudeCodeSession {
    args: Vec<String>,
    state: ProviderState,
    /// Message IDs with at least one text/thinking delta. The later full
    /// `assistant` message remains useful for tools but must not repeat text.
    partial_messages: HashSet<String>,
    /// Claude stream event blocks do not always carry a message ID. Track the
    /// current block index too so suppression works across CLI versions.
    partial_blocks: HashSet<u64>,
}

impl ExternalProviderSession for ClaudeCodeSession {
    fn program(&self) -> &'static str {
        "claude"
    }

    fn args(&self) -> &[String] {
        &self.args
    }

    fn handle_line(&mut self, line: &str) -> ProviderUpdate {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return ProviderUpdate::empty();
        };
        ProviderUpdate::events(self.parse_event(&value))
    }

    fn state(&self) -> &ProviderState {
        &self.state
    }
}

impl ClaudeCodeSession {
    fn parse_event(&mut self, value: &Value) -> Vec<ChildEvent> {
        if let Some(session_id) = value.get("session_id").and_then(Value::as_str) {
            self.state.session_id = Some(session_id.to_owned());
        }
        let mut events = Vec::new();
        match value.get("type").and_then(Value::as_str) {
            Some("stream_event") => self.parse_stream_event(value, &mut events),
            Some("assistant") => self.parse_assistant(value, &mut events),
            Some("user") => self.parse_user(value, &mut events),
            Some("result") => {
                if let Some(result) = value.get("result").and_then(Value::as_str) {
                    self.state.final_text = result.to_owned();
                }
                self.parse_usage(value);
                if value
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    self.state.error_count = self.state.error_count.saturating_add(1);
                }
            }
            _ => {}
        }
        events
    }

    fn parse_stream_event(&mut self, value: &Value, events: &mut Vec<ChildEvent>) {
        let Some(event) = value.get("event") else {
            return;
        };
        let event_type = event.get("type").and_then(Value::as_str);
        if event_type == Some("message_start") {
            if let Some(message_id) = event.pointer("/message/id").and_then(Value::as_str) {
                self.partial_messages.remove(message_id);
            }
            events.push(ChildEvent::StreamStarted);
            return;
        }
        if event_type == Some("content_block_start") {
            let Some(block) = event.get("content_block") else {
                return;
            };
            if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("claude-tool")
                    .to_owned();
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Tool")
                    .to_owned();
                if !self.state.knows_tool(&id) {
                    self.state.record_tool(&id, &name);
                    events.push(ChildEvent::ToolStarted {
                        id,
                        title: name.clone(),
                        kind: tool_kind(&name),
                        raw_input: block.get("input").cloned(),
                    });
                }
            }
            return;
        }
        if event_type != Some("content_block_delta") {
            return;
        }
        let block_index = event.get("index").and_then(Value::as_u64);
        let message_id = stream_message_id(value, event);
        let Some(delta) = event.get("delta") else {
            return;
        };
        let child_event = match delta.get("type").and_then(Value::as_str) {
            Some("text_delta") => delta
                .get("text")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(|text| {
                    self.state.final_text.push_str(text);
                    ChildEvent::AgentMessage(text.to_owned())
                }),
            Some("thinking_delta") => delta
                .get("thinking")
                .or_else(|| delta.get("text"))
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(|text| ChildEvent::Thought(text.to_owned())),
            _ => None,
        };
        if let Some(event) = child_event {
            if let Some(message_id) = message_id {
                self.partial_messages.insert(message_id.to_owned());
            }
            if let Some(index) = block_index {
                self.partial_blocks.insert(index);
            }
            events.push(event);
        }
    }

    fn parse_assistant(&mut self, value: &Value, events: &mut Vec<ChildEvent>) {
        self.state.turns = self.state.turns.saturating_add(1);
        let message_id = value.pointer("/message/id").and_then(Value::as_str);
        let message_had_partials = message_id.is_some_and(|id| self.partial_messages.remove(id));
        if let Some(content) = value.pointer("/message/content").and_then(Value::as_array) {
            for (index, item) in content.iter().enumerate() {
                let block_had_partials = self.partial_blocks.remove(&(index as u64));
                match item.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(text) = item.get("text").and_then(Value::as_str) {
                            self.state.final_text = text.to_owned();
                            if !message_had_partials && !block_had_partials {
                                events.push(ChildEvent::AgentMessage(text.to_owned()));
                            }
                        }
                    }
                    Some("thinking") => {
                        if !message_had_partials
                            && !block_had_partials
                            && let Some(text) = item.get("thinking").and_then(Value::as_str)
                        {
                            events.push(ChildEvent::Thought(text.to_owned()));
                        }
                    }
                    Some("tool_use") => {
                        let id = item
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("claude-tool")
                            .to_owned();
                        let name = item
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("Tool")
                            .to_owned();
                        if !self.state.knows_tool(&id) {
                            self.state.record_tool(&id, &name);
                            events.push(ChildEvent::ToolStarted {
                                id,
                                title: name.clone(),
                                kind: tool_kind(&name),
                                raw_input: item.get("input").cloned(),
                            });
                        } else if let Some(raw_input) = item.get("input").cloned() {
                            events.push(ChildEvent::ToolUpdated { id, raw_input });
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn parse_user(&mut self, value: &Value, events: &mut Vec<ChildEvent>) {
        if let Some(content) = value.pointer("/message/content").and_then(Value::as_array) {
            for item in content
                .iter()
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("tool_result"))
            {
                let id = item
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .unwrap_or("claude-tool")
                    .to_owned();
                let success = !item
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if !success {
                    self.state.error_count = self.state.error_count.saturating_add(1);
                }
                events.push(ChildEvent::ToolFinished {
                    id,
                    title: None,
                    output: item.get("content").and_then(json_text),
                    raw_output: None,
                    success,
                });
            }
        }
    }

    fn parse_usage(&mut self, value: &Value) {
        let usage = value.get("usage").unwrap_or(&Value::Null);
        // Claude's cache-read and cache-creation tokens are part of the live
        // prompt context even though `input_tokens` reports only the uncached
        // portion for billing.
        let tokens_used = [
            "input_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
            "output_tokens",
        ]
        .iter()
        .filter_map(|key| usage.get(*key).and_then(Value::as_u64))
        .fold(0u64, u64::saturating_add);
        let model_usage = value.get("modelUsage").and_then(Value::as_object);
        if let Some(model) = model_usage
            .filter(|models| models.len() == 1)
            .and_then(|models| models.keys().next())
        {
            self.state.record_resolved_model(&display_model(model));
        }
        let context_window = model_usage
            .and_then(|models| models.values().find_map(|model| model.get("contextWindow")))
            .and_then(Value::as_u64);
        self.state.record_usage(tokens_used, context_window);
    }
}

fn stream_message_id<'a>(outer: &'a Value, event: &'a Value) -> Option<&'a str> {
    outer
        .get("message_id")
        .or_else(|| outer.get("messageId"))
        .or_else(|| event.get("message_id"))
        .or_else(|| event.get("messageId"))
        .and_then(Value::as_str)
}

fn provider_model(model: Option<&str>) -> String {
    match model.unwrap_or(DEFAULT_MODEL) {
        GLOBAL_OPUS_MODEL_CLASS => DEFAULT_MODEL.to_owned(),
        model => model.to_owned(),
    }
}

fn display_model(provider_model: &str) -> String {
    let concise = provider_model
        .strip_prefix("claude-")
        .unwrap_or(provider_model);
    let mut parts = concise.rsplitn(3, '-');
    let minor = parts.next();
    let major = parts.next();
    let family = parts.next();
    match (family, major, minor) {
        (Some(family), Some(major), Some(minor))
            if major.chars().all(|ch| ch.is_ascii_digit())
                && minor.chars().all(|ch| ch.is_ascii_digit()) =>
        {
            format!("{family}-{major}.{minor}")
        }
        _ => concise.to_owned(),
    }
}

fn build_args(invocation: ProviderInvocation<'_>, provider_model: &str) -> Vec<String> {
    let mut args = vec![
        "--print".to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
        "--include-partial-messages".to_owned(),
        "--forward-subagent-text".to_owned(),
        "--permission-mode".to_owned(),
        "dontAsk".to_owned(),
        "--allowedTools".to_owned(),
        allowed_tools(invocation.capability).to_owned(),
    ];
    args.extend(["--model".to_owned(), provider_model.to_owned()]);
    if let Some(effort) = invocation.reasoning_effort {
        args.extend(["--effort".to_owned(), effort.to_owned()]);
    }
    if let Some(session_id) = invocation.resume_session_id {
        args.extend(["--resume".to_owned(), session_id.to_owned()]);
    }
    // `--allowedTools` accepts a variadic list, so terminate option parsing
    // before the positional prompt.
    args.extend(["--".to_owned(), invocation.prompt.to_owned()]);
    args
}

fn allowed_tools(capability: SubagentCapabilityMode) -> &'static str {
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

fn tool_kind(name: &str) -> acp::ToolKind {
    match name {
        "Read" => acp::ToolKind::Read,
        "Glob" | "Grep" | "WebSearch" => acp::ToolKind::Search,
        "Edit" | "Write" | "NotebookEdit" => acp::ToolKind::Edit,
        "Bash" => acp::ToolKind::Execute,
        "WebFetch" => acp::ToolKind::Fetch,
        _ => acp::ToolKind::Other,
    }
}

fn json_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_owned());
    }
    if let Some(items) = value.as_array() {
        let text = items
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return Some(text);
        }
    }
    (!value.is_null()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation<'a>(prompt: &'a str, model: Option<&'a str>) -> ProviderInvocation<'a> {
        ProviderInvocation {
            prompt,
            capability: SubagentCapabilityMode::ReadOnly,
            model,
            reasoning_effort: Some("high"),
            resume_session_id: None,
        }
    }

    fn session() -> ClaudeCodeSession {
        let invocation = invocation("inspect repository", None);
        let provider_model = provider_model(invocation.model);
        ClaudeCodeSession {
            args: build_args(invocation, &provider_model),
            state: ProviderState::new(display_model(&provider_model)),
            partial_messages: HashSet::new(),
            partial_blocks: HashSet::new(),
        }
    }

    #[test]
    fn args_enable_partial_messages_and_bound_prompt() {
        let invocation = invocation("inspect repository", None);
        let provider_model = provider_model(invocation.model);
        let args = build_args(invocation, &provider_model);
        assert!(args.iter().any(|arg| arg == "--include-partial-messages"));
        assert!(!args.iter().any(|arg| arg.contains("dangerously")));
        assert!(args.windows(2).any(|pair| pair == ["--effort", "high"]));
        assert_eq!(&args[args.len() - 2..], ["--", "inspect repository"]);
    }

    #[test]
    fn defaults_to_full_provider_model_id() {
        let session = CLAUDE_CODE.create_session(invocation("inspect", None), Path::new("/tmp"));
        assert_eq!(session.state().effective_model, "opus-4.8");
        assert!(
            session
                .args()
                .windows(2)
                .any(|pair| pair == ["--model", "claude-opus-4-8"])
        );
    }

    #[test]
    fn normalizes_only_global_opus_class_and_preserves_other_overrides() {
        let alias = CLAUDE_CODE.create_session(
            invocation("inspect", Some("openrouter-opus")),
            Path::new("/tmp"),
        );
        assert_eq!(alias.state().effective_model, "opus-4.8");
        assert!(
            alias
                .args()
                .windows(2)
                .any(|pair| pair == ["--model", "claude-opus-4-8"])
        );

        let explicit = CLAUDE_CODE.create_session(
            invocation("inspect", Some("claude-sonnet-4-6")),
            Path::new("/tmp"),
        );
        assert_eq!(explicit.state().effective_model, "sonnet-4.6");
        assert!(
            explicit
                .args()
                .windows(2)
                .any(|pair| pair == ["--model", "claude-sonnet-4-6"])
        );
    }

    #[test]
    fn streams_text_and_thinking_without_repeating_full_message() {
        let mut session = session();
        let text = session.parse_event(&serde_json::json!({
            "type": "stream_event",
            "message_id": "msg-1",
            "event": {"type": "content_block_delta", "index": 0,
                "delta": {"type": "text_delta", "text": "Work"}}
        }));
        assert!(matches!(&text[0], ChildEvent::AgentMessage(value) if value == "Work"));
        let thinking = session.parse_event(&serde_json::json!({
            "type": "stream_event",
            "message_id": "msg-1",
            "event": {"type": "content_block_delta", "index": 1,
                "delta": {"type": "thinking_delta", "thinking": "inspect"}}
        }));
        assert!(matches!(&thinking[0], ChildEvent::Thought(value) if value == "inspect"));

        let full = session.parse_event(&serde_json::json!({
            "type": "assistant",
            "message": {"id": "msg-1", "content": [
                {"type": "text", "text": "Working"},
                {"type": "thinking", "thinking": "inspect first"},
                {"type": "tool_use", "id": "tool-1", "name": "Read", "input": {"file_path": "README.md"}}
            ]}
        }));
        assert_eq!(full.len(), 1);
        assert!(
            matches!(&full[0], ChildEvent::ToolStarted { id, kind: acp::ToolKind::Read, .. } if id == "tool-1")
        );
        assert_eq!(session.state.final_text, "Working");
    }

    #[test]
    fn result_normalizes_live_context_usage() {
        let mut session = session();
        session.parse_event(&serde_json::json!({
            "type": "result",
            "is_error": false,
            "result": "OK",
            "usage": {
                "input_tokens": 2,
                "cache_creation_input_tokens": 3552,
                "cache_read_input_tokens": 9120,
                "output_tokens": 4
            },
            "modelUsage": {
                "claude-opus-4-8": {"contextWindow": 1000000}
            }
        }));
        assert_eq!(session.state.tokens_used, 12_678);
        assert_eq!(session.state.context_window_tokens, 1_000_000);
        assert_eq!(session.state.context_usage_pct(), 1);
        assert_eq!(session.state.effective_model, "opus-4.8");
    }

    #[test]
    fn translates_full_transcript_when_no_partial_events_arrive() {
        let mut session = session();
        let events = session.parse_event(&serde_json::json!({
            "type": "assistant",
            "session_id": "claude-session",
            "message": {"content": [
                {"type": "thinking", "thinking": "inspect first"},
                {"type": "text", "text": "Working"}
            ]}
        }));
        assert!(matches!(&events[0], ChildEvent::Thought(text) if text == "inspect first"));
        assert!(matches!(&events[1], ChildEvent::AgentMessage(text) if text == "Working"));
        assert_eq!(session.state.session_id.as_deref(), Some("claude-session"));
    }
}
