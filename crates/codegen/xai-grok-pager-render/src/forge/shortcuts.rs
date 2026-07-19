//! Thread-local state and resolution for Forge shortcut policy.

use std::cell::Cell;

use xai_grok_shared::ui_config::UiConfig;

/// Bottom contextual shortcuts bar default — single-sourced from `UiConfig`.
const SHORTCUTS_BAR_DEFAULT: bool = UiConfig::SHOW_SHORTCUTS_BAR_DEFAULT;

thread_local! {
    /// Explicit override from config.toml or settings (`Some`); when `None`,
    /// the active theme package may supply a default (Forge → hide footer).
    static SHORTCUTS_BAR_EXPLICIT: Cell<Option<bool>> = const { Cell::new(None) };
    static SHORTCUTS_BAR_LOADED: Cell<bool> = const { Cell::new(false) };
}

/// Resolve `[ui].show_shortcuts_bar`.
///
/// Precedence: explicit config / settings → active theme package default
/// (Forge hides the bar) → global default (show).
pub fn load_show_shortcuts_bar() -> bool {
    SHORTCUTS_BAR_LOADED.with(|loaded| {
        if !loaded.get() {
            let explicit = load_optional_bool_from_effective_config("show_shortcuts_bar");
            SHORTCUTS_BAR_EXPLICIT.with(|current| current.set(explicit));
            loaded.set(true);
        }
    });
    if let Some(value) = SHORTCUTS_BAR_EXPLICIT.with(Cell::get) {
        return value;
    }
    crate::theme::cache::current_kind()
        .package_show_shortcuts_bar_default()
        .unwrap_or(SHORTCUTS_BAR_DEFAULT)
}

fn load_optional_bool_from_effective_config(key: &str) -> Option<bool> {
    let root = xai_grok_config::load_effective_config_disk_only().ok()?;
    root.get("ui")
        .and_then(|ui| ui.get(key))
        .and_then(|value| value.as_bool())
}

pub fn set_show_shortcuts_bar(enabled: bool) {
    SHORTCUTS_BAR_EXPLICIT.with(|current| current.set(Some(enabled)));
    SHORTCUTS_BAR_LOADED.with(|loaded| loaded.set(true));
}

/// Seed shortcut state from the live `UiConfig` without collapsing an unset
/// value into the global default; package policy must remain free to apply.
pub(crate) fn prime(ui: &UiConfig) {
    if let Some(value) = ui.show_shortcuts_bar {
        set_show_shortcuts_bar(value);
    } else {
        SHORTCUTS_BAR_EXPLICIT.with(|current| current.set(None));
        SHORTCUTS_BAR_LOADED.with(|loaded| loaded.set(true));
    }
}

#[cfg(test)]
pub(crate) const fn shortcuts_bar_default() -> bool {
    SHORTCUTS_BAR_DEFAULT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{self, ThemeKind};

    #[test]
    fn resolution_precedence_is_explicit_then_package_then_global() {
        let _guard = theme::cache::test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        theme::cache::reset_for_test();

        theme::cache::set(ThemeKind::Forge);
        prime(&UiConfig::default());
        assert!(!load_show_shortcuts_bar());
        set_show_shortcuts_bar(true);
        assert!(load_show_shortcuts_bar());

        theme::cache::set(ThemeKind::GrokNight);
        prime(&UiConfig {
            show_shortcuts_bar: Some(false),
            ..UiConfig::default()
        });
        assert!(!load_show_shortcuts_bar());
        prime(&UiConfig::default());
        assert!(load_show_shortcuts_bar());

        theme::cache::reset_for_test();
    }

    #[test]
    fn shortcut_override_remains_thread_local() {
        std::thread::spawn(|| {
            set_show_shortcuts_bar(false);
            std::thread::spawn(|| {
                set_show_shortcuts_bar(true);
                assert!(load_show_shortcuts_bar());
            })
            .join()
            .expect("nested shortcut cache test thread should finish");
            assert!(!load_show_shortcuts_bar());
        })
        .join()
        .expect("shortcut cache test thread should finish");
    }
}
