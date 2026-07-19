//! Concise Forge orchestration guidance appended after upstream prompts.

use crate::prompt::context::PromptAudience;
use xai_grok_tools::bridge::ToolBridge;
use xai_grok_tools::types::tool::ToolKind;

const PRIMARY_EXTENSION: &str = r#"<forge_orchestration>
Delegate only when parallelism, specialization, isolation, or independent verification outweighs coordination cost. Choose among the targets and model slugs advertised by the task tool based on task fit and required capabilities.

Unless the user specifies otherwise, prefer a suitable subscription-backed or included harness over a separately metered API model. This is a cost preference, not a quality ranking: explicit user choices and required capabilities win.

For complex or high-risk work, use a strong reasoning or implementation model; use faster options for bounded lookup or mechanical work. Mix roles, model families, or harnesses when complementary perspectives help, especially for independent review. Give workers non-overlapping scopes, avoid redundant fan-out, and keep one synthesis owner. Omit `model` to inherit or use the harness default; set `reasoning_effort` only when a deliberate per-task budget helps.
</forge_orchestration>"#;

const SUBAGENT_EXTENSION: &str = r#"<forge_subagent>
You are one worker in a potentially mixed-model, mixed-harness workflow. Complete only the assigned scope, preserve project instructions, and return concrete findings, edits, tests, and unresolved risks so the parent can synthesize reliably. Do not broaden the task or repeat work assigned elsewhere.
</forge_subagent>"#;

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
        PromptAudience::Primary => "<forge_orchestration>",
        PromptAudience::Subagent => "<forge_subagent>",
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
        assert!(prompt.contains("targets and model slugs advertised by the task tool"));
        assert!(prompt.contains("subscription-backed or included harness"));
        assert!(prompt.contains("separately metered API model"));
        assert!(prompt.contains("explicit user choices and required capabilities win"));
        assert!(prompt.contains("model families, or harnesses"));
        assert!(prompt.contains("independent review"));
        assert!(prompt.contains("one synthesis owner"));
        assert!(prompt.contains("`reasoning_effort`"));
        assert!(PRIMARY_EXTENSION.len() < 1_500);
        assert!(!prompt.contains("claude-code"));
        assert!(!prompt.contains("codex-cli"));
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

        assert!(prompt.starts_with("<forge_subagent>"));
        assert!(prompt.contains("Complete only the assigned scope"));
        assert!(!prompt.contains("claude-code"));
        assert!(!prompt.contains("spawn"));
    }
}
