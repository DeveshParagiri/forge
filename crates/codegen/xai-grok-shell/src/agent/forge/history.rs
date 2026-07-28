//! Portable history transform for cross-provider model switches.

use xai_grok_sampling_types::ConversationItem;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct NormalizationStats {
    pub reasoning_removed: usize,
    pub backend_tools_flattened: usize,
}

impl NormalizationStats {
    pub fn changed(&self) -> bool {
        self.reasoning_removed > 0 || self.backend_tools_flattened > 0
    }
}

/// Normalize provider-bound history into forms accepted by another provider.
/// Opaque reasoning is removed and typed backend-hosted calls become ordinary
/// assistant text using the shared conversation summary implementation.
pub fn normalize_portable_history(items: &mut Vec<ConversationItem>) -> NormalizationStats {
    let mut stats = NormalizationStats::default();
    items.retain_mut(|item| match item {
        ConversationItem::Reasoning(_) => {
            stats.reasoning_removed += 1;
            false
        }
        ConversationItem::BackendToolCall(call) => {
            *item = ConversationItem::assistant(call.text_summary());
            stats.backend_tools_flattened += 1;
            true
        }
        _ => true,
    });
    stats
}

/// Apply the portable-history transform only when the endpoint resolves to a
/// different provider family.
///
/// TODO: Make the read/transform/write operation atomic in the dedicated
/// history correctness follow-up. This preserves the current hook semantics.
pub fn normalize_for_provider_switch(
    previous_base_url: &str,
    next_base_url: &str,
    items: &mut Vec<ConversationItem>,
) -> NormalizationStats {
    if super::identity::provider_scope_for_base(previous_base_url)
        == super::identity::provider_scope_for_base(next_base_url)
    {
        return NormalizationStats::default();
    }
    normalize_portable_history(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_sampling_types::{BackendToolCallItem, BackendToolKind, rs};

    fn reasoning() -> ConversationItem {
        ConversationItem::Reasoning(rs::ReasoningItem {
            id: "rs_foreign".into(),
            summary: Vec::new(),
            content: None,
            encrypted_content: Some("foreign-ciphertext".into()),
            status: None,
        })
    }

    fn backend_search() -> ConversationItem {
        ConversationItem::BackendToolCall(BackendToolCallItem {
            kind: BackendToolKind::WebSearch(rs::WebSearchToolCall {
                id: "ws_foreign".into(),
                status: rs::WebSearchToolCallStatus::Completed,
                action: rs::WebSearchToolCallAction::Search(rs::WebSearchActionSearch {
                    query: "portable query".into(),
                    sources: Some(Vec::new()),
                }),
            }),
        })
    }

    #[test]
    fn drops_reasoning_and_flattens_backend_tools_but_keeps_other_history() {
        let mut items = vec![
            ConversationItem::system("system"),
            ConversationItem::user("hello"),
            reasoning(),
            backend_search(),
            ConversationItem::assistant("answer"),
            ConversationItem::tool_result("call-1", "tool output"),
        ];

        assert_eq!(
            normalize_portable_history(&mut items),
            NormalizationStats {
                reasoning_removed: 1,
                backend_tools_flattened: 1,
            }
        );
        assert_eq!(items.len(), 5);
        assert!(matches!(items[0], ConversationItem::System(_)));
        assert!(matches!(items[1], ConversationItem::User(_)));
        let ConversationItem::Assistant(summary) = &items[2] else {
            panic!("backend tool must become assistant text");
        };
        assert_eq!(
            summary.content.as_ref(),
            "[backend web_search] search: portable query"
        );
        assert!(matches!(items[3], ConversationItem::Assistant(_)));
        assert!(matches!(items[4], ConversationItem::ToolResult(_)));
    }

    #[test]
    fn cross_provider_normalizes_but_same_provider_keeps_native_items() {
        let mut cross_provider = vec![
            ConversationItem::user("hello"),
            reasoning(),
            backend_search(),
        ];
        assert_eq!(
            normalize_for_provider_switch(
                "https://api.x.ai/v1",
                "https://chatgpt.com/backend-api/codex",
                &mut cross_provider,
            ),
            NormalizationStats {
                reasoning_removed: 1,
                backend_tools_flattened: 1,
            }
        );
        assert_eq!(cross_provider.len(), 2);

        let mut same_provider = vec![
            ConversationItem::user("hello"),
            reasoning(),
            backend_search(),
        ];
        assert_eq!(
            normalize_for_provider_switch(
                "https://chatgpt.com/backend-api/codex",
                "https://chatgpt.com/backend-api/codex/",
                &mut same_provider,
            ),
            NormalizationStats::default()
        );
        assert_eq!(same_provider.len(), 3);
        assert!(matches!(same_provider[1], ConversationItem::Reasoning(_)));
        assert!(matches!(
            same_provider[2],
            ConversationItem::BackendToolCall(_)
        ));
    }
}
