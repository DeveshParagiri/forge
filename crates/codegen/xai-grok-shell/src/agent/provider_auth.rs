//! Compatibility exports for the Forge provider integration.
//!
//! New shell code should use [`crate::agent::forge`]. This module remains
//! public because the pager crate consumes the provider login API.

pub use crate::agent::forge::*;
