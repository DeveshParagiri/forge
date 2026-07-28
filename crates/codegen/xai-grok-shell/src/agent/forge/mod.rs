//! Forge multi-provider extensions.
//!
//! Generic endpoints and credentials use upstream model-provider config. This
//! module keeps only provider identity, the narrow ChatGPT subscription shim,
//! catalog policy, request profiles, usage, and cross-provider history.

pub mod catalog;
pub mod credentials;
pub(crate) mod fast_mode;
pub mod history;
pub mod identity;
pub mod profile;
pub mod usage;

pub use catalog::{ProviderCatalogConfig, ProviderCatalogRule};
pub use credentials::{
    codex_auth_path, env_requests_codex_token, read_codex_access_token, read_codex_account_id,
};
pub use identity::{
    ProviderId, display_model_name, is_third_party_model_base, provider_id_for_base,
    provider_scope_for_base,
};
pub use usage::{ProviderUsageSnapshot, UsageCredits, UsageWindow, fetch_provider_usage};
