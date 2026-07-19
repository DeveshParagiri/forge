//! Forge-owned external-session support for the `/sessions` dashboard.
//!
//! Upstream owns foreign-session discovery for the `/resume` picker. Forge adds
//! an explicit dashboard opt-in and presentation policy without duplicating the
//! store readers.

use std::path::Path;

use xai_grok_workspace::foreign_sessions::{
    EnabledForeignSessionSources, ForeignSessionSummary, ForeignSessionTool,
};

use crate::views::dashboard::{DashboardRow, DashboardRowId, RowState};

/// In-memory dashboard state for external sessions. This state is deliberately
/// separate from the `/resume` picker's sequence/coordinator so opening one
/// surface cannot cancel or overwrite the other surface's scan.
#[derive(Debug, Clone, Default)]
pub(crate) struct DashboardExternalSessions {
    pub(crate) enabled: bool,
    pub(crate) loading: bool,
    pub(crate) seq: u64,
    pub(crate) entries: Vec<ForeignSessionSummary>,
    pub(crate) coordinator: crate::app::ForeignScanCoordinator,
}

/// Parse Forge's master opt-in. Missing or malformed values fail closed.
pub(crate) fn show_external_from_config(config: Option<&toml::Value>) -> bool {
    config
        .and_then(|root| root.get("sessions"))
        .and_then(|sessions| sessions.get("show_external"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false)
}

/// Apply the master opt-in to the existing per-vendor compatibility gates.
/// Cursor is intentionally excluded: this feature promises Claude and Codex.
pub(crate) fn enabled_sources(
    show_external: bool,
    compat: EnabledForeignSessionSources,
) -> EnabledForeignSessionSources {
    EnabledForeignSessionSources {
        claude: show_external && compat.claude,
        codex: show_external && compat.codex,
        cursor: false,
    }
}

pub(crate) fn display_label(tool: ForeignSessionTool) -> &'static str {
    match tool {
        ForeignSessionTool::Claude => "Claude Code",
        ForeignSessionTool::Codex => "Codex CLI",
        ForeignSessionTool::Cursor => "Cursor",
    }
}

pub(crate) fn resume_prompt(tool: ForeignSessionTool, native_id: &str) -> String {
    let skill = match tool {
        ForeignSessionTool::Claude => "resume-claude",
        ForeignSessionTool::Codex => "resume-codex",
        ForeignSessionTool::Cursor => "resume-cursor",
    };
    format!("/{skill} {native_id}")
}

/// Start (or supersede) a dashboard scan. The existing effect executor keeps
/// store access off the event-loop thread and gates sources on resume skills.
pub(crate) fn scan_effect(
    state: &mut DashboardExternalSessions,
    cwd: &Path,
    compat: EnabledForeignSessionSources,
    grok_home: &Path,
) -> Option<crate::app::actions::Effect> {
    let sources = enabled_sources(state.enabled, compat);
    state.seq = state.seq.wrapping_add(1);
    state.coordinator.begin_request(state.seq);
    state.loading = sources.claude || sources.codex;
    if !state.loading {
        state.entries.clear();
        return None;
    }
    Some(crate::app::actions::Effect::ScanDashboardExternalSessions {
        cwd: cwd.to_path_buf(),
        sources,
        grok_home: grok_home.to_path_buf(),
        coordinator: state.coordinator.clone(),
        seq: state.seq,
    })
}

pub(crate) fn apply_scan(
    state: &mut DashboardExternalSessions,
    seq: u64,
    entries: Vec<ForeignSessionSummary>,
) {
    if seq != state.seq {
        return;
    }
    state.loading = false;
    state.entries = entries
        .into_iter()
        .filter(|entry| {
            matches!(
                entry.tool,
                ForeignSessionTool::Claude | ForeignSessionTool::Codex
            )
        })
        .collect();
}

