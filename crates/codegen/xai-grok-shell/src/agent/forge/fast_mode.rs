//! Forge-owned fast-mode capability and session policy.
//!
//! Fast Mode is a session-only sampling mutation. It intentionally does not
//! accept a model id, so toggling it cannot select a stale pager model.

use crate::agent::MvpAgent;
use crate::agent::config::ModelInfo;
use crate::extensions::{ExtResult, parse_params};
use crate::session::SessionCommand;
use agent_client_protocol as acp;
use tokio::sync::{mpsc, oneshot};

/// ACP metadata key exported for capable models.
pub(crate) const SUPPORTS_META_KEY: &str = "supportsFastMode";
/// Dedicated extension used to mutate only the live session's Fast Mode flag.
pub(crate) const EXT_METHOD: &str = "x.ai/session/fast_mode";

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetFastModeRequest {
    session_id: String,
    enabled: bool,
}

pub(crate) fn advertise(info: &ModelInfo, meta: &mut acp::Meta) {
    if info.supports_fast_mode {
        meta.insert(SUPPORTS_META_KEY.to_owned(), serde_json::Value::Bool(true));
    }
}

/// Update only `sampling_config.fast_mode` for the authoritative live session.
pub(crate) async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let request: SetFastModeRequest = parse_params(args)?;
    let session_id = acp::SessionId::new(request.session_id);

    // Resolve an in-flight session/load before taking the mutation lock. A load
    // may itself apply a restored model under this lock, so waiting while the
    // lock is held would stall both operations until the load timeout.
    let handle = agent
        .session_handle_waiting_for_load(&session_id)
        .await
        .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
    // Serialize with user model switches. Whichever request acquires the lock
    // first wins; a following Fast request validates against the switched model,
    // while a following model switch preserves the Fast state set here.
    let dispatch_lock = agent.dispatch_lock(&session_id);
    let _dispatch_guard = dispatch_lock.lock().await;

    let (tx, rx) = oneshot::channel();
    handle
        .cmd_tx
        .send(SessionCommand::SetSamplingFastMode {
            enabled: request.enabled,
            responds_to: tx,
        })
        .map_err(|_| acp::Error::internal_error().data("session actor is unavailable"))?;
    let live_model = rx
        .await
        .map_err(|_| acp::Error::internal_error().data("session actor did not respond"))??;

    xai_grok_telemetry::unified_log::info(
        "fast mode changed",
        Some(session_id.0.as_ref()),
        Some(serde_json::json!({
            "model": live_model,
            "enabled": request.enabled,
        })),
    );
    crate::extensions::to_raw_response(&serde_json::json!({ "enabled": request.enabled }))
}

/// Preserve Fast Mode across capable model switches and clear it when the new
/// model does not advertise support.
pub(crate) async fn resolve_for_switch(
    cmd_tx: &mpsc::UnboundedSender<SessionCommand>,
    supports_fast_mode: bool,
) -> bool {
    if !supports_fast_mode {
        return false;
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
    fn dedicated_request_has_no_model_field() {
        let request: SetFastModeRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-1",
            "enabled": true,
            "modelId": "stale-model",
        }))
        .expect("unknown fields are ignored");
        assert_eq!(request.session_id, "session-1");
        assert!(request.enabled);
    }
}
