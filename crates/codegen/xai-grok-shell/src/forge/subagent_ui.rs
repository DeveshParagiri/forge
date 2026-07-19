//! Native TUI lifecycle bridge for Forge external CLI subagents.
//!
//! This module deliberately owns all provider-neutral UI translation for the
//! Claude Code and Codex adapters. Core tool notification schemas remain
//! untouched; the only integration seam is construction from the session's
//! existing gateway/persistence handles.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex as StdMutex};

use agent_client_protocol as acp;
use xai_acp_lib::AcpAgentGatewaySender as GatewaySender;

use crate::extensions::notification::{
    SessionNotification as XaiSessionNotification, SessionUpdate as XaiSessionUpdate,
};
use crate::session::persistence::PersistenceMsg;

#[derive(Clone)]
pub(crate) struct ExternalSubagentUi {
    parent_session_id: String,
    parent_cwd: std::path::PathBuf,
    gateway: GatewaySender,
    persistence_tx: tokio::sync::mpsc::UnboundedSender<PersistenceMsg>,
    /// Native AgentView uses these timestamps to establish a live stream
    /// boundary, animate running thought/tool blocks, and show thought time.
    child_streams: Arc<StdMutex<HashMap<String, ChildStreamMeta>>>,
}

#[derive(Debug, Clone, Copy)]
struct ChildStreamMeta {
    turn_start_ms: i64,
    stream_start_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ChildEvent {
    /// Starts a fresh native streaming interval. The empty chunk is not shown,
    /// but its changed stream timestamp activates the stock thinking state.
    StreamStarted,
    AgentMessage(String),
    Thought(String),
    ToolStarted {
        id: String,
        title: String,
        kind: acp::ToolKind,
        raw_input: Option<serde_json::Value>,
    },
    ToolUpdated {
        id: String,
        raw_input: serde_json::Value,
    },
    ToolProgress {
        id: String,
        delta: String,
    },
    ToolFinished {
        id: String,
        title: Option<String>,
        output: Option<String>,
        raw_output: Option<serde_json::Value>,
        success: bool,
    },
}

fn child_update(event: ChildEvent) -> acp::SessionUpdate {
    match event {
        ChildEvent::StreamStarted => acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(
            acp::ContentBlock::Text(acp::TextContent::new(String::new())),
        )),
        ChildEvent::AgentMessage(text) => acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(text))),
        ),
        ChildEvent::Thought(text) => acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(
            acp::ContentBlock::Text(acp::TextContent::new(text)),
        )),
        ChildEvent::ToolStarted {
            id,
            title,
            kind,
            raw_input,
        } => {
            let call = acp::ToolCall::new(acp::ToolCallId::new(id), title)
                .kind(kind)
                .status(acp::ToolCallStatus::InProgress)
                .content(vec![])
                .locations(vec![])
                .raw_input(raw_input);
            acp::SessionUpdate::ToolCall(call)
        }
        ChildEvent::ToolUpdated { id, raw_input } => {
            acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                acp::ToolCallId::new(id),
                acp::ToolCallUpdateFields::new()
                    .status(Some(acp::ToolCallStatus::InProgress))
                    .raw_input(Some(raw_input)),
            ))
        }
        ChildEvent::ToolProgress { id, delta } => {
            let raw_output = serde_json::json!({
                "type": "Bash",
                "output": [],
                "output_for_prompt": "",
                "exit_code": 0,
                "command": "",
                "truncated": false,
                "signal": null,
                "timed_out": false,
                "description": null,
                "current_dir": "",
                "output_file": "",
                "total_bytes": delta.len(),
                "output_delta": delta.as_bytes(),
                "was_bare_echo": false
            });
            acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                acp::ToolCallId::new(id),
                acp::ToolCallUpdateFields::new()
                    .status(Some(acp::ToolCallStatus::InProgress))
                    .raw_output(Some(raw_output)),
            ))
        }
        ChildEvent::ToolFinished {
            id,
            title,
            output,
            raw_output,
            success,
        } => {
            let content = output.map(|text| {
                vec![acp::ToolCallContent::from(acp::ContentBlock::Text(
                    acp::TextContent::new(text),
                ))]
            });
            acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                acp::ToolCallId::new(id),
                acp::ToolCallUpdateFields::new()
                    .status(Some(if success {
                        acp::ToolCallStatus::Completed
                    } else {
                        acp::ToolCallStatus::Failed
                    }))
                    .title(title)
                    .content(content)
                    .raw_output(raw_output),
            ))
        }
    }
}

