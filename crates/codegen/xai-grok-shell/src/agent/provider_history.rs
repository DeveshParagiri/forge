//! Personal: portable history transform for cross-provider model switches.
//!
//! User, assistant, and tool context remains intact. Responses API reasoning
//! items are deliberately removed because their `encrypted_content` and
//! `rs_*` identities are scoped to the provider/account that minted them.

use xai_grok_sampling_types::ConversationItem;

/// Strip non-portable opaque reasoning and return the number of removed rows.
pub fn strip_nonportable_reasoning(items: &mut Vec<ConversationItem>) -> usize {
    let before = items.len();
    items.retain(|item| !matches!(item, ConversationItem::Reasoning(_)));
    before.saturating_sub(items.len())
}

/// Apply the transform only when the endpoint resolves to a different
/// provider family. Returns the number of removed reasoning rows.
pub fn strip_for_provider_switch(
    previous_base_url: &str,
    next_base_url: &str,
    items: &mut Vec<ConversationItem>,
) -> usize {
    if crate::agent::provider_auth::provider_scope_for_base(previous_base_url)
        == crate::agent::provider_auth::provider_scope_for_base(next_base_url)
    {
        return 0;
    }
    strip_nonportable_reasoning(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_sampling_types::rs;

    #[test]
    fn keeps_text_and_tools_but_drops_opaque_reasoning() {
        let mut items = vec![
            ConversationItem::system("system"),
            ConversationItem::user("hello"),
            ConversationItem::Reasoning(rs::ReasoningItem {
                id: "rs_foreign".into(),
                summary: Vec::new(),
                content: None,
                encrypted_content: Some("foreign-ciphertext".into()),
                status: None,
            }),
            ConversationItem::assistant("answer"),
            ConversationItem::tool_result("call-1", "tool output"),
        ];

        assert_eq!(strip_nonportable_reasoning(&mut items), 1);
        assert_eq!(items.len(), 4);
        assert!(matches!(items[0], ConversationItem::System(_)));
        assert!(matches!(items[1], ConversationItem::User(_)));
        assert!(matches!(items[2], ConversationItem::Assistant(_)));
        assert!(matches!(items[3], ConversationItem::ToolResult(_)));
    }

    #[test]
    fn grok_to_codex_strips_but_codex_to_codex_keeps_reasoning() {
        let reasoning = || {
            ConversationItem::Reasoning(rs::ReasoningItem {
                id: "rs_foreign".into(),
                summary: Vec::new(),
                content: None,
                encrypted_content: Some("foreign-ciphertext".into()),
                status: None,
            })
        };
        let mut cross_provider = vec![ConversationItem::user("hello"), reasoning()];
        assert_eq!(
            strip_for_provider_switch(
                "https://api.x.ai/v1",
                "https://chatgpt.com/backend-api/codex",
                &mut cross_provider,
            ),
            1
        );
        assert_eq!(cross_provider.len(), 1);

        let mut same_provider = vec![ConversationItem::user("hello"), reasoning()];
        assert_eq!(
            strip_for_provider_switch(
                "https://chatgpt.com/backend-api/codex",
                "https://chatgpt.com/backend-api/codex/",
                &mut same_provider,
            ),
            0
        );
        assert_eq!(same_provider.len(), 2);
    }
}
