use crate::config::AgentScope;
use crate::discovery::{SubagentEntry, SubagentSource};
use xai_grok_tools::types::config_source::ConfigSource;

const EXTERNAL_SUBAGENTS: [(&str, &str); 2] = [
    (
        "claude-code",
        "External Claude Code CLI running headlessly with structured streaming. Use for an independent Claude Code implementation or review perspective; it uses CLI-native tools and authentication rather than Exaforge's hosted tools.",
    ),
    (
        "codex-cli",
        "External Codex CLI running headlessly with structured streaming. Use for an independent Codex implementation or review perspective; it uses CLI-native tools and authentication rather than Exaforge's hosted tools.",
    ),
];

/// Add Exaforge's external harness adapters to the discoverable task roster.
pub(crate) fn append(
    entries: &mut Vec<SubagentEntry>,
    toggle: &std::collections::HashMap<String, bool>,
) {
    for (name, description) in EXTERNAL_SUBAGENTS {
        if toggle.get(name).copied() == Some(false)
            || entries.iter().any(|entry| entry.name == name)
        {
            continue;
        }
        entries.push(SubagentEntry {
            name: name.to_owned(),
            description: description.to_owned(),
            source: SubagentSource::UserDefined {
                scope: AgentScope::Bundled,
            },
            shadows_builtin: None,
            config_source: ConfigSource::Builtin,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_both_external_harnesses_and_honors_toggle() {
        let mut entries = Vec::new();
        append(
            &mut entries,
            &std::collections::HashMap::from([("codex-cli".to_owned(), false)]),
        );
        assert!(entries.iter().any(|entry| entry.name == "claude-code"));
        assert!(!entries.iter().any(|entry| entry.name == "codex-cli"));
    }
}
