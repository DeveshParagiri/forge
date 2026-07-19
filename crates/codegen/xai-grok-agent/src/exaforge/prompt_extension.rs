//! Concise Exaforge orchestration guidance appended after upstream prompts.

use crate::prompt::context::PromptAudience;
use xai_grok_tools::bridge::ToolBridge;
use xai_grok_tools::types::tool::ToolKind;

const PRIMARY_EXTENSION: &str = r#"<exaforge_orchestration>
Use subagents when independent work benefits from parallelism, specialization, or an isolated context; do not delegate trivial work or duplicate the same investigation.

Choose deliberately:
- Prefer native `general-purpose`, `explore`, and `plan` agents using their inherited/default model, or the subscription-backed `claude-code` and `codex-cli` external harnesses. Pick the role and harness that best fit the work.
- Treat an explicit `model` override on a native agent as last priority when it selects a separately billed, pay-as-you-go API model. Use one when the user requests it or the task clearly requires it; cost preference must not override an explicit user choice.
- External harnesses require their CLIs to be installed and authenticated and do not inherit Exaforge-only hosted tools. Use them for implementation, review, or an independent provider perspective when useful.
- Mixed-model teams are useful when perspectives differ: parallelize independent tasks or assign implementation and verification to different models/harnesses. Keep one clear owner for synthesis and avoid redundant fan-out.

Set `reasoning_effort` proportionally when delegating: `low` for lookup or mechanical checks, `medium` for routine implementation, `high` for difficult debugging/design/review, and `xhigh` only for the hardest ambiguous work. Omit it to use the selected model or harness default. Explicit model choices must use the advertised model slugs; otherwise omit `model`. For external harnesses, omission uses the adapter's provider-native default.
</exaforge_orchestration>"#;

const SUBAGENT_EXTENSION: &str = r#"<exaforge_subagent>
You are one worker in a potentially mixed-model, mixed-harness workflow. Complete only the assigned scope, preserve project instructions, and return concrete findings, edits, tests, and unresolved risks so the parent can synthesize reliably. Do not broaden the task or repeat work assigned elsewhere.
</exaforge_subagent>"#;

pub(crate) async fn append(
    prompt: &mut String,
    audience: PromptAudience,
    tool_bridge: &ToolBridge,
) {
    let has_task_tool = tool_bridge.tool_for_kind(ToolKind::Task).await.is_some();
    append_for_available_tools(prompt, audience, has_task_tool);
}

fn append_for_available_tools(prompt: &mut String, audience: PromptAudience, has_task_tool: bool) {
    let extension = match audience {
        PromptAudience::Primary if !has_task_tool => return,
        PromptAudience::Primary => PRIMARY_EXTENSION,
        PromptAudience::Subagent => SUBAGENT_EXTENSION,
    };

    if prompt.contains(extension_open_tag(audience)) {
        return;
    }
    if !prompt.is_empty() {
        prompt.push_str("\n\n");
    }
    prompt.push_str(extension);
}

fn extension_open_tag(audience: PromptAudience) -> &'static str {
    match audience {
        PromptAudience::Primary => "<exaforge_orchestration>",
        PromptAudience::Subagent => "<exaforge_subagent>",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_extension_is_concise_complete_and_idempotent() {
        let mut prompt = "upstream".to_owned();
        append_for_available_tools(&mut prompt, PromptAudience::Primary, true);
        let once = prompt.clone();
        append_for_available_tools(&mut prompt, PromptAudience::Primary, true);

        assert_eq!(prompt, once);
        assert!(prompt.contains("`claude-code`"));
        assert!(prompt.contains("`codex-cli`"));
        assert!(prompt.contains("Mixed-model teams"));
        assert!(prompt.contains("subscription-backed"));
        assert!(prompt.contains("pay-as-you-go API model"));
        assert!(prompt.contains("explicit user choice"));
        assert!(prompt.contains("`reasoning_effort`"));
        assert!(prompt.contains("`xhigh`"));
        assert!(!prompt.contains("dangerously-skip-permissions"));
    }

    #[test]
    fn primary_extension_is_omitted_without_task_tool() {
        let mut prompt = "upstream".to_owned();
        append_for_available_tools(&mut prompt, PromptAudience::Primary, false);
        assert_eq!(prompt, "upstream");
    }

    #[test]
    fn subagent_extension_scopes_the_worker_without_advertising_nested_spawns() {
        let mut prompt = String::new();
        append_for_available_tools(&mut prompt, PromptAudience::Subagent, false);

        assert!(prompt.starts_with("<exaforge_subagent>"));
        assert!(prompt.contains("Complete only the assigned scope"));
        assert!(!prompt.contains("claude-code"));
        assert!(!prompt.contains("spawn"));
    }
}
