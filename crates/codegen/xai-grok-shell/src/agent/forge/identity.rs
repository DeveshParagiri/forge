/// Provider families with additional Forge behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderId {
    /// SpaceXAI / Grok subscription (`grok login` / `~/.grok/auth.json`).
    Spacexai,
    /// OpenAI Codex via ChatGPT Plus/Pro OAuth (`~/.codex/auth.json`).
    OpenaiCodex,
    /// OpenRouter through upstream model-provider credentials.
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

fn parsed_https_url(base_url: &str) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(base_url).ok()?;
    (url.scheme() == "https").then_some(url)
}

fn path_has_prefix(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

pub(crate) fn is_codex_base(base_url: &str) -> bool {
    let Some(url) = parsed_https_url(base_url) else {
        return false;
    };
    url.host_str() == Some("chatgpt.com") && path_has_prefix(url.path(), "/backend-api/codex")
}

/// True only for the canonical ChatGPT Codex base URL allowed to source an
/// OAuth bearer from `~/.codex/auth.json`. Request adaptation may support
/// subpaths, but credential-file discovery is deliberately exact and narrower.
pub(crate) fn is_codex_auth_base(base_url: &str) -> bool {
    base_url == "https://chatgpt.com/backend-api/codex"
}

fn is_openrouter_base(base_url: &str) -> bool {
    parsed_https_url(base_url)
        .and_then(|url| url.host_str().map(str::to_owned))
        .is_some_and(|host| host == "openrouter.ai")
}

fn is_spacexai_base(base_url: &str) -> bool {
    parsed_https_url(base_url)
        .and_then(|url| url.host_str().map(str::to_owned))
        .is_some_and(|host| {
            host == "x.ai"
                || host.ends_with(".x.ai")
                || host == "grok.com"
                || host.ends_with(".grok.com")
        })
}

/// Resolve Forge provider identity from positively matched HTTPS hosts.
pub fn provider_id_for_base(base_url: &str) -> Option<ProviderId> {
    if is_codex_base(base_url) {
        Some(ProviderId::OpenaiCodex)
    } else if is_openrouter_base(base_url) {
        Some(ProviderId::Openrouter)
    } else if is_spacexai_base(base_url) {
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

/// True unless the endpoint is a positively recognized HTTPS xAI host.
/// Unknown, custom, malformed, and cleartext endpoints therefore fail closed
/// for xAI-only turn capabilities and first-party bearer refresh behavior.
pub fn is_third_party_model_base(base_url: &str) -> bool {
    !is_spacexai_base(base_url)
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
    fn codex_auth_file_fallback_requires_canonical_base_url() {
        assert!(is_codex_auth_base("https://chatgpt.com/backend-api/codex"));
        for url in [
            "https://chatgpt.com/backend-api/codex/",
            "https://chatgpt.com/backend-api/codex/v1",
            "https://chatgpt.com:444/backend-api/codex",
            "https://chatgpt.com/backend-api/codex?proxy=true",
            "http://chatgpt.com/backend-api/codex",
            "https://chatgpt.com.attacker.example/backend-api/codex",
            "https://chatgpt.com@attacker.example/backend-api/codex",
            "https://evil.example/v1",
        ] {
            assert!(!is_codex_auth_base(url), "accepted {url}");
        }
    }

    #[test]
    fn exact_https_provider_bases_are_classified() {
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
        assert_eq!(
            provider_id_for_base("https://cli-chat-proxy.grok.com/v1"),
            Some(ProviderId::Spacexai)
        );
    }

    #[test]
    fn spoofed_or_cleartext_provider_urls_are_rejected() {
        for url in [
            "https://chatgpt.com.attacker.example/backend-api/codex",
            "https://chatgpt.com@attacker.example/backend-api/codex",
            "https://proxy.example/backend-api/codex",
            "http://chatgpt.com/backend-api/codex",
            "https://openrouter.ai.attacker.example/api/v1",
            "http://openrouter.ai/api/v1",
            "https://api.x.ai.attacker.example/v1",
            "http://api.x.ai/v1",
        ] {
            assert_eq!(provider_id_for_base(url), None, "classified {url}");
        }
    }

    #[test]
    fn third_party_bases() {
        assert!(is_third_party_model_base(
            "https://chatgpt.com/backend-api/codex"
        ));
        assert!(is_third_party_model_base("https://openrouter.ai/api/v1"));
        assert!(is_third_party_model_base("https://api.openai.com/v1"));
        assert!(!is_third_party_model_base("https://api.x.ai/v1"));
        assert!(is_third_party_model_base(
            "https://chatgpt.com.attacker.example/backend-api/codex"
        ));
        assert!(is_third_party_model_base("https://unknown.example/v1"));
        assert!(is_third_party_model_base("http://api.x.ai/v1"));
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
