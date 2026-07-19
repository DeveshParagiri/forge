//! Pure policy for provider-login question chrome (prefix + footer).
//!
//! Buffer painting and footer allocation stay inline in agent_view/render.rs;
//! this module only decides labels and prefix geometry.

use crate::views::question_view::LocalQuestionKind;

/// Geometry for a direct freeform field (OpenRouter key entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DirectInputPrefix {
    /// Columns occupied by the leading prompt arrow (`❯ `).
    pub leading_w: u16,
}

/// Fixed direct-input prefix: prompt arrow only (no option number/marker).
pub(crate) fn direct_input_prefix() -> DirectInputPrefix {
    DirectInputPrefix { leading_w: 2 }
}

/// Enter-button label for question footers.
pub(crate) fn enter_label(is_direct: bool, is_on_freeform: bool, is_last: bool) -> &'static str {
    if is_direct {
        "submit"
    } else if is_on_freeform {
        "edit"
    } else if is_last {
        "submit"
    } else {
        "select"
    }
}

/// How the question footer left cluster should be built for provider surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FooterLeftPolicy {
    /// Esc cancel only (direct API-key field).
    DirectInput,
    /// Navigate + Esc cancel (provider picker).
    ProviderPicker,
    /// Stock question footer (navigate + y copy, multi-question chrome).
    Standard,
}

pub(crate) fn footer_left_policy(kind: Option<&LocalQuestionKind>) -> FooterLeftPolicy {
    if super::is_direct_input_kind(kind) {
        FooterLeftPolicy::DirectInput
    } else if super::is_provider_picker_kind(kind) {
        FooterLeftPolicy::ProviderPicker
    } else {
        FooterLeftPolicy::Standard
    }
}
