//! Extensible provider interface for Exaforge external CLI subagents.
//!
//! Lifecycle, polling, cancellation, and native TUI routing are
//! provider-neutral. Each adapter owns its CLI arguments, transport protocol,
//! provider state, and wire-event translation.

use std::collections::HashSet;
use std::path::Path;

use xai_tool_types::SubagentCapabilityMode;

use crate::exaforge::subagent_ui::ChildEvent;

mod claude_code;
mod codex_cli;

pub(crate) use claude_code::CLAUDE_CODE_TYPE;
pub(crate) use codex_cli::CODEX_CLI_TYPE;

pub(crate) struct ProviderInvocation<'a> {
    pub prompt: &'a str,
    pub capability: SubagentCapabilityMode,
    pub model: Option<&'a str>,
    pub reasoning_effort: Option<&'a str>,
    pub resume_session_id: Option<&'a str>,
}

pub(crate) struct ProviderState {
    /// Provider-native model selected for this invocation. Adapters seed this
    /// before process launch and may replace it with a wire-resolved model.
    pub effective_model: String,
    pub final_text: String,
    pub session_id: Option<String>,
    /// Current provider-reported context length, not cumulative billed tokens.
    pub tokens_used: u64,
    pub context_window_tokens: u64,
    pub tool_calls: u32,
    pub turns: u32,
    pub error_count: u32,
    pub tools_used: HashSet<String>,
    known_tools: HashSet<String>,
}

impl ProviderState {
    pub(crate) fn new(effective_model: impl Into<String>) -> Self {
        Self {
            effective_model: effective_model.into(),
            final_text: String::new(),
            session_id: None,
            tokens_used: 0,
            context_window_tokens: 0,
            tool_calls: 0,
            turns: 0,
            error_count: 0,
            tools_used: HashSet::new(),
            known_tools: HashSet::new(),
        }
    }

    pub(crate) fn record_resolved_model(&mut self, model: &str) {
        let model = model.trim();
        if !model.is_empty() {
            self.effective_model = model.to_owned();
        }
    }

    pub(crate) fn record_usage(&mut self, tokens_used: u64, context_window_tokens: Option<u64>) {
        self.tokens_used = tokens_used;
        if let Some(context_window_tokens) = context_window_tokens.filter(|value| *value > 0) {
            self.context_window_tokens = context_window_tokens;
        }
    }

    pub(crate) fn context_usage_pct(&self) -> u8 {
        if self.context_window_tokens == 0 {
            return 0;
        }
        self.tokens_used
            .saturating_mul(100)
            .checked_div(self.context_window_tokens)
            .unwrap_or_default()
            .min(100) as u8
    }

    pub(crate) fn record_tool(&mut self, id: &str, name: &str) {
        if self.known_tools.insert(id.to_owned()) {
            self.tool_calls = self.tool_calls.saturating_add(1);
        }
        self.tools_used.insert(name.to_owned());
    }

    pub(crate) fn knows_tool(&self, id: &str) -> bool {
        self.known_tools.contains(id)
    }
}

pub(crate) struct ProviderUpdate {
    pub events: Vec<ChildEvent>,
    /// JSONL messages to write back to an interactive provider transport.
    pub outbound: Vec<String>,
    /// A protocol-level terminal result. Long-lived servers are stopped by the
    /// provider-neutral process host after this arrives.
    pub terminal: Option<Result<(), String>>,
}

impl ProviderUpdate {
    pub(crate) fn events(events: Vec<ChildEvent>) -> Self {
        Self {
            events,
            outbound: Vec::new(),
            terminal: None,
        }
    }

    pub(crate) fn empty() -> Self {
        Self::events(Vec::new())
    }
}

/// One invocation of an external provider. This object lets adapters such as
/// Codex own a stateful JSON-RPC handshake without putting provider protocol
/// details in the shared process host.
pub(crate) trait ExternalProviderSession: Send {
    fn program(&self) -> &'static str;
    fn args(&self) -> &[String];
    fn initial_input(&mut self) -> Vec<String> {
        Vec::new()
    }
    fn handle_line(&mut self, line: &str) -> ProviderUpdate;
    fn state(&self) -> &ProviderState;
}

pub(crate) trait ExternalProvider: Sync {
    fn subagent_type(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn program(&self) -> &'static str;
    fn create_session(
        &self,
        invocation: ProviderInvocation<'_>,
        cwd: &Path,
    ) -> Box<dyn ExternalProviderSession>;
}

static PROVIDERS: [&dyn ExternalProvider; 2] = [&claude_code::CLAUDE_CODE, &codex_cli::CODEX_CLI];

pub(crate) fn find(subagent_type: &str) -> Option<&'static dyn ExternalProvider> {
    PROVIDERS
        .iter()
        .copied()
        .find(|provider| provider.subagent_type() == subagent_type)
}

pub(crate) fn names() -> impl Iterator<Item = &'static str> {
    PROVIDERS.iter().map(|provider| provider.subagent_type())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_exposes_each_provider() {
        assert_eq!(
            find(CLAUDE_CODE_TYPE).map(ExternalProvider::program),
            Some("claude")
        );
        assert_eq!(
            find(CODEX_CLI_TYPE).map(ExternalProvider::program),
            Some("codex")
        );
        assert!(find("opencode").is_none());
        assert_eq!(
            names().collect::<Vec<_>>(),
            [CLAUDE_CODE_TYPE, CODEX_CLI_TYPE]
        );
    }
}
