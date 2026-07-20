use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::identity::{ProviderId, provider_id_for_base};
use crate::agent::config::{ConfigModelOverride, ModelEntry};
use crate::agent::models::ModelGlobSet;

/// Per-provider model allow/deny list from `[catalog.*]`.
///
/// An empty `include` means all models for that provider. `exclude` always
/// wins. Patterns use the same glob syntax as `[models].allowed_models`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderCatalogRule {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// Provider-scoped catalog configuration.
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

pub(crate) fn validate_filters(config: &ProviderCatalogConfig) -> Result<(), String> {
    for (provider, rule) in config.configured() {
        for (field, list) in [("include", &rule.include), ("exclude", &rule.exclude)] {
            if let Err(bad) = ModelGlobSet::compile(Some(list)) {
                return Err(format!(
                    "catalog.{}.{field} has an invalid pattern: {}. Patterns use * and ? wildcards.",
                    provider.catalog_key(),
                    bad.join(", ")
                ));
            }
        }
    }
    Ok(())
}

/// Apply provider packs to parsed model entries after model-local overrides.
pub(crate) fn apply_provider_override(
    providers: &IndexMap<String, super::provider_config::ProviderConfig>,
    model_override: &ConfigModelOverride,
    key: &str,
    entry: &mut ModelEntry,
) {
    let Some(ref provider_name) = model_override.provider else {
        return;
    };
    if let Some(provider) = providers.get(provider_name) {
        // An explicit `auth_provider` owns this model's credential: the pack must
        // not inject static/inferred keys over it, or `own_credential()` would
        // shadow the helper (see `ModelEntry::effective_auth_provider`).
        let creds_governed = model_override.api_key.is_some()
            || model_override.env_key.is_some()
            || model_override.auth_provider.is_some();
        provider.apply_to_entry(
            entry,
            model_override.base_url.is_some(),
            model_override.api_backend.is_some(),
            creds_governed,
        );
    } else {
        tracing::warn!(
            model_key = %key,
            provider = %provider_name,
            "model references unknown [provider.{provider_name}]; check config.toml"
        );
    }
}

/// Refine the stock global allowlist using configured provider catalog rules.
pub(crate) fn apply_policy(
    config: &ProviderCatalogConfig,
    catalog: &mut IndexMap<String, ModelEntry>,
) {
    for (provider, rule) in config.configured() {
        // A configured OpenRouter catalog is useful only with a key. Keep the
        // entries in the internal catalog for routing/history, but do not show
        // them in `/model` until auth exists. Other providers keep their
        // existing login-on-selection behavior.
        let auth_ready =
            provider != ProviderId::Openrouter || super::status::status_for(provider).is_ready();
        let (include, exclude) = match (
            ModelGlobSet::compile(Some(&rule.include)),
            ModelGlobSet::compile(Some(&rule.exclude)),
        ) {
            (Ok(include), Ok(exclude)) => (include, exclude),
            (include, exclude) => {
                tracing::error!(
                    provider = provider.as_str(),
                    include_error = ?include.err(),
                    exclude_error = ?exclude.err(),
                    "invalid provider catalog filter; hiding provider models"
                );
                for entry in catalog.values_mut() {
                    if provider_id_for_base(&entry.info.base_url) == Some(provider) {
                        entry.info.user_selectable = false;
                    }
                }
                continue;
            }
        };
        for (key, entry) in catalog.iter_mut() {
            if provider_id_for_base(&entry.info.base_url) != Some(provider) {
                continue;
            }
            let included = include
                .as_ref()
                .map(|set| set.matches(key, &entry.model))
                .unwrap_or(true);
            let excluded = exclude
                .as_ref()
                .is_some_and(|set| set.matches(key, &entry.model));
            entry.info.user_selectable &= included && !excluded && auth_ready;
        }
    }
}

#[cfg(test)]
mod provider_override_tests {
    use super::*;
    use crate::agent::config::{EndpointsConfig, EnvKeys};
    use crate::agent::forge::provider_config::ProviderConfig;

    fn pack() -> IndexMap<String, ProviderConfig> {
        let mut providers = IndexMap::new();
        providers.insert(
            "custom".to_string(),
            ProviderConfig {
                base_url: Some("https://vendor.example/v1".to_string()),
                api_key: Some("pack-static-key".to_string()),
                env_key: Some(EnvKeys::single("PACK_ENV_KEY")),
                supports_fast_mode: true,
                ..ProviderConfig::default()
            },
        );
        providers
    }

    /// A model that declares an explicit `auth_provider` must keep the helper
    /// in charge of credentials: the pack contributes endpoint/backend/Fast
    /// capability but never injects static/inferred keys that would let
    /// `own_credential()` shadow the provider.
    #[test]
    fn explicit_auth_provider_blocks_pack_credential_injection() {
        let providers = pack();
        let model_override = ConfigModelOverride {
            provider: Some("custom".to_string()),
            auth_provider: Some("mint-helper".to_string()),
            ..ConfigModelOverride::default()
        };
        let mut entry = ModelEntry::fallback("m", &EndpointsConfig::default());
        apply_provider_override(&providers, &model_override, "m", &mut entry);

        // Credentials stay ungoverned by the pack.
        assert_eq!(entry.api_key, None, "pack must not inject a static key");
        assert_eq!(entry.env_key, None, "pack must not inject an env key");
        // Non-credential capability still flows through.
        assert_eq!(entry.info.base_url, "https://vendor.example/v1");
        assert!(entry.info.supports_fast_mode, "Fast capability preserved");
    }

    /// Without an `auth_provider` (and no model-local key), the pack still
    /// supplies its own static/inferred credentials as before.
    #[test]
    fn pack_injects_credentials_without_auth_provider() {
        let providers = pack();
        let model_override = ConfigModelOverride {
            provider: Some("custom".to_string()),
            ..ConfigModelOverride::default()
        };
        let mut entry = ModelEntry::fallback("m", &EndpointsConfig::default());
        apply_provider_override(&providers, &model_override, "m", &mut entry);

        assert_eq!(entry.api_key.as_deref(), Some("pack-static-key"));
        assert!(entry.info.supports_fast_mode);
    }
}
