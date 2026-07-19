//! Forge-owned wire mapping for the generic fast inference capability.
//!
//! Session and capability handling stay provider-neutral. Only the compatible
//! Codex Responses adapter calls this helper to emit its concrete request field.

use serde_json::{Map, Value, json};

pub(crate) fn apply_codex_request_option(body: &mut Map<String, Value>, enabled: bool) {
    if enabled {
        body.insert("service_tier".into(), json!("priority"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_priority_tier_is_present_only_while_enabled() {
        let mut disabled = Map::new();
        apply_codex_request_option(&mut disabled, false);
        assert!(!disabled.contains_key("service_tier"));

        let mut enabled = Map::new();
        apply_codex_request_option(&mut enabled, true);
        assert_eq!(enabled.get("service_tier"), Some(&json!("priority")));
    }
}
