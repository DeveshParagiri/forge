use std::path::{Path, PathBuf};

use crate::agent::config::EnvKeys;

pub fn codex_auth_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
        .join("auth.json")
}

fn read_codex_token_field(path: &Path, field: &str) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get("tokens")
        .and_then(|tokens| tokens.get(field))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Read the ChatGPT OAuth access token owned by the Codex CLI.
pub fn read_codex_access_token() -> Option<String> {
    read_codex_token_field(&codex_auth_path(), "access_token")
}

/// Read the ChatGPT account id owned by the Codex CLI.
pub fn read_codex_account_id() -> Option<String> {
    read_codex_token_field(&codex_auth_path(), "account_id")
}

/// Whether environment key names request the narrow Codex credential fallback.
pub fn env_requests_codex_token(names: &[&str]) -> bool {
    names.iter().any(|key| {
        key.eq_ignore_ascii_case("CODEX_ACCESS_TOKEN")
            || key.eq_ignore_ascii_case("OPENAI_CODEX_TOKEN")
    })
}

fn codex_file_fallback_allowed(base_url: &str, env_key: Option<&EnvKeys>) -> bool {
    let names = env_key.map(EnvKeys::names).unwrap_or_default();
    super::identity::is_codex_auth_base(base_url) && env_requests_codex_token(&names)
}

/// Resolve ordinary static/env credentials, with one additional fallback to
/// the Codex CLI credential file only when the configured env key explicitly
/// names the ChatGPT subscription token and the model uses the canonical
/// official HTTPS Codex endpoint.
pub(crate) fn resolve_own(
    api_key: Option<&str>,
    env_key: Option<&EnvKeys>,
    base_url: &str,
) -> Option<String> {
    api_key
        .filter(|key| !key.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| env_key.and_then(EnvKeys::resolve_value))
        .or_else(|| {
            codex_file_fallback_allowed(base_url, env_key)
                .then(read_codex_access_token)
                .flatten()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth_file(body: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        std::fs::write(&path, body).unwrap();
        (dir, path)
    }

    #[test]
    fn fixture_parser_reads_token_and_account() {
        let (_dir, path) = auth_file(
            r#"{"tokens":{"access_token":"  token-value  ","account_id":" account-1 "}}"#,
        );
        assert_eq!(
            read_codex_token_field(&path, "access_token").as_deref(),
            Some("token-value")
        );
        assert_eq!(
            read_codex_token_field(&path, "account_id").as_deref(),
            Some("account-1")
        );
    }

    #[test]
    fn fixture_parser_fails_closed_for_missing_blank_or_malformed_values() {
        let (_missing_dir, missing) = auth_file(r#"{"tokens":{}}"#);
        assert!(read_codex_token_field(&missing, "access_token").is_none());

        let (_blank_dir, blank) = auth_file(r#"{"tokens":{"access_token":"  "}}"#);
        assert!(read_codex_token_field(&blank, "access_token").is_none());

        let (_bad_dir, bad) = auth_file("not-json");
        assert!(read_codex_token_field(&bad, "access_token").is_none());

        let nonexistent = bad.with_file_name("absent.json");
        assert!(read_codex_token_field(&nonexistent, "access_token").is_none());
    }

    #[test]
    fn only_explicit_codex_env_names_enable_file_fallback() {
        assert!(env_requests_codex_token(&["CODEX_ACCESS_TOKEN"]));
        assert!(env_requests_codex_token(&["openai_codex_token"]));
        assert!(!env_requests_codex_token(&["OPENROUTER_API_KEY"]));
        assert!(!env_requests_codex_token(&[]));
    }

    #[test]
    fn codex_file_fallback_is_bound_to_canonical_official_endpoint() {
        let env_key = EnvKeys::single("CODEX_ACCESS_TOKEN");
        assert!(codex_file_fallback_allowed(
            "https://chatgpt.com/backend-api/codex",
            Some(&env_key)
        ));
        for url in [
            "https://evil.example/v1",
            "https://chatgpt.com.attacker.example/backend-api/codex",
            "https://proxy.example/backend-api/codex",
            "http://chatgpt.com/backend-api/codex",
            "https://chatgpt.com/backend-api/codex/v1",
        ] {
            assert!(
                !codex_file_fallback_allowed(url, Some(&env_key)),
                "allowed Codex auth-file fallback for {url}"
            );
        }
        assert!(!codex_file_fallback_allowed(
            "https://chatgpt.com/backend-api/codex",
            Some(&EnvKeys::single("OPENROUTER_API_KEY"))
        ));
    }
}
