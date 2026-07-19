//! Theme-package behavior owned by the Forge extension.

use crate::theme::ThemeKind;

/// Package default for `[ui].show_shortcuts_bar` when the key is unset.
/// `None` preserves the global client default.
#[must_use]
pub(crate) const fn package_show_shortcuts_bar_default(kind: ThemeKind) -> Option<bool> {
    match kind {
        ThemeKind::Forge => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_package_policy_is_opted_in_only_for_forge() {
        assert_eq!(
            package_show_shortcuts_bar_default(ThemeKind::Forge),
            Some(false)
        );

        for kind in [
            ThemeKind::GrokNight,
            ThemeKind::GrokDay,
            ThemeKind::TokyoNight,
            ThemeKind::RosePineMoon,
            ThemeKind::OscuraMidnight,
            ThemeKind::Auto,
        ] {
            assert_eq!(package_show_shortcuts_bar_default(kind), None);
        }
    }
}
