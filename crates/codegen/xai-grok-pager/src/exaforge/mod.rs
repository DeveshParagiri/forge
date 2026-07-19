//! Exaforge personal UI behavior for the pager crate.
//!
//! Bulk fork-specific logic lives here so upstream rebases of stock pager
//! files only re-touch thin `// Exaforge:` hooks.

pub(crate) mod effort;
pub(crate) mod layout;
pub(crate) mod provider_login;
pub(crate) mod welcome;

#[cfg(test)]
mod tests;
