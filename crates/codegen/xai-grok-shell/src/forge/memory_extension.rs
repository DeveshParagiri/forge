//! Forge guidance for learning model and harness fit through ordinary memory.
//!
//! Storage, gating, and lifecycle remain owned by the existing memory system.
//! Forge only extends the LLM prompt used to extract or consolidate memory.

const EXTENSION: &str = r#"

<forge_memory>
As an exception to any general preference exclusion, retain durable evidence about which models, harnesses, and subagent setups work best for this user for different task types. Consider explicit direction and corrections, repeated choices, the target used, outcome quality, and the user's reactions or feedback. Preserve a short quotation attributed to the user when its wording carries the intent. Explicit user direction outweighs inference; infer only from clear or repeated evidence, and do not invent certainty from one ambiguous result. Record concise ordinary Markdown under `## Model and harness preferences`, with no scores or separate schema.
</forge_memory>"#;

pub(crate) fn extend_prompt(mut prompt: String) -> String {
    prompt.push_str(EXTENSION);
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extends_normal_memory_with_semantic_model_and_harness_learning() {
        let prompt = extend_prompt("upstream memory prompt".to_owned());

        assert!(prompt.starts_with("upstream memory prompt"));
        assert!(prompt.contains("different task types"));
        assert!(prompt.contains("outcome quality"));
        assert!(prompt.contains("reactions or feedback"));
        assert!(prompt.contains("Explicit user direction outweighs inference"));
        assert!(prompt.contains("clear or repeated evidence"));
        assert!(prompt.contains("no scores or separate schema"));
    }
}
