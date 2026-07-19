//! Compatibility exports for the Exaforge provider integration.
//!
//! New shell code should use [`crate::agent::exaforge`]. This module remains
//! public because the pager crate consumes the provider login API.

pub use crate::agent::exaforge::*;
