/// Remove the provider prefix from a catalog display name for the prompt footer.
///
/// Provider-qualified names remain available in model pickers, where they help
/// distinguish otherwise similar entries. The compact prompt footer shows only
/// the selected model name.
pub(crate) fn prompt_footer_model_name(display_name: String) -> String {
    const PROVIDER_SEPARATOR: &str = " · ";

    if let Some((_, model_name)) = display_name.split_once(PROVIDER_SEPARATOR) {
        model_name.trim().to_owned()
    } else {
        display_name
    }
}

/// Canonical primary label plus Forge-owned session decorations.
pub(crate) fn primary_model_label(display_name: String, fast_mode: bool) -> String {
    crate::forge::fast_mode::decorate_model_label(prompt_footer_model_name(display_name), fast_mode)
}

#[cfg(test)]
mod tests {
    use super::{primary_model_label, prompt_footer_model_name};

    #[test]
    fn removes_provider_prefix() {
        assert_eq!(
            prompt_footer_model_name("OpenAI · GPT-5.6 Sol".to_owned()),
            "GPT-5.6 Sol"
        );
        assert_eq!(
            prompt_footer_model_name("OpenRouter · Gemini 3.5 Flash".to_owned()),
            "Gemini 3.5 Flash"
        );
    }

    #[test]
    fn preserves_unqualified_model_name() {
        assert_eq!(prompt_footer_model_name("Grok 4.5".to_owned()), "Grok 4.5");
    }

    #[test]
    fn strips_spacex_compact_prefix() {
        assert_eq!(
            prompt_footer_model_name("SpaceX · Grok 4.5".to_owned()),
            "Grok 4.5"
        );
    }

    #[test]
    fn adds_lightning_only_when_fast_mode_is_enabled() {
        assert_eq!(
            primary_model_label("OpenAI · Codex".to_owned(), true),
            "⚡ Codex"
        );
        assert_eq!(
            primary_model_label("OpenAI · Codex".to_owned(), false),
            "Codex"
        );
    }
}
