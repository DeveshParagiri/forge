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
        provider.apply_to_entry(
            entry,
            model_override.base_url.is_some(),
            model_override.api_backend.is_some(),
            model_override.api_key.is_some() || model_override.env_key.is_some(),
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
