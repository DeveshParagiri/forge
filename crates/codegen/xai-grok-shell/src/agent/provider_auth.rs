//! Personal: multi-provider credentials (Codex ChatGPT OAuth, OpenRouter API key).
//!
//! Isolated from upstream so rebases stay clean. Upstream `grok login` / SpaceXAI
//! OAuth is untouched; this only adds third-party packs used by `[provider.*]`.
//!
//! Storage:
//! - Codex: `~/.codex/auth.json` (owned by Codex CLI — we only read)
//! - OpenRouter: `~/.grok/provider_keys.json` under key `openrouter`
//!
//! Status helpers power the Pi-style `/login` picker in the pager.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Personal per-provider model allow/deny list from `[catalog.*]`.
///
/// An empty `include` means all models for that provider. `exclude` always
/// wins. Patterns use the same glob syntax as `[models].allowed_models`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderCatalogRule {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// Personal provider-scoped catalog configuration.
///
/// `Option` preserves whether a section was present: an absent provider
/// section leaves the upstream catalog untouched, while an empty section
/// explicitly enables all configured models for that provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderCatalogConfig {
    pub spacexai: Option<ProviderCatalogRule>,
    pub openai_codex: Option<ProviderCatalogRule>,
    pub openrouter: Option<ProviderCatalogRule>,
}

impl ProviderCatalogConfig {
    pub fn rule(&self, id: ProviderId) -> Option<&ProviderCatalogRule> {
        match id {
            ProviderId::Spacexai => self.spacexai.as_ref(),
            ProviderId::OpenaiCodex => self.openai_codex.as_ref(),
            ProviderId::Openrouter => self.openrouter.as_ref(),
        }
    }

    pub fn configured(&self) -> impl Iterator<Item = (ProviderId, &ProviderCatalogRule)> {
        [
            ProviderId::Spacexai,
            ProviderId::OpenaiCodex,
            ProviderId::Openrouter,
        ]
        .into_iter()
        .filter_map(|id| self.rule(id).map(|rule| (id, rule)))
    }
}

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

/// Resolve the three personal provider families from a model endpoint.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderAuthStatus {
    /// Subscription / OAuth token present and looks usable.
    Stored {
        detail: String,
    },
    /// API key present (env or file).
    ApiKeyConfigured {
        detail: String,
    },
    Unconfigured,
}

impl ProviderAuthStatus {
    pub fn label(&self) -> String {
        match self {
            Self::Stored { detail } => format!("✓ stored ({detail})"),
            Self::ApiKeyConfigured { detail } => format!("✓ api key ({detail})"),
            Self::Unconfigured => "unconfigured".into(),
        }
    }

    pub fn is_ready(&self) -> bool {
        !matches!(self, Self::Unconfigured)
    }

    /// Deliberately hides credential source, account, plan, and key details.
    pub fn configured_label(&self) -> &'static str {
        if self.is_ready() {
            "configured"
        } else {
            "not configured"
        }
    }
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

