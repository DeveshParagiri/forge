//! Exaforge-specific task input extensions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Per-task reasoning budget shared by native and external subagents.
///
/// The set is intentionally limited to levels supported across Exaforge's
/// native sampler, Claude Code, and Codex CLI adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SubagentReasoningEffort {
    Low,
    Medium,
    High,
    Xhigh,
}

impl SubagentReasoningEffort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_effort_wire_values_are_stable() {
        for (effort, expected) in [
            (SubagentReasoningEffort::Low, "low"),
            (SubagentReasoningEffort::Medium, "medium"),
            (SubagentReasoningEffort::High, "high"),
            (SubagentReasoningEffort::Xhigh, "xhigh"),
        ] {
            assert_eq!(effort.as_str(), expected);
            assert_eq!(
                serde_json::to_string(&effort).unwrap(),
                format!("\"{expected}\"")
            );
        }
    }
}
