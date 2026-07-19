/// Provider families supported by the Forge model packs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderId {
    /// SpaceXAI / Grok subscription (existing `grok login` / `~/.grok/auth.json`).
    Spacexai,
    /// OpenAI Codex via ChatGPT Plus/Pro OAuth (`~/.codex/auth.json`).
    OpenaiCodex,
    /// OpenRouter API key (`~/.grok/provider_keys.json` + `OPENROUTER_API_KEY`).
    Openrouter,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spacexai => "spacexai",
            Self::OpenaiCodex => "openai-codex",
            Self::Openrouter => "openrouter",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Spacexai => "SpaceXAI",
            Self::OpenaiCodex => "OpenAI Codex",
            Self::Openrouter => "OpenRouter",
        }
    }

    /// Compact provider prefix used in the model picker and status line.
    pub fn model_prefix(self) -> &'static str {
        match self {
            Self::Spacexai => "SpaceX",
            Self::OpenaiCodex => "OpenAI",
            Self::Openrouter => "OpenRouter",
        }
    }

    pub fn catalog_key(self) -> &'static str {
        match self {
            Self::Spacexai => "spacexai",
            Self::OpenaiCodex => "openai_codex",
            Self::Openrouter => "openrouter",
        }
    }

    pub fn from_str_id(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "spacexai" | "xai" | "grok" => Some(Self::Spacexai),
            "openai-codex" | "codex" | "chatgpt" => Some(Self::OpenaiCodex),
            "openrouter" | "or" => Some(Self::Openrouter),
            _ => None,
        }
    }
}

/// Resolve the three Forge provider families from a model endpoint.
///
/// TODO: Replace substring classification with parsed-host matching in the
/// dedicated hardening follow-up. This intentionally preserves current broad
/// third-party semantics.
pub fn provider_id_for_base(base_url: &str) -> Option<ProviderId> {
    let url = base_url.to_ascii_lowercase();
    if url.contains("chatgpt.com") || url.contains("backend-api/codex") {
        Some(ProviderId::OpenaiCodex)
    } else if url.contains("openrouter.ai") {
        Some(ProviderId::Openrouter)
    } else if url.contains("api.x.ai")
        || url.contains("grok.com")
        || url.contains("spacexai")
        || url.contains(".x.ai")
    {
        Some(ProviderId::Spacexai)
    } else {
        None
    }
}

/// Stable provider identity used to decide whether opaque reasoning can be
/// replayed after a model switch. Unknown endpoints remain distinct by URL.
pub fn provider_scope_for_base(base_url: &str) -> String {
    provider_id_for_base(base_url)
        .map(|id| id.as_str().to_string())
        .unwrap_or_else(|| base_url.trim().trim_end_matches('/').to_ascii_lowercase())
}

/// True when `base_url` is a third-party host that must never receive Grok session OIDC.
pub fn is_third_party_model_base(base_url: &str) -> bool {
    let u = base_url.to_ascii_lowercase();
    u.contains("chatgpt.com")
        || u.contains("backend-api/codex")
        || u.contains("openrouter.ai")
        || u.contains("api.openai.com")
}

/// Normalize a configured model name, then prefix it with its provider.
///
/// This is display-only: catalog keys and wire model ids remain unchanged.
pub fn display_model_name(provider: ProviderId, configured_name: &str) -> String {
    let mut name = configured_name.trim();
    for prefix in [
        "SpaceXAI · ",
        "SpaceX · ",
        "OpenAI Codex · ",
        "OpenAI · ",
        "OpenRouter · ",
    ] {
        if let Some(rest) = name.strip_prefix(prefix) {
            name = rest.trim();
            break;
        }
    }
    if let Some(rest) = name.strip_suffix(" (Codex)") {
        name = rest.trim();
    }
    format!("{} · {name}", provider.model_prefix())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_id_parse() {
        assert_eq!(
            ProviderId::from_str_id("codex"),
            Some(ProviderId::OpenaiCodex)
        );
        assert_eq!(
            ProviderId::from_str_id("openrouter"),
            Some(ProviderId::Openrouter)
        );
        assert_eq!(ProviderId::from_str_id("xai"), Some(ProviderId::Spacexai));
    }

    #[test]
    fn third_party_bases() {
        assert!(is_third_party_model_base(
            "https://chatgpt.com/backend-api/codex"
        ));
        assert!(is_third_party_model_base("https://openrouter.ai/api/v1"));
        assert!(!is_third_party_model_base("https://api.x.ai/v1"));
    }

    #[test]
    fn provider_base_classification() {
        assert_eq!(
            provider_id_for_base("https://chatgpt.com/backend-api/codex"),
            Some(ProviderId::OpenaiCodex)
        );
        assert_eq!(
            provider_id_for_base("https://openrouter.ai/api/v1"),
            Some(ProviderId::Openrouter)
        );
        assert_eq!(
            provider_id_for_base("https://api.x.ai/v1"),
            Some(ProviderId::Spacexai)
        );
        assert_eq!(provider_id_for_base("http://localhost:11434/v1"), None);
    }

    #[test]
    fn display_model_names_have_one_compact_provider_prefix() {
        assert_eq!(
            display_model_name(ProviderId::Spacexai, "Grok 4.5"),
            "SpaceX · Grok 4.5"
        );
        assert_eq!(
            display_model_name(ProviderId::OpenaiCodex, "GPT-5.6 Sol (Codex)"),
            "OpenAI · GPT-5.6 Sol"
        );
        assert_eq!(
            display_model_name(ProviderId::Openrouter, "OpenRouter · Gemini 3.5 Flash"),
            "OpenRouter · Gemini 3.5 Flash"
        );
    }
}
