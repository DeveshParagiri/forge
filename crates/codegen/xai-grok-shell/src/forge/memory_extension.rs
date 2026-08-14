//! Forge guidance for learning model and harness fit through ordinary memory.
//!
//! Storage, gating, and lifecycle remain owned by the existing memory system.
//! Forge only extends the LLM prompt used to extract or consolidate memory.

const EXTENSION: &str = r#"

<forge_memory>
Retain only evidence that will improve future work for this user: explicitly stated current projects, workstreams, and near-term goals; explicit preferences and corrections; stable workflow or environment facts; and clear or repeated evidence about which models, harnesses, and subagent setups work best for different task types. Label active-work context as current and replace or remove it when the user changes direction. Explicit user direction always outweighs inference. When new evidence conflicts with an older preference, update or consolidate the stale guidance instead of preserving contradictory rules. Do not save step-by-step task progress, temporary execution plans, logs, completed-work narration, unresolved failures, or conclusions from one ambiguous result. Preserve a short quotation attributed to the user only when its wording carries durable intent. Record concise ordinary Markdown under `## Model and harness preferences`, with no scores or separate schema.
</forge_memory>"#;

pub(crate) fn extend_prompt(mut prompt: String) -> String {
    prompt.push_str(EXTENSION);
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extends_normal_memory_with_durable_model_and_harness_learning() {
        let prompt = extend_prompt("upstream memory prompt".to_owned());

        assert!(prompt.starts_with("upstream memory prompt"));
        assert!(prompt.contains("different task types"));
        assert!(prompt.contains("current projects, workstreams, and near-term goals"));
        assert!(prompt.contains("replace or remove it when the user changes direction"));
        assert!(prompt.contains("explicit preferences and corrections"));
        assert!(prompt.contains("Explicit user direction always outweighs inference"));
        assert!(prompt.contains("update or consolidate the stale guidance"));
        assert!(prompt.contains("Do not save step-by-step task progress"));
        assert!(prompt.contains("no scores or separate schema"));
    }
}
