use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agent::config::{EnvKeys, ModelEntry, ResolvedCredentials};

/// On-disk store for BYOK provider keys that are not SpaceXAI session auth.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderKeysFile {
    #[serde(default)]
    pub openrouter: Option<ProviderKeyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderKeyEntry {
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

pub(super) fn grok_home() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".grok")
}

pub fn provider_keys_path() -> PathBuf {
    grok_home().join("provider_keys.json")
}

pub fn codex_auth_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
        .join("auth.json")
}

pub fn load_provider_keys() -> ProviderKeysFile {
    let path = provider_keys_path();
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return ProviderKeysFile::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_provider_keys(file: &ProviderKeysFile) -> std::io::Result<()> {
    let path = provider_keys_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(file)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // TODO: Make provider-key writes atomic in the dedicated correctness follow-up.
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(body.as_bytes())?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, body)?;
    }
    Ok(())
}

pub fn set_openrouter_api_key(api_key: &str) -> std::io::Result<()> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "empty API key",
        ));
    }
    let mut file = load_provider_keys();
    file.openrouter = Some(ProviderKeyEntry {
        api_key: key.to_string(),
        updated_at: Some(chrono_lite_now()),
    });
    save_provider_keys(&file)
}

fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

/// Read OpenRouter key: env first, then provider_keys.json.
pub fn read_openrouter_api_key() -> Option<String> {
    if let Ok(value) = std::env::var("OPENROUTER_API_KEY") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    load_provider_keys()
        .openrouter
        .map(|entry| entry.api_key)
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

/// Read Codex ChatGPT OAuth access token from `~/.codex/auth.json`.
pub fn read_codex_access_token() -> Option<String> {
    let raw = std::fs::read_to_string(codex_auth_path()).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get("tokens")
        .and_then(|tokens| tokens.get("access_token"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
}

/// Read Codex ChatGPT account id.
pub fn read_codex_account_id() -> Option<String> {
    let raw = std::fs::read_to_string(codex_auth_path()).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get("tokens")
        .and_then(|tokens| tokens.get("account_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty())
        .map(str::to_owned)
}

/// Whether environment key names should fall back to Codex credentials.
pub fn env_requests_codex_token(names: &[&str]) -> bool {
    names.iter().any(|key| {
        key.eq_ignore_ascii_case("CODEX_ACCESS_TOKEN")
            || key.eq_ignore_ascii_case("OPENAI_CODEX_TOKEN")
    })
}

/// Whether environment key names should fall back to OpenRouter credentials.
pub fn env_requests_openrouter_token(names: &[&str]) -> bool {
    names
        .iter()
        .any(|key| key.eq_ignore_ascii_case("OPENROUTER_API_KEY"))
}

pub(crate) fn resolve_own(api_key: Option<&str>, env_key: Option<&EnvKeys>) -> Option<String> {
    api_key
        .filter(|key| !key.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| env_key.and_then(EnvKeys::resolve_value))
        .or_else(|| {
            let names = env_key.map(EnvKeys::names).unwrap_or_default();
            let name_refs: Vec<&str> = names.to_vec();
            if env_requests_codex_token(&name_refs) {
                read_codex_access_token()
            } else if env_requests_openrouter_token(&name_refs) {
                read_openrouter_api_key()
            } else {
                None
            }
        })
}

pub(crate) fn resolve(model: &ModelEntry, session_key: Option<&str>) -> ResolvedCredentials {
    let info = model.info();
    let (api_key, base_url, auth_type) = if let Some(key) = model.own_credential() {
        (
            Some(key),
            info.base_url.clone(),
            xai_chat_state::AuthType::ApiKey,
        )
    } else if let Some(key) =
        session_key.filter(|_| !super::identity::is_third_party_model_base(&info.base_url))
    {
        (
            Some(key.to_owned()),
            info.base_url.clone(),
            xai_chat_state::AuthType::SessionToken,
        )
    } else if !super::identity::is_third_party_model_base(&info.base_url)
        && let Ok(key) = crate::agent::auth_method::read_xai_api_key_env()
    {
        let url = model
            .api_base_url
            .clone()
            .unwrap_or_else(|| info.base_url.clone());
        (Some(key), url, xai_chat_state::AuthType::ApiKey)
    } else {
        if let Some(ref env_keys) = model.env_key
            && !env_keys.is_empty()
        {
            tracing::warn!(
                model = %info.model,
                env_key = %env_keys,
                "model has env_key configured but none of the environment variables are set — requests will have no API key",
            );
        }
        (
            None,
            info.base_url.clone(),
            xai_chat_state::AuthType::ApiKey,
        )
    };
    tracing::debug!(
        model = %info.model,
        auth_type = ?auth_type,
        "resolved credentials"
    );
    ResolvedCredentials {
        api_key,
        base_url,
        auth_type,
        auth_scheme: info.auth_scheme,
    }
}
