//! Forge policy for showing external harness sessions in `/sessions`.
//!
//! Upstream owns discovery, normalization, picker merging, and `/resume-*`
//! dispatch. Forge contributes only the explicit opt-in gate and harness labels.

use xai_grok_workspace::foreign_sessions::EnabledForeignSessionSources;

const ENV_FLAG: &str = "FORGE_SHOW_EXTERNAL_SESSIONS";

/// Resolve whether `/sessions` may include sessions from external harnesses.
///
/// Precedence is environment over `[sessions].show_external`; the default is
/// intentionally off. When enabled, upstream compatibility settings still
/// decide which individual sources may be scanned.
pub(crate) fn enabled_sources(
    config: Option<&toml::Value>,
    compatible: EnabledForeignSessionSources,
) -> EnabledForeignSessionSources {
    enabled_sources_from(
        std::env::var(ENV_FLAG).ok().as_deref(),
        config_value(config),
        compatible,
    )
}

fn enabled_sources_from(
    env: Option<&str>,
    config: Option<bool>,
    compatible: EnabledForeignSessionSources,
) -> EnabledForeignSessionSources {
    if show_external_from(env, config) {
        compatible
    } else {
        EnabledForeignSessionSources::default()
    }
}

fn config_value(config: Option<&toml::Value>) -> Option<bool> {
    config?.get("sessions")?.get("show_external")?.as_bool()
}

fn show_external_from(env: Option<&str>, config: Option<bool>) -> bool {
    env.and_then(parse_bool).or(config).unwrap_or(false)
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Human-readable harness name shown beside external `/sessions` entries.
pub(crate) fn harness_badge(source: &str) -> Option<&'static str> {
    match source {
        "claude" => Some("Claude Code"),
        "codex" => Some("Codex"),
        "cursor" => Some("Cursor"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compatible() -> EnabledForeignSessionSources {
        EnabledForeignSessionSources {
            claude: true,
            codex: true,
            cursor: false,
        }
    }

    #[test]
    fn external_sessions_are_opt_in() {
        assert_eq!(
            enabled_sources_from(None, None, compatible()),
            EnabledForeignSessionSources::default()
        );
    }

    #[test]
    fn config_flag_enables_only_compatible_sources() {
        let config: toml::Value = toml::from_str("[sessions]\nshow_external = true\n").unwrap();
        assert_eq!(config_value(Some(&config)), Some(true));
        assert_eq!(
            enabled_sources_from(None, Some(true), compatible()),
            compatible()
        );
    }

    #[test]
    fn resolver_precedence_and_invalid_env_fallback_are_stable() {
        assert!(show_external_from(Some("on"), Some(false)));
        assert!(!show_external_from(Some("0"), Some(true)));
        assert!(show_external_from(Some("invalid"), Some(true)));
        assert!(!show_external_from(None, None));
    }

    #[test]
    fn external_badges_use_harness_names() {
        assert_eq!(harness_badge("claude"), Some("Claude Code"));
        assert_eq!(harness_badge("codex"), Some("Codex"));
        assert_eq!(harness_badge("local"), None);
    }
}