impl ExternalSubagentUi {
    pub(crate) fn parent_session_id(&self) -> &str {
        &self.parent_session_id
    }

    pub(crate) fn parent_cwd(&self) -> &Path {
        &self.parent_cwd
    }

    pub(crate) fn new(
        parent_session_id: String,
        parent_cwd: std::path::PathBuf,
        gateway: GatewaySender,
        persistence_tx: tokio::sync::mpsc::UnboundedSender<PersistenceMsg>,
    ) -> Self {
        Self {
            parent_session_id,
            parent_cwd,
            gateway,
            persistence_tx,
            child_streams: Arc::default(),
        }
    }

    pub(crate) fn spawned(
        &self,
        id: &str,
        subagent_type: &str,
        description: &str,
        prompt: &str,
        cwd: &Path,
        parent_prompt_id: Option<String>,
        capability_mode: Option<String>,
        model: Option<String>,
        resumed_from: Option<String>,
        display_name: &str,
    ) {
        self.emit_parent(
            XaiSessionUpdate::SubagentSpawned {
                subagent_id: id.to_owned(),
                parent_session_id: self.parent_session_id.clone(),
                parent_prompt_id,
                child_session_id: id.to_owned(),
                subagent_type: subagent_type.to_owned(),
                description: description.to_owned(),
                effective_context_source: Some(if resumed_from.is_some() {
                    "resumed".to_owned()
                } else {
                    "new".to_owned()
                }),
                context_normalized: false,
                capability_mode,
                persona: None,
                role: Some(display_name.to_owned()),
                model,
                resumed_from,
            },
            true,
        );
        // The native child view normally enriches these fields from meta.json.
        // External sessions have no shell-owned child directory, so seed the
        // visible task prompt directly through the child ACP stream.
        self.emit_child(
            id,
            acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new(prompt.to_owned()),
            ))),
            true,
            false,
        );
        self.child_event(id, ChildEvent::StreamStarted);
        tracing::debug!(subagent_id = id, cwd = %cwd.display(), "external native UI spawned");
    }

    pub(crate) fn progress(
        &self,
        id: &str,
        duration_ms: u64,
        turns: u32,
        tool_calls: u32,
        tokens_used: u64,
        context_window_tokens: u64,
        context_usage_pct: u8,
        tools_used: Vec<String>,
        error_count: u32,
    ) {
        self.emit_parent(
            XaiSessionUpdate::SubagentProgress {
                subagent_id: id.to_owned(),
                parent_session_id: self.parent_session_id.clone(),
                child_session_id: id.to_owned(),
                duration_ms,
                turn_count: turns,
                tool_call_count: tool_calls,
                tokens_used,
                context_window_tokens,
                context_usage_pct,
                tools_used,
                error_count,
            },
            false,
        );
    }

    /// External children have no native session actor/fs watcher, so seed the
    /// stock child header with the same git-head notification native sessions
    /// use. The pager remains provider-agnostic.
    pub(crate) async fn workspace(&self, id: &str, cwd: &Path) {
        let branch = xai_grok_workspace::session::git::get_branch(cwd).await;
        let worktree = xai_grok_workspace::session::git::get_worktree_info(cwd).await;
        let (is_worktree, main_repo) = worktree.unwrap_or((false, None));
        let params = xai_grok_workspace::session::git::GitHeadChanged {
            session_id: id.to_owned(),
            branch,
            is_worktree,
            main_repo,
        };
        if let Ok(raw) = serde_json::value::to_raw_value(&params) {
            self.gateway
                .forward_fire_and_forget(acp::ExtNotification::new(
                    "x.ai/git_head_changed",
                    raw.into(),
                ));
        }
    }

    pub(crate) fn child_event(&self, id: &str, event: ChildEvent) {
        let starts_stream = matches!(event, ChildEvent::StreamStarted);
        self.emit_child(id, child_update(event), true, starts_stream);
    }

    pub(crate) fn finished(
        &self,
        id: &str,
        status: &str,
        error: Option<String>,
        tool_calls: u32,
        turns: u32,
        duration_ms: u64,
        tokens_used: u64,
        output: Option<String>,
    ) {
        self.child_streams
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(id);
        self.emit_parent(
            XaiSessionUpdate::SubagentFinished {
                subagent_id: id.to_owned(),
                child_session_id: id.to_owned(),
                status: status.to_owned(),
                error,
                tool_calls,
                turns,
                duration_ms,
                tokens_used,
                output,
                will_wake: false,
            },
            true,
        );
    }

    fn emit_parent(&self, update: XaiSessionUpdate, persist: bool) {
        let mut meta = None;
        crate::util::event_id::ensure_event_id_meta(&self.parent_session_id, &mut meta);
        let notification = XaiSessionNotification {
            session_id: acp::SessionId::new(self.parent_session_id.clone()),
            update,
            meta: meta.map(serde_json::Value::Object),
        };
        if persist {
            let _ = self.persistence_tx.send(PersistenceMsg::Update(
                crate::session::storage::SessionUpdate::Xai(Box::new(notification.clone())),
            ));
        }
        if let Ok(raw) = serde_json::to_value(&notification)
            .and_then(|value| serde_json::value::to_raw_value(&value))
        {
            self.gateway
                .forward_fire_and_forget(acp::ExtNotification::new(
                    "x.ai/session_notification",
                    raw.into(),
                ));
        }
    }

    fn emit_child(
        &self,
        child_id: &str,
        update: acp::SessionUpdate,
        persist: bool,
        starts_stream: bool,
    ) {
        self.emit_child_with_tokens(child_id, update, persist, starts_stream, None);
    }

    pub(crate) fn context(&self, child_id: &str, tokens_used: u64) {
        // A metadata-only empty chunk is invisible but follows the native
        // `totalTokens` path that drives the child context bar.
        self.emit_child_with_tokens(
            child_id,
            acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new(String::new()),
            ))),
            false,
            false,
            Some(tokens_used),
        );
    }

    fn emit_child_with_tokens(
        &self,
        child_id: &str,
        update: acp::SessionUpdate,
        persist: bool,
        starts_stream: bool,
        tokens_used: Option<u64>,
    ) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let stream = {
            let mut streams = self
                .child_streams
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let entry = streams
                .entry(child_id.to_owned())
                .or_insert(ChildStreamMeta {
                    turn_start_ms: now_ms,
                    stream_start_ms: now_ms,
                });
            if starts_stream {
                // Guarantee a changed boundary even when two provider events
                // arrive within the same wall-clock millisecond.
                entry.stream_start_ms = now_ms.max(entry.stream_start_ms.saturating_add(1));
            }
            *entry
        };
        let mut notification =
            acp::SessionNotification::new(acp::SessionId::new(child_id.to_owned()), update);
        crate::util::event_id::ensure_event_id_meta(child_id, &mut notification.meta);
        let meta = notification.meta.get_or_insert_default();
        meta.insert("agentTimestampMs".to_owned(), now_ms.into());
        meta.insert("turnStartMs".to_owned(), stream.turn_start_ms.into());
        meta.insert("streamStartMs".to_owned(), stream.stream_start_ms.into());
        if let Some(tokens_used) = tokens_used {
            meta.insert("totalTokens".to_owned(), tokens_used.into());
        }
        if persist {
            let _ = self.persistence_tx.send(PersistenceMsg::Update(
                crate::session::storage::SessionUpdate::Acp(Box::new(notification.clone())),
            ));
        }
        self.gateway.forward_fire_and_forget(notification);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_messages_map_to_native_acp_chunks() {
        let message = child_update(ChildEvent::AgentMessage("hello".to_owned()));
        assert!(matches!(message, acp::SessionUpdate::AgentMessageChunk(_)));
        let thought = child_update(ChildEvent::Thought("considering".to_owned()));
        assert!(matches!(thought, acp::SessionUpdate::AgentThoughtChunk(_)));
    }

    #[test]
    fn child_tools_map_to_native_acp_lifecycle() {
        let started = child_update(ChildEvent::ToolStarted {
            id: "tool-1".to_owned(),
            title: "Read".to_owned(),
            kind: acp::ToolKind::Read,
            raw_input: Some(serde_json::json!({"path": "README.md"})),
        });
        assert!(matches!(
            started,
            acp::SessionUpdate::ToolCall(call)
                if call.tool_call_id.0.as_ref() == "tool-1"
                    && call.status == acp::ToolCallStatus::InProgress
                    && call.kind == acp::ToolKind::Read
        ));

        let finished = child_update(ChildEvent::ToolFinished {
            id: "tool-1".to_owned(),
            title: Some("Read".to_owned()),
            output: Some("contents".to_owned()),
            raw_output: None,
            success: true,
        });
        assert!(matches!(
            finished,
            acp::SessionUpdate::ToolCallUpdate(update)
                if update.tool_call_id.0.as_ref() == "tool-1"
                    && update.fields.status == Some(acp::ToolCallStatus::Completed)
        ));
    }
}
