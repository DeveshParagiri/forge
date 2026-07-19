//! Exaforge multi-provider extensions.
//!
//! Provider identity, config packs, credentials, status, catalog policy,
//! request profiles, and cross-provider history handling live here so the
//! stock agent config and catalog code retain only narrow integration hooks.

pub mod catalog;
pub mod credentials;
pub mod history;
pub mod identity;
pub mod profile;
pub mod provider_config;
pub mod status;

pub use catalog::{ProviderCatalogConfig, ProviderCatalogRule};
pub use credentials::{
    ProviderKeyEntry, ProviderKeysFile, codex_auth_path, env_requests_codex_token,
    env_requests_openrouter_token, load_provider_keys, provider_keys_path, read_codex_access_token,
    read_codex_account_id, read_openrouter_api_key, save_provider_keys, set_openrouter_api_key,
};
pub use identity::{
    ProviderId, display_model_name, is_third_party_model_base, provider_id_for_base,
    provider_scope_for_base,
};
pub use provider_config::ProviderConfig;
pub use status::{
    ProviderAuthStatus, codex_status, login_picker_providers, openrouter_status,
    picker_auth_status, spacexai_status, status_for,
};
