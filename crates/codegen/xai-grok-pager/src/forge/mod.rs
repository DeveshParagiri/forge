//! Forge fork UI behavior for the pager crate.
//!
//! Bulk fork-specific logic lives here so upstream rebases of stock pager
//! files only re-touch thin `// Forge:` hooks.

pub(crate) mod effort;
pub(crate) mod external_sessions;
pub(crate) mod fast_mode;
pub(crate) mod layout;
pub(crate) mod model_label;
pub(crate) mod provider_usage;
pub(crate) mod remote_bridge;
pub(crate) mod remote_control;
pub(crate) mod remote_state;
pub(crate) mod remote_usage;
pub(crate) mod sessions;
pub(crate) mod shortcuts;
pub(crate) mod welcome;

#[cfg(test)]
mod tests;
