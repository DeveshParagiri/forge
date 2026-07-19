//! Provider-login input translation and Esc cancel predicates.

use crate::app::actions::Action;
use crate::app::app_view::InputOutcome;
use crate::views::question_view::{LocalQuestionKind, QuestionSelection, QuestionViewState};

/// Dedicated freeform field (OpenRouter key) rather than option-list + Other.
pub(crate) fn is_direct_input_kind(kind: Option<&LocalQuestionKind>) -> bool {
    matches!(kind, Some(LocalQuestionKind::OpenRouterApiKey))
}

/// Provider picker option list (no freeform row).
pub(crate) fn is_provider_picker_kind(kind: Option<&LocalQuestionKind>) -> bool {
    matches!(kind, Some(LocalQuestionKind::ProviderLogin))
}

/// InputMode Esc should cancel the whole dialog (not step back to Navigation).
pub(crate) fn esc_cancels_direct_input(kind: Option<&LocalQuestionKind>) -> bool {
    is_direct_input_kind(kind)
}

/// Navigation Esc should cancel provider auth surfaces as true dialogs.
///
/// ProjectSelect stays stock and is not included here.
pub(crate) fn esc_cancels_provider_dialog(kind: Option<&LocalQuestionKind>) -> bool {
    matches!(
        kind,
        Some(LocalQuestionKind::ProviderLogin | LocalQuestionKind::OpenRouterApiKey)
    )
}

/// Translate a local provider-login submit into an action, if applicable.
///
/// Returns `None` when `kind` is not a provider-login surface (caller continues
/// with stock arms). Returns `Some` for OpenRouter key + ProviderLogin picks.
pub(crate) fn translate_provider_login_submit(
    qv: &QuestionViewState,
    kind: &LocalQuestionKind,
) -> Option<InputOutcome> {
    match kind {
        LocalQuestionKind::OpenRouterApiKey => {
            let freeform = qv
                .per_question_freeform
                .first()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_default();
            Some(if freeform.is_empty() {
                InputOutcome::Changed
            } else {
                InputOutcome::Action(Action::OpenRouterKeySubmitted { api_key: freeform })
            })
        }
        LocalQuestionKind::ProviderLogin => {
            let Some(QuestionSelection::Single(Some(idx))) = qv.selections.first() else {
                return Some(InputOutcome::Changed);
            };
            let provider_id = qv
                .questions
                .first()
                .and_then(|q| q.options.get(*idx))
                .and_then(|o| o.id.clone())
                .unwrap_or_default();
            Some(InputOutcome::Action(Action::ProviderLoginSelected {
                provider_id,
            }))
        }
        _ => None,
    }
}
