//! Forge-owned shortcut contributions.
//!
//! Keep fork-specific key semantics and UI metadata here. Upstream-owned input,
//! help, and rendering code consume the composed [`ActionRegistry`] through one
//! narrow construction hook instead of branching on themes or Forge behavior.

use crate::actions::{ActionDef, ActionId, Category, When};
use crate::app::actions::Effect;
use crate::app::app_view::AppView;

/// Dispatch the Forge action contributed for `Shift+Tab`.
pub(crate) fn dispatch_cycle_effort(app: &mut AppView) -> Vec<Effect> {
    super::effort::dispatch_cycle_effort(app)
}

/// Add or replace Forge shortcut definitions in the upstream action list.
///
/// Contributions are keyed by [`ActionId`], so a Forge override updates key
/// dispatch, shortcut help, footer metadata, and conflict checks together.
pub(crate) fn contribute(actions: &mut Vec<ActionDef>) {
    // Shift+Tab belongs to Forge in prompt context. Remove the upstream mode
    // action before registering the distinct extension action so lookup order
    // cannot leave two actions claiming the same chord.
    actions.retain(|def| def.id != ActionId::CycleMode);
    replace(actions, cycle_effort());
}

fn replace(actions: &mut Vec<ActionDef>, contribution: ActionDef) {
    if let Some(existing) = actions.iter_mut().find(|def| def.id == contribution.id) {
        *existing = contribution;
    } else {
        actions.push(contribution);
    }
}

fn cycle_effort() -> ActionDef {
    let keys = crate::input::key::shift_tab_keys();
    ActionDef {
        id: ActionId::CycleEffort,
        label: "effort",
        description: "Cycle reasoning effort",
        default_key: keys[0],
        alt_keys: keys[1..].to_vec(),
        category: Category::GettingStarted,
        context: When::PromptFocused,
        hint_priority: None,
        hint_key_display: Some("Shift+Tab"),
        requires_confirmation: false,
        long_help: Some(
            "Steps through the active model's supported reasoning-effort levels.\n\
             The binding is a Forge action and behaves the same in every color theme.\n\
             Use /plan for plan mode and Ctrl+O or /always-approve for permission mode.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::ActionRegistry;
    use crate::input::key::shift_tab_keys;

    #[test]
    fn contribution_replaces_upstream_action_metadata_without_duplication() {
        let registry = ActionRegistry::defaults();
        let defs: Vec<_> = registry
            .all()
            .iter()
            .filter(|def| def.id == ActionId::CycleEffort)
            .collect();
        assert_eq!(defs.len(), 1);
        assert!(registry.find(ActionId::CycleMode).is_none());
        assert_eq!(defs[0].label, "effort");
        assert_eq!(defs[0].description, "Cycle reasoning effort");
        assert_eq!(defs[0].default_key, shift_tab_keys()[0]);
        assert_eq!(defs[0].alt_keys, shift_tab_keys()[1..]);
    }

    #[test]
    fn all_shift_tab_encodings_resolve_to_the_forge_action() {
        let registry = ActionRegistry::defaults();
        for shortcut in shift_tab_keys() {
            assert_eq!(
                registry.lookup(&shortcut.to_key_event(), When::PromptFocused),
                Some(ActionId::CycleEffort),
                "{shortcut:?} must resolve through the composed registry",
            );
        }
    }
}
