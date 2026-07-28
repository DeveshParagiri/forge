use indexmap::IndexMap;

use crate::agent::config::ResolvedCredentials;

/// Apply the one dynamic ChatGPT subscription header. Static provider headers
/// belong in upstream `[model_providers.<id>.extra_headers]` configuration.
pub(crate) fn apply_headers(headers: &mut IndexMap<String, String>, base_url: &str) {
    if super::identity::is_codex_base(base_url)
        && let Some(account_id) = super::credentials::read_codex_account_id()
    {
        headers
            .entry("ChatGPT-Account-Id".to_string())
            .or_insert(account_id);
    }
}

/// Provider-specific turn settings applied at the sampler boundary.
pub(crate) struct TurnProfile {
    pub api_key: Option<String>,
    pub use_bearer_resolver: bool,
    pub stream_tool_calls: bool,
    pub user_id: Option<String>,
    pub supports_backend_search: bool,
    pub compactions_remaining: Option<xai_grok_sampling_types::CompactionsRemaining>,
    pub compaction_at_tokens: Option<xai_grok_sampling_types::CompactionAtTokens>,
    pub doom_loop_recovery: Option<xai_grok_sampling_types::DoomLoopRecoveryPolicy>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn turn_profile(
    base_url: &str,
    model_id: &str,
    current_api_key: Option<String>,
    use_bearer_resolver: bool,
    stream_tool_calls: Option<bool>,
    user_id: Option<String>,
    supports_backend_search: bool,
    compactions_remaining: Option<xai_grok_sampling_types::CompactionsRemaining>,
    compaction_at_tokens: Option<xai_grok_sampling_types::CompactionAtTokens>,
    doom_loop_recovery: Option<xai_grok_sampling_types::DoomLoopRecoveryPolicy>,
) -> TurnProfile {
    let third_party = super::identity::is_third_party_model_base(base_url);
    let api_key = if third_party {
        crate::agent::config::try_resolve_model_credentials(model_id, None)
            .and_then(|ResolvedCredentials { api_key, .. }| api_key)
            .or(current_api_key)
    } else {
        current_api_key
    };
    TurnProfile {
        api_key,
        use_bearer_resolver: use_bearer_resolver && !third_party,
        stream_tool_calls: if third_party {
            false
        } else {
            stream_tool_calls.unwrap_or(false)
        },
        user_id: (!third_party).then_some(user_id).flatten(),
        supports_backend_search: !third_party && supports_backend_search,
        compactions_remaining: (!third_party).then_some(compactions_remaining).flatten(),
        compaction_at_tokens: (!third_party).then_some(compaction_at_tokens).flatten(),
        doom_loop_recovery: (!third_party).then_some(doom_loop_recovery).flatten(),
    }
}
