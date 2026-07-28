//! Compatibility exports for Forge provider identity and usage.
//!
//! New shell code should use [`crate::agent::forge`]. This module remains
//! public because the pager consumes the provider-aware usage API.

pub use crate::agent::forge::*;
