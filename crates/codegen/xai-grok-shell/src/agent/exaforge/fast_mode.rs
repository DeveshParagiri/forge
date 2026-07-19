//! Forge-owned fast-mode capability and session policy.
//!
//! Stock model-switch and sampler code call these narrow helpers. Provider
//! identity is resolved when catalog metadata is built; eligibility thereafter
//! depends only on the declared capability.

use crate::agent::config::ModelInfo;
use crate::session::SessionCommand;
use agent_client_protocol as acp;
use tokio::sync::{mpsc, oneshot};

/// ACP metadata key exported for capable models.
pub(crate) const SUPPORTS_META_KEY: &str = "supportsFastMode";
/// ACP request metadata key used to toggle the session state.
pub(crate) const REQUEST_META_KEY: &str = "fastMode";

pub(crate) fn parse_request(meta: Option<&acp::Meta>) -> Option<bool> {
    meta.and_then(|m| m.get(REQUEST_META_KEY))
        .and_then(|v| v.as_bool())
}

pub(crate) fn advertise(info: &ModelInfo, meta: &mut acp::Meta) {
    if info.supports_fast_mode {
        meta.insert(SUPPORTS_META_KEY.to_owned(), serde_json::Value::Bool(true));
    }
}

/// Resolve the next session state for a model switch or `/fast` toggle.
pub(crate) async fn resolve_for_switch(
    cmd_tx: &mpsc::UnboundedSender<SessionCommand>,
    supports_fast_mode: bool,
    requested: Option<bool>,
) -> bool {
    if !supports_fast_mode {
        return false;
    }
    if let Some(enabled) = requested {
        return enabled;
    }
    let (tx, rx) = oneshot::channel();
    if cmd_tx
        .send(SessionCommand::GetSamplingFastMode { responds_to: tx })
        .is_err()
    {
        return false;
    }
    rx.await.unwrap_or(false)
}

/// Copy the ACP session selection into the provider-neutral sampler config.
pub(crate) fn apply_to_sampler_config(config: &xai_grok_sampling_types::SamplingConfig) -> bool {
    config.fast_mode.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_metadata_is_typed_and_defaults_absent() {
        assert_eq!(parse_request(None), None);
        let mut meta = acp::Meta::new();
        meta.insert(REQUEST_META_KEY.into(), serde_json::Value::Bool(true));
        assert_eq!(parse_request(Some(&meta)), Some(true));
        meta.insert(
            REQUEST_META_KEY.into(),
            serde_json::Value::String("true".into()),
        );
        assert_eq!(parse_request(Some(&meta)), None);
    }
}
