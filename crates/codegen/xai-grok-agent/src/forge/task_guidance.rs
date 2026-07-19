//! Forge-specific additions to the model-facing task tool description.

const REASONING_PARAM: &str = "${{ params.task.reasoning_effort }}";

pub(crate) fn append(description: &mut String) {
    description.push_str(&format!(
        "\n\nUse `{REASONING_PARAM}` only when a deliberate per-task reasoning budget is useful: \
         `low` for lookup or mechanical checks, `medium` for routine implementation, `high` for \
         difficult debugging/design/review, and `xhigh` only for the hardest ambiguous work. \
         Omit it to use the selected model or harness default."
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_reasoning_guidance_once_per_description_build() {
        let mut description = "task".to_owned();
        append(&mut description);
        assert!(description.contains(REASONING_PARAM));
        assert!(description.contains("`low`"));
        assert!(description.contains("`xhigh`"));
    }
}