/// Append external rows, then apply the dashboard's shared filter/sort policy.
/// Native rows win duplicate identity checks naturally because external row IDs
/// include their source tool and native ID.
pub(crate) fn append_rows(
    rows: &mut Vec<DashboardRow>,
    entries: &[ForeignSessionSummary],
    filter: &crate::views::dashboard::Filter,
    grouping: crate::views::dashboard::Grouping,
    reorder: &[DashboardRowId],
    home: Option<&str>,
) {
    for entry in entries {
        let source = display_label(entry.tool);
        let title = sanitize(&entry.title);
        let label = if title.trim().is_empty() {
            format!("{source} session")
        } else {
            title
        };
        let cwd = entry.cwd.clone();
        rows.push(DashboardRow {
            id: DashboardRowId::External {
                tool: entry.tool,
                native_id: entry.native_id.clone(),
            },
            label,
            // The harness is already identified on the second line. Keep the
            // title clean instead of repeating `[Claude Code]` / `[Codex CLI]`
            // or showing the foreign repository's `HEAD` branch marker.
            subtitle: None,
            // Keep opted-in external rows visible on first open. The dashboard
            // collapses `Inactive` by default, which would hide the very rows
            // this explicit Forge setting asks to expose.
            state: RowState::Idle,
            activity: None,
            secondary_line: Some(source.to_string()),
            cwd_display: crate::views::dashboard::state::compact_cwd(&cwd, home),
            cwd,
            last_change_at: entry.updated_at,
            pinned: false,
            is_active: false,
            badges: Vec::new(),
            context_pct: None,
            indent: 0,
            parent_label: None,
            is_more_placeholder: false,
            more_count: 0,
        });
    }
    crate::views::dashboard::row::apply_filter(rows, filter, home);
    crate::views::dashboard::row::sort_rows(rows, grouping, reorder);
}

fn sanitize(value: &str) -> String {
    crate::views::session_title::sanitize_display_text(value).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_opt_in_fails_closed() {
        assert!(!show_external_from_config(None));
        let missing: toml::Value = toml::from_str("[sessions]\n").unwrap();
        assert!(!show_external_from_config(Some(&missing)));
        let malformed: toml::Value = toml::from_str("[sessions]\nshow_external = 'yes'\n").unwrap();
        assert!(!show_external_from_config(Some(&malformed)));
        let enabled: toml::Value = toml::from_str("[sessions]\nshow_external = true\n").unwrap();
        assert!(show_external_from_config(Some(&enabled)));
    }

    #[test]
    fn source_gate_honors_compat_and_excludes_cursor() {
        let all = EnabledForeignSessionSources {
            claude: true,
            codex: true,
            cursor: true,
        };
        assert_eq!(
            enabled_sources(false, all),
            EnabledForeignSessionSources::default()
        );
        assert_eq!(
            enabled_sources(true, all),
            EnabledForeignSessionSources {
                claude: true,
                codex: true,
                cursor: false,
            }
        );
    }

    #[test]
    fn resume_prompts_preserve_import_semantics() {
        assert_eq!(
            resume_prompt(ForeignSessionTool::Claude, "claude-id"),
            "/resume-claude claude-id"
        );
        assert_eq!(
            resume_prompt(ForeignSessionTool::Codex, "codex-id"),
            "/resume-codex codex-id"
        );
    }

    #[test]
    fn external_rows_are_visible_and_source_labeled() {
        let updated_at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(42);
        let entries = vec![ForeignSessionSummary {
            tool: ForeignSessionTool::Claude,
            source: xai_grok_workspace::foreign_sessions::ForeignSessionSource::ClaudeCode,
            native_id: "native-1".into(),
            title: "Fix the dashboard".into(),
            cwd: std::path::PathBuf::from("/repo"),
            updated_at,
            branch: Some("dev".into()),
        }];
        let mut rows = Vec::new();
        append_rows(
            &mut rows,
            &entries,
            &crate::views::dashboard::Filter::None,
            crate::views::dashboard::Grouping::State,
            &[],
            None,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, RowState::Idle);
        assert_eq!(rows[0].subtitle, None);
        assert!(rows[0].badges.is_empty());
        assert_eq!(rows[0].secondary_line.as_deref(), Some("Claude Code"));
        assert!(matches!(
            rows[0].id,
            DashboardRowId::External {
                tool: ForeignSessionTool::Claude,
                ref native_id,
            } if native_id == "native-1"
        ));
    }
}