fn grok_home() -> PathBuf {
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
    // Write with restrictive perms when possible.
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        f.write_all(body.as_bytes())?;
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
    // Avoid pulling chrono into shell just for a stamp; RFC3339-ish local.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

/// Read OpenRouter key: env first, then provider_keys.json.
pub fn read_openrouter_api_key() -> Option<String> {
    if let Ok(v) = std::env::var("OPENROUTER_API_KEY") {
        let t = v.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    load_provider_keys()
        .openrouter
        .map(|e| e.api_key)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Read Codex ChatGPT OAuth access token from `~/.codex/auth.json`.
pub fn read_codex_access_token() -> Option<String> {
    let path = codex_auth_path();
    let raw = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    v.get("tokens")
        .and_then(|t| t.get("access_token"))
        .and_then(|t| t.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Read Codex ChatGPT account id.
pub fn read_codex_account_id() -> Option<String> {
    let path = codex_auth_path();
    let raw = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    v.get("tokens")
        .and_then(|t| t.get("account_id"))
        .and_then(|t| t.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

pub fn codex_status() -> ProviderAuthStatus {
    let path = codex_auth_path();
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return ProviderAuthStatus::Unconfigured;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return ProviderAuthStatus::Unconfigured;
    };
    let mode = v
        .get("auth_mode")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");
    let has_token = v
        .get("tokens")
        .and_then(|t| t.get("access_token"))
        .and_then(|t| t.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !has_token {
        return ProviderAuthStatus::Unconfigured;
    }
    let plan = jwt_chatgpt_plan(
        v.get("tokens")
            .and_then(|t| t.get("access_token"))
            .and_then(|t| t.as_str())
            .unwrap_or(""),
    );
    let detail = match plan {
        Some(p) => format!("{mode}, {p}"),
        None => mode.to_string(),
    };
    ProviderAuthStatus::Stored { detail }
}

fn jwt_chatgpt_plan(token: &str) -> Option<String> {
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let payload = parts[1];
    let padded = match payload.len() % 4 {
        2 => format!("{payload}=="),
        3 => format!("{payload}="),
        _ => payload.to_string(),
    };
    let decoded = base64_url_decode(&padded)?;
    let v: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    v.get("https://api.openai.com/auth")
        .and_then(|a| a.get("chatgpt_plan_type"))
        .and_then(|p| p.as_str())
        .map(|s| s.to_string())
}

fn base64_url_decode(s: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(s))
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(s))
        .ok()
}

pub fn openrouter_status() -> ProviderAuthStatus {
    if let Some(key) = read_openrouter_api_key() {
        let prefix = key.chars().take(8).collect::<String>();
        return ProviderAuthStatus::ApiKeyConfigured {
            detail: format!("{prefix}…"),
        };
    }
    ProviderAuthStatus::Unconfigured
}

pub fn spacexai_status() -> ProviderAuthStatus {
    // Presence of ~/.grok/auth.json entry — best-effort without pulling full AuthManager.
    let path = grok_home().join("auth.json");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return ProviderAuthStatus::Unconfigured;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return ProviderAuthStatus::Unconfigured;
    };
    // auth.json is a map of scopes to credentials. Subscription logins use
    // `key` plus `refresh_token`; older builds used token/access_token.
    let has_any = v.as_object().is_some_and(|o| {
        o.values().any(|entry| {
            ["key", "token", "access_token", "refresh_token"]
                .into_iter()
                .any(|field| {
                    entry
                        .get(field)
                        .and_then(|t| t.as_str())
                        .is_some_and(|s| !s.trim().is_empty())
                })
                || entry.as_str().is_some_and(|s| !s.trim().is_empty())
        })
    });
    if has_any {
        ProviderAuthStatus::Stored {
            detail: "session".into(),
        }
    } else {
        ProviderAuthStatus::Unconfigured
    }
}

pub fn status_for(id: ProviderId) -> ProviderAuthStatus {
    match id {
        ProviderId::Spacexai => spacexai_status(),
        ProviderId::OpenaiCodex => codex_status(),
        ProviderId::Openrouter => openrouter_status(),
    }
}

/// Short, non-secret status shown beside models in the `/model` picker.
pub fn picker_auth_status(id: ProviderId) -> &'static str {
    if status_for(id).is_ready() {
        return "ready";
    }
    match id {
        ProviderId::Spacexai | ProviderId::OpenaiCodex => "login required",
        ProviderId::Openrouter => "key missing",
    }
}

/// Providers shown in the Pi-style `/login` picker (order = display order).
pub fn login_picker_providers() -> &'static [ProviderId] {
    &[
        ProviderId::Spacexai,
        ProviderId::OpenaiCodex,
        ProviderId::Openrouter,
    ]
}

/// True when `base_url` is a third-party host that must never receive Grok session OIDC.
pub fn is_third_party_model_base(base_url: &str) -> bool {
    let u = base_url.to_ascii_lowercase();
    u.contains("chatgpt.com")
        || u.contains("backend-api/codex")
        || u.contains("openrouter.ai")
        || u.contains("api.openai.com")
}

/// Env key names that should fall back to personal credential files.
pub fn env_requests_codex_token(names: &[&str]) -> bool {
    names.iter().any(|k| {
        k.eq_ignore_ascii_case("CODEX_ACCESS_TOKEN") || k.eq_ignore_ascii_case("OPENAI_CODEX_TOKEN")
    })
}

pub fn env_requests_openrouter_token(names: &[&str]) -> bool {
    names
        .iter()
        .any(|k| k.eq_ignore_ascii_case("OPENROUTER_API_KEY"))
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

    #[test]
    fn configured_labels_do_not_expose_credential_details() {
        assert_eq!(
            ProviderAuthStatus::Unconfigured.configured_label(),
            "not configured"
        );
        assert_eq!(
            ProviderAuthStatus::ApiKeyConfigured {
                detail: "secret-prefix".into()
            }
            .configured_label(),
            "configured"
        );
    }
}

#[test]
fn codex_token_readable_from_home() {
    // Only meaningful on a machine with Codex login; skip if missing.
    if read_codex_access_token().is_none() {
        eprintln!("skip: no ~/.codex/auth.json token");
        return;
    }
    assert!(read_codex_access_token().unwrap().len() > 20);
    assert!(read_codex_account_id().is_some());
    assert!(codex_status().is_ready());
}

#[test]
fn spacex_subscription_readable_from_home() {
    if !grok_home().join("auth.json").is_file() {
        eprintln!("skip: no ~/.grok/auth.json");
        return;
    }
    assert!(
        spacexai_status().is_ready(),
        "existing Grok subscription credentials must render as configured"
    );
}
