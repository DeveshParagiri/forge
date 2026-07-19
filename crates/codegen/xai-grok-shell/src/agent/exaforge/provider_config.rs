use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::agent::config::{EnvKeys, ModelEntry};
use crate::sampling::ApiBackend;

/// Shared endpoint + credential pack for multiple `[model.*]` entries.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProviderConfig {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub api_backend: Option<ApiBackend>,
    pub api_key: Option<String>,
    pub env_key: Option<EnvKeys>,
    /// Auth pack id: `codex` or `openrouter`.
    pub auth: Option<String>,
    #[serde(default)]
    pub extra_headers: IndexMap<String, String>,
}

impl ProviderConfig {
    /// Fold provider defaults into a model entry after model-local overrides.
    pub fn apply_to_entry(
        &self,
        entry: &mut ModelEntry,
        model_set_base_url: bool,
        model_set_backend: bool,
        model_set_creds: bool,
    ) {
        if !model_set_base_url {
            if let Some(ref url) = self.base_url {
                entry.info.base_url = url.clone();
            }
        }
        if !model_set_backend {
            if let Some(ref backend) = self.api_backend {
                entry.info.api_backend = backend.clone();
            }
        }
        if !model_set_creds {
            if let Some(ref key) = self.api_key {
                entry.api_key = Some(key.clone());
            }
            if let Some(ref env_key) = self.env_key {
                entry.env_key = Some(env_key.clone());
            }
        }
        if let Some(auth) = self.auth.as_deref() {
            if auth.eq_ignore_ascii_case("codex") {
                if entry.env_key.is_none() {
                    entry.env_key = Some(EnvKeys::single("CODEX_ACCESS_TOKEN"));
                }
                if !model_set_base_url && self.base_url.is_none() {
                    entry.info.base_url = "https://chatgpt.com/backend-api/codex".to_string();
                }
                if !model_set_backend && self.api_backend.is_none() {
                    entry.info.api_backend = ApiBackend::Responses;
                }
            } else if auth.eq_ignore_ascii_case("openrouter") {
                if entry.env_key.is_none() {
                    entry.env_key = Some(EnvKeys::single("OPENROUTER_API_KEY"));
                }
                if !model_set_base_url && self.base_url.is_none() {
                    entry.info.base_url = "https://openrouter.ai/api/v1".to_string();
                }
                if !model_set_backend && self.api_backend.is_none() {
                    entry.info.api_backend = ApiBackend::ChatCompletions;
                }
            }
        }
        for (key, value) in &self.extra_headers {
            entry
                .info
                .extra_headers
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
        entry.info.supported_in_api = true;
    }
}

/// Parse `[provider.<name>]` tables from raw config.toml.
pub(crate) fn parse(raw_config: &toml::Value) -> IndexMap<String, ProviderConfig> {
    let mut providers = IndexMap::new();
    let Some(section) = raw_config.get("provider").and_then(toml::Value::as_table) else {
        return providers;
    };
    for (name, value) in section {
        match value.clone().try_into::<ProviderConfig>() {
            Ok(config) => {
                providers.insert(name.clone(), config);
            }
            Err(error) => {
                tracing::warn!(
                    provider = %name,
                    error = %error,
                    "failed to parse [provider.{name}]; entry skipped"
                );
            }
        }
    }
    providers
}
