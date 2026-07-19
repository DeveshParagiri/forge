//! Theme-package behavior owned by the Exaforge extension.

use crate::theme::ThemeKind;

/// Package default for `[ui].show_shortcuts_bar` when the key is unset.
/// `None` preserves the global client default.
#[must_use]
pub(crate) const fn package_show_shortcuts_bar_default(kind: ThemeKind) -> Option<bool> {
    match kind {
        ThemeKind::Claude => Some(false),
        _ => None,
    }
}

/// Whether Shift+Tab cycles reasoning effort instead of permission mode.
#[must_use]
pub(crate) const fn package_shift_tab_cycles_effort(kind: ThemeKind) -> bool {
    matches!(kind, ThemeKind::Claude)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_package_policy_is_opted_in_only_for_claude() {
        assert_eq!(
            package_show_shortcuts_bar_default(ThemeKind::Claude),
            Some(false)
        );
        assert!(package_shift_tab_cycles_effort(ThemeKind::Claude));

        for kind in [
            ThemeKind::GrokNight,
            ThemeKind::GrokDay,
            ThemeKind::TokyoNight,
            ThemeKind::RosePineMoon,
            ThemeKind::OscuraMidnight,
            ThemeKind::Auto,
        ] {
            assert_eq!(package_show_shortcuts_bar_default(kind), None);
            assert!(!package_shift_tab_cycles_effort(kind));
        }
    }
}
