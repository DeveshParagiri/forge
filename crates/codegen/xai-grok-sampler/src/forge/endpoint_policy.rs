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

/// True only for the official ChatGPT Codex Responses endpoint family.
pub(crate) fn is_codex_endpoint(base_url: &str) -> bool {
    let Some(url) = parsed_https_url(base_url) else {
        return false;
    };
    url.host_str() == Some("chatgpt.com") && path_has_prefix(url.path(), "/backend-api/codex")
}

/// Positive allowlist for xAI-private request metadata. Unknown hosts, proxies,
/// cleartext URLs, and third-party providers all fail closed.
pub(crate) fn accepts_xai_private_headers(base_url: &str) -> bool {
    parsed_https_url(base_url)
        .and_then(|url| url.host_str().map(str::to_owned))
        .is_some_and(|host| {
            host == "x.ai"
                || host.ends_with(".x.ai")
                || host == "grok.com"
                || host.ends_with(".grok.com")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_endpoint_requires_exact_https_host_and_path_prefix() {
        assert!(is_codex_endpoint("https://chatgpt.com/backend-api/codex"));
        assert!(is_codex_endpoint(
            "https://chatgpt.com/backend-api/codex/v1"
        ));
        for url in [
            "http://chatgpt.com/backend-api/codex",
            "https://chatgpt.com.attacker.example/backend-api/codex",
            "https://chatgpt.com@attacker.example/backend-api/codex",
            "https://proxy.example/backend-api/codex",
            "https://chatgpt.com/backend-api/codex-evil",
        ] {
            assert!(!is_codex_endpoint(url), "accepted {url}");
        }
    }

    #[test]
    fn private_headers_require_exact_xai_https_domains() {
        for url in [
            "https://api.x.ai/v1",
            "https://x.ai/v1",
            "https://cli-chat-proxy.grok.com/v1",
            "https://grok.com/v1",
        ] {
            assert!(accepts_xai_private_headers(url), "rejected {url}");
        }
        for url in [
            "http://api.x.ai/v1",
            "https://api.x.ai.attacker.example/v1",
            "https://api.x.ai@attacker.example/v1",
            "https://openrouter.ai/api/v1",
            "https://chatgpt.com/backend-api/codex",
            "https://api.openai.com/v1",
            "https://proxy.example/v1",
        ] {
            assert!(!accepts_xai_private_headers(url), "accepted {url}");
        }
    }
}
