//! Session-pinned bridge between the private browser gateway and the pager.
//!
//! Commands enter through a bounded channel that directly wakes the pager
//! event loop. Every mutation goes through the same pager reducers and
//! interaction response senders as terminal input. The gateway sees only an
//! authoritative browser view model, never raw ACP frames.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use agent_client_protocol as acp;
use tokio::sync::{Mutex, broadcast, mpsc, watch};
use xai_grok_shell::remote_control::{
    RemoteCommand, RemoteCommandOutcome, RemoteCommandRequest, RemoteError,
    RemoteInteractionResponse, RemotePagerEvent, RemotePlanOutcome, RemoteRevocationReason,
    RemoteSessionAcceptance, RemoteSnapshot, RemoteTransport,
};

use crate::app::actions::Effect;
use crate::app::agent::AgentId;
use crate::app::app_view::AppView;
use crate::forge::remote_state::{interaction_key_for_browser_id, project_session};
use crate::forge::remote_usage::RemoteUsageRequestIdentity;

const COMMAND_CAPACITY: usize = 32;
const EVENT_CAPACITY: usize = 256;
#[cfg(not(test))]
const SESSION_ACCEPTANCE_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(test)]
const SESSION_ACCEPTANCE_TIMEOUT: Duration = Duration::from_millis(100);
#[cfg(not(test))]
const SESSION_ACCEPTANCE_COMMIT_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(test)]
const SESSION_ACCEPTANCE_COMMIT_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteTarget {
    agent_id: AgentId,
    session_id: acp::SessionId,
    session_binding_epoch: u32,
    bridge_generation: u64,
    gateway_generation: Option<u64>,
}

struct ActiveBridge {
    target: RemoteTarget,
    events: broadcast::Sender<RemotePagerEvent>,
    snapshot_tx: watch::Sender<Option<RemoteSnapshot>>,
    revision: u64,
    last_session: Option<serde_json::Value>,
    interaction_ids: HashMap<String, String>,
    suspended: bool,
}

struct CommandBus {
    tx: mpsc::Sender<RemoteCommandRequest>,
    rx: Mutex<mpsc::Receiver<RemoteCommandRequest>>,
}

static BRIDGES: OnceLock<Mutex<HashMap<u64, ActiveBridge>>> = OnceLock::new();
static COMMAND_BUS: OnceLock<CommandBus> = OnceLock::new();
static LIFECYCLE: OnceLock<Mutex<()>> = OnceLock::new();
static PENDING_NEW_SESSIONS: OnceLock<std::sync::Mutex<HashMap<AgentId, PendingRemoteNewSession>>> =
    OnceLock::new();
static NEXT_BRIDGE_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingRemoteNewSession {
    pub(crate) source_binding_generation: u64,
    pub(crate) source_gateway_generation: u64,
    pub(crate) source_client_generation: u64,
    pub(crate) source_session_id: String,
    pub(crate) command_id: String,
}

fn bridges() -> &'static Mutex<HashMap<u64, ActiveBridge>> {
    BRIDGES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn command_bus() -> &'static CommandBus {
    COMMAND_BUS.get_or_init(|| {
        let (tx, rx) = mpsc::channel(COMMAND_CAPACITY);
        CommandBus {
            tx,
            rx: Mutex::new(rx),
        }
    })
}

fn pending_new_sessions() -> &'static std::sync::Mutex<HashMap<AgentId, PendingRemoteNewSession>> {
    PENDING_NEW_SESSIONS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

pub(crate) fn take_pending_new_session(agent_id: AgentId) -> Option<PendingRemoteNewSession> {
    pending_new_sessions()
        .lock()
        .expect("remote new-session pending lock poisoned")
        .remove(&agent_id)
}

/// Serialize the complete pager+gateway lifecycle transaction. `/rc` effects
/// cross several awaits, so neither component's internal lock can prevent a
/// stale Start or Stop task from tearing down a newer pairing on its own.
pub(crate) async fn lock_lifecycle() -> tokio::sync::MutexGuard<'static, ()> {
    LIFECYCLE.get_or_init(|| Mutex::new(())).lock().await
}

fn next_bridge_generation() -> u64 {
    NEXT_BRIDGE_GENERATION.fetch_add(1, Ordering::Relaxed)
}

fn transport(active: &ActiveBridge) -> RemoteTransport {
    RemoteTransport {
        binding_generation: active.target.bridge_generation,
        commands: command_bus().tx.clone(),
        events: active.events.clone(),
        snapshots: active.snapshot_tx.subscribe(),
    }
}

/// Arm the exact pager-owned session selected by `/rc`.
///
/// Re-arming the same exact target preserves its transport and pairing. A
/// duplicate canonical session ID owned by another AgentId or binding epoch is
/// a separate remote and never replaces this entry.
pub async fn arm(
    agent_id: AgentId,
    session_id: acp::SessionId,
    session_binding_epoch: u32,
) -> RemoteTransport {
    let mut guard = bridges().lock().await;
    if let Some(active) = guard.values_mut().find(|active| {
        active.target.agent_id == agent_id
            && active.target.session_id == session_id
            && active.target.session_binding_epoch == session_binding_epoch
    }) {
        if active.suspended {
            active.suspended = false;
            active.interaction_ids.clear();
            active.last_session = None;
            active.revision = 0;
            active.snapshot_tx.send_replace(None);
        }
        return transport(active);
    }

    let (events, _) = broadcast::channel(EVENT_CAPACITY);
    let (snapshot_tx, snapshot_rx) = watch::channel(None);
    let binding_generation = next_bridge_generation();
    guard.insert(
        binding_generation,
        ActiveBridge {
            target: RemoteTarget {
                agent_id,
                session_id,
                session_binding_epoch,
                bridge_generation: binding_generation,
                gateway_generation: None,
            },
            events: events.clone(),
            snapshot_tx,
            revision: 0,
            last_session: None,
            interaction_ids: HashMap::new(),
            suspended: false,
        },
    );
    RemoteTransport {
        binding_generation,
        commands: command_bus().tx.clone(),
        events,
        snapshots: snapshot_rx,
    }
}

/// Bind the transport to the process-local gateway generation returned by the
/// shell. Commands cannot execute before this exact value is installed.
pub async fn bind_gateway_generation(
    agent_id: AgentId,
    session_id: &acp::SessionId,
    session_binding_epoch: u32,
    gateway_generation: u64,
) -> bool {
    let mut guard = bridges().lock().await;
    let Some(active) = guard.values_mut().find(|active| {
        active.target.agent_id == agent_id
            && &active.target.session_id == session_id
            && active.target.session_binding_epoch == session_binding_epoch
    }) else {
        return false;
    };
    active.target.gateway_generation = Some(gateway_generation);
    active.suspended = false;
    true
}

/// Seed a just-created binding before its gateway URL can be handed to a
/// client. This avoids exposing a pairing that would answer hello with
/// `snapshotUnavailable` until the next pager-loop projection.
pub(crate) async fn seed_initial_snapshot(
    binding_generation: u64,
    session_id: &acp::SessionId,
    session: serde_json::Value,
) -> bool {
    let mut guard = bridges().lock().await;
    let Some(active) = guard.get_mut(&binding_generation) else {
        return false;
    };
    if active.suspended || &active.target.session_id != session_id {
        return false;
    }
    active.revision = 1;
    active.last_session = Some(session.clone());
    active.snapshot_tx.send_replace(Some(RemoteSnapshot {
        session_id: session_id.0.to_string(),
        revision: 1,
        session,
    }));
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RemoteBinding {
    pub(crate) binding_generation: u64,
    pub(crate) gateway_generation: Option<u64>,
}

pub(crate) async fn binding_for_target(
    agent_id: AgentId,
    session_id: &acp::SessionId,
    session_binding_epoch: u32,
) -> Option<RemoteBinding> {
    bridges()
        .lock()
        .await
        .values()
        .find(|active| {
            active.target.agent_id == agent_id
                && &active.target.session_id == session_id
                && active.target.session_binding_epoch == session_binding_epoch
        })
        .map(|active| RemoteBinding {
            binding_generation: active.target.bridge_generation,
            gateway_generation: active.target.gateway_generation,
        })
}

/// Revalidate every transport component captured by a remote usage request.
/// The async fetch checks this immediately before publishing its task result;
/// the AgentView cache performs the same exact identity comparison on apply.
pub(crate) async fn usage_request_is_current(identity: &RemoteUsageRequestIdentity) -> bool {
    bridges()
        .lock()
        .await
        .get(&identity.bridge_generation)
        .is_some_and(|active| {
            !active.suspended
                && active.target.agent_id == identity.agent_id
                && active.target.session_id == identity.session_id
                && active.target.session_binding_epoch == identity.session_binding_epoch
                && active.target.gateway_generation == Some(identity.gateway_generation)
        })
}

/// Suspend one exact target before stopping its gateway. Keeping the inert
/// bridge entry lets a later `/rc stop` retry cleanup of that same Serve path.
pub(crate) async fn suspend_target(
    agent_id: AgentId,
    session_id: &acp::SessionId,
    session_binding_epoch: u32,
    reason: RemoteRevocationReason,
) -> Option<RemoteBinding> {
    let mut guard = bridges().lock().await;
    let active = guard.values_mut().find(|active| {
        active.target.agent_id == agent_id
            && &active.target.session_id == session_id
            && active.target.session_binding_epoch == session_binding_epoch
    })?;
    let binding = RemoteBinding {
        binding_generation: active.target.bridge_generation,
        gateway_generation: active.target.gateway_generation.take(),
    };
    active.suspended = true;
    active.interaction_ids.clear();
    let _ = active.events.send(RemotePagerEvent::Revoked {
        session_id: active.target.session_id.0.to_string(),
        reason,
    });
    Some(binding)
}

pub(crate) async fn forget_binding(binding_generation: u64) -> bool {
    bridges().lock().await.remove(&binding_generation).is_some()
}

/// Remove only the exact binding captured by a stale async task.
#[cfg(test)]
pub(crate) async fn revoke_if_binding(
    binding_generation: u64,
    gateway_generation: Option<u64>,
    reason: RemoteRevocationReason,
) -> bool {
    let mut guard = bridges().lock().await;
    let matches = guard.get(&binding_generation).is_some_and(|active| {
        gateway_generation
            .is_none_or(|generation| active.target.gateway_generation == Some(generation))
    });
    if !matches {
        return false;
    }
    let active = guard
        .remove(&binding_generation)
        .expect("binding checked above");
    let _ = active.events.send(RemotePagerEvent::Revoked {
        session_id: active.target.session_id.0.to_string(),
        reason,
    });
    true
}

async fn revoke_all(reason: RemoteRevocationReason) {
    let active = bridges()
        .lock()
        .await
        .drain()
        .map(|(_, active)| active)
        .collect::<Vec<_>>();
    for active in active {
        let _ = active.events.send(RemotePagerEvent::Revoked {
            session_id: active.target.session_id.0.to_string(),
            reason,
        });
    }
}

/// App teardown is deliberately the only global shutdown operation.
pub async fn shutdown_all(reason: RemoteRevocationReason) -> Result<bool, String> {
    let _lifecycle = lock_lifecycle().await;
    revoke_all(reason).await;
    xai_grok_shell::remote_control::stop_all_gateways_checked(reason).await
}

#[derive(Debug)]
pub(crate) struct RemoteInbound {
    bridge_generation: u64,
    request: RemoteCommandRequest,
}

/// A wakeable, bounded FIFO shared by every live pairing. Gateway generations
/// disambiguate duplicate canonical session IDs before canonical dispatch.
pub(crate) async fn next_request() -> RemoteInbound {
    loop {
        let request = command_bus().rx.lock().await.recv().await;
        let Some(request) = request else {
            continue;
        };
        let guard = bridges().lock().await;
        let bridge_generation = guard
            .values()
            .find(|active| {
                !active.suspended
                    && active.target.session_id.0.as_ref() == request.session_id
                    && active.target.gateway_generation == Some(request.gateway_generation)
            })
            .map(|active| active.target.bridge_generation)
            .or_else(|| {
                let mut same_session = guard.values().filter(|active| {
                    !active.suspended && active.target.session_id.0.as_ref() == request.session_id
                });
                let only = same_session.next()?;
                same_session
                    .next()
                    .is_none()
                    .then_some(only.target.bridge_generation)
            })
            .unwrap_or_default();
        drop(guard);
        return RemoteInbound {
            bridge_generation,
            request,
        };
    }
}

pub(crate) struct RemoteExecution {
    bridge_generation: u64,
    session_id: String,
    client_generation: u64,
    command_id: String,
    outcome: RemoteCommandOutcome,
    pub(crate) effects: Vec<Effect>,
    force_snapshot: bool,
    defer_result: bool,
}

fn command_error(code: &str, message: impl Into<String>, retryable: bool) -> RemoteCommandOutcome {
    RemoteCommandOutcome::Error {
        error: RemoteError::new(code, message, retryable),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueuedPromptControl {
    Edit,
    Steer,
    Cancel,
}

fn validate_shared_queue_item(
    app: &AppView,
    agent_id: AgentId,
    queue_item_id: &str,
    expected_version: u64,
    control: QueuedPromptControl,
) -> Result<(), (&'static str, &'static str, bool)> {
    let Some(agent) = app.agents.get(&agent_id) else {
        return Err((
            "session_closed",
            "The armed Forge session is no longer available.",
            false,
        ));
    };
    let Some(item) = agent
        .shared_queue
        .iter()
        .find(|item| item.id == queue_item_id && item.version == expected_version)
    else {
        return Err((
            "queue_item_stale",
            "This queued message changed or is no longer queued. Refresh and try again.",
            true,
        ));
    };
    if control == QueuedPromptControl::Edit && item.kind != "prompt" {
        return Err((
            "queue_action_unavailable",
            "Only plain queued prompts can be edited remotely.",
            false,
        ));
    }
    if control == QueuedPromptControl::Steer && !agent.session.state.is_turn_running() {
        return Err((
            "queue_action_unavailable",
            "There is no running turn to steer.",
            true,
        ));
    }
    Ok(())
}

fn remote_queue_control_effects(
    app: &AppView,
    target: &RemoteTarget,
    command: RemoteCommand,
) -> Result<Vec<Effect>, (&'static str, &'static str, bool)> {
    match command {
        RemoteCommand::EditQueuedPrompt {
            queue_item_id,
            expected_version,
            text,
        } => {
            if text.trim().is_empty() {
                return Err((
                    "invalid_queue_edit",
                    "A queued message cannot be empty.",
                    false,
                ));
            }
            validate_shared_queue_item(
                app,
                target.agent_id,
                &queue_item_id,
                expected_version,
                QueuedPromptControl::Edit,
            )?;
            Ok(vec![Effect::QueueEdit {
                session_id: target.session_id.clone(),
                id: queue_item_id,
                new_text: text,
                expected_version: Some(expected_version),
            }])
        }
        RemoteCommand::SteerQueuedPrompt {
            queue_item_id,
            expected_version,
        } => {
            validate_shared_queue_item(
                app,
                target.agent_id,
                &queue_item_id,
                expected_version,
                QueuedPromptControl::Steer,
            )?;
            Ok(vec![Effect::QueueInterject {
                session_id: target.session_id.clone(),
                id: queue_item_id,
                expected_version,
                new_text: None,
            }])
        }
        RemoteCommand::CancelQueuedPrompt {
            queue_item_id,
            expected_version,
        } => {
            validate_shared_queue_item(
                app,
                target.agent_id,
                &queue_item_id,
                expected_version,
                QueuedPromptControl::Cancel,
            )?;
            Ok(vec![Effect::QueueRemove {
                session_id: target.session_id.clone(),
                id: queue_item_id,
                expected_version,
            }])
        }
        _ => unreachable!("remote queue helper only accepts queue commands"),
    }
}

fn source_has_pending_new_session(binding_generation: u64, gateway_generation: u64) -> bool {
    pending_new_sessions()
        .lock()
        .expect("remote new-session pending lock poisoned")
        .values()
        .any(|pending| {
            pending.source_binding_generation == binding_generation
                && pending.source_gateway_generation == gateway_generation
        })
}

pub(crate) fn register_pending_new_session(agent_id: AgentId, pending: PendingRemoteNewSession) {
    let mut guard = pending_new_sessions()
        .lock()
        .expect("remote new-session pending lock poisoned");
    guard.insert(agent_id, pending);
}

pub(crate) async fn complete_new_session_handoff_success(
    pending: &PendingRemoteNewSession,
    new_session_id: String,
    pairing_url: String,
    expires_at: String,
) -> bool {
    let guard = bridges().lock().await;
    let Some(source) = guard.get(&pending.source_binding_generation) else {
        return false;
    };
    if source.suspended
        || source.target.session_id.0.as_ref() != pending.source_session_id
        || source.target.gateway_generation != Some(pending.source_gateway_generation)
    {
        return false;
    }
    let (delivery_ack, mut delivery_result) = mpsc::unbounded_channel();
    let published = source
        .events
        .send(RemotePagerEvent::SessionCreated {
            session_id: pending.source_session_id.clone(),
            client_generation: pending.source_client_generation,
            command_id: pending.command_id.clone(),
            new_session_id,
            pairing_url,
            expires_at,
            delivery_ack,
        })
        .is_ok();
    drop(guard);
    if !published {
        return false;
    }
    let Ok(Some(RemoteSessionAcceptance::Begin { granted })) =
        tokio::time::timeout(SESSION_ACCEPTANCE_TIMEOUT, delivery_result.recv()).await
    else {
        return false;
    };
    if granted.send(()).is_err() {
        return false;
    }
    matches!(
        tokio::time::timeout(SESSION_ACCEPTANCE_COMMIT_TIMEOUT, delivery_result.recv()).await,
        Ok(Some(RemoteSessionAcceptance::Commit))
    )
}

pub(crate) async fn complete_new_session_handoff_failure(
    pending: &PendingRemoteNewSession,
    code: &str,
    message: impl Into<String>,
    retryable: bool,
) -> bool {
    let guard = bridges().lock().await;
    let Some(source) = guard.get(&pending.source_binding_generation) else {
        return false;
    };
    if source.suspended
        || source.target.session_id.0.as_ref() != pending.source_session_id
        || source.target.gateway_generation != Some(pending.source_gateway_generation)
    {
        return false;
    }
    source
        .events
        .send(RemotePagerEvent::CommandResult {
            session_id: pending.source_session_id.clone(),
            client_generation: pending.source_client_generation,
            command_id: pending.command_id.clone(),
            outcome: command_error(code, message, retryable),
        })
        .is_ok()
}

/// Revoke an armed child pairing that could not be delivered to its source
/// socket. Generation checks ensure a stale cleanup can never stop a newer
/// pairing for the child.
pub(crate) async fn revoke_undelivered_new_session(
    agent_id: AgentId,
    session_id: &acp::SessionId,
    session_binding_epoch: u32,
    binding_generation: u64,
    gateway_generation: u64,
) -> bool {
    suspend_target(
        agent_id,
        session_id,
        session_binding_epoch,
        RemoteRevocationReason::Stopped,
    )
    .await;
    match xai_grok_shell::remote_control::stop_gateway_generation_checked(
        binding_generation,
        gateway_generation,
        RemoteRevocationReason::Stopped,
    )
    .await
    {
        Ok(true) => forget_binding(binding_generation).await,
        Ok(false) | Err(_) => false,
    }
}

/// Retain the freshly armed child only after the exact requesting client has
/// persisted and validated it, then completed the bounded Begin/grant/Commit
/// handshake. Any missing acceptance, failed result write, or vanished socket
/// makes the fresh bearer unreachable, so revoke it immediately.
pub(crate) async fn finalize_new_session_handoff(
    pending: &PendingRemoteNewSession,
    agent_id: AgentId,
    session_id: &acp::SessionId,
    session_binding_epoch: u32,
    binding_generation: u64,
    gateway_generation: u64,
    pairing_url: String,
    expires_at: String,
) -> bool {
    let delivered = complete_new_session_handoff_success(
        pending,
        session_id.0.to_string(),
        pairing_url,
        expires_at,
    )
    .await;
    if !delivered {
        revoke_undelivered_new_session(
            agent_id,
            session_id,
            session_binding_epoch,
            binding_generation,
            gateway_generation,
        )
        .await;
        complete_new_session_handoff_failure(
            pending,
            "new_session_acceptance_failed",
            "The new session was not accepted by the requesting client in time.",
            true,
        )
        .await;
    }
    delivered
}

impl RemoteExecution {
    fn error(
        inbound: RemoteInbound,
        code: &str,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            bridge_generation: inbound.bridge_generation,
            session_id: inbound.request.session_id,
            client_generation: inbound.request.client_generation,
            command_id: inbound.request.command_id,
            outcome: command_error(code, message, retryable),
            effects: Vec::new(),
            force_snapshot: false,
            defer_result: false,
        }
    }
}

/// Validate and apply one gateway-bound command through canonical pager state.
pub(crate) async fn execute_request(app: &mut AppView, inbound: RemoteInbound) -> RemoteExecution {
    let (target, interaction_key) = {
        let guard = bridges().lock().await;
        let Some(active) = guard.get(&inbound.bridge_generation) else {
            return RemoteExecution::error(
                inbound,
                "remote_not_armed",
                "Forge Remote is no longer armed.",
                false,
            );
        };
        if active.suspended
            || active.target.session_id.0.as_ref() != inbound.request.session_id
            || active.target.gateway_generation != Some(inbound.request.gateway_generation)
        {
            return RemoteExecution::error(
                inbound,
                "stale_binding",
                "This command belongs to an expired Forge Remote pairing.",
                false,
            );
        }
        let interaction_key = match &inbound.request.command {
            RemoteCommand::ResolveInteraction { interaction_id, .. } => {
                interaction_key_for_browser_id(&active.interaction_ids, interaction_id)
                    .map(str::to_owned)
            }
            _ => None,
        };
        (active.target.clone(), interaction_key)
    };

    let valid_target = app.agents.get(&target.agent_id).is_some_and(|agent| {
        agent.session.session_id.as_ref() == Some(&target.session_id)
            && agent.session_binding_epoch == target.session_binding_epoch
    });
    if !valid_target {
        return RemoteExecution::error(
            inbound,
            "session_closed",
            "The armed Forge session is no longer available.",
            false,
        );
    }

    if app.reconnect_pending
        && matches!(
            &inbound.request.command,
            RemoteCommand::Prompt { .. }
                | RemoteCommand::NewSession { .. }
                | RemoteCommand::EditQueuedPrompt { .. }
                | RemoteCommand::SteerQueuedPrompt { .. }
                | RemoteCommand::CancelQueuedPrompt { .. }
        )
    {
        return RemoteExecution::error(
            inbound,
            "session_reconnecting",
            "Forge is reconnecting this session. Retry the prompt in a moment.",
            true,
        );
    }

    let session_id = inbound.request.session_id.clone();
    let client_generation = inbound.request.client_generation;
    let command_id = inbound.request.command_id.clone();
    let gateway_generation = inbound.request.gateway_generation;
    let mut effects = Vec::new();
    let mut force_snapshot = false;
    let mut rejection_code = "command_rejected";
    let mut rejection_retryable = false;
    let result = match inbound.request.command {
        RemoteCommand::Prompt { text, images } => {
            if text.trim().is_empty() && images.is_empty() {
                Err("A prompt cannot be empty.".into())
            } else {
                effects = crate::app::dispatch::dispatch_remote_prompt(
                    app,
                    target.agent_id,
                    text,
                    images,
                );
                Ok(())
            }
        }
        RemoteCommand::PhoneReady => {
            if let Some(agent) = app.agents.get_mut(&target.agent_id)
                && matches!(
                    agent.active_modal,
                    Some(crate::views::modal::ActiveModal::ForgeRemote { .. })
                )
            {
                agent.active_modal = None;
            }
            Ok(())
        }
        RemoteCommand::NewSession { .. } => {
            if source_has_pending_new_session(inbound.bridge_generation, gateway_generation) {
                rejection_code = "new_session_pending";
                rejection_retryable = true;
                Err("A new session is already being created from this remote session.".into())
            } else {
                match crate::app::dispatch::dispatch_remote_new_session(app, target.agent_id) {
                    Ok((new_agent_id, resolved)) => {
                        let pending = PendingRemoteNewSession {
                            source_binding_generation: inbound.bridge_generation,
                            source_gateway_generation: gateway_generation,
                            source_client_generation: inbound.request.client_generation,
                            source_session_id: session_id.clone(),
                            command_id: command_id.clone(),
                        };
                        register_pending_new_session(new_agent_id, pending);
                        effects = resolved;
                        // ACP creation and fresh route activation are asynchronous. The
                        // source socket receives a single terminal result after both.
                        return RemoteExecution {
                            bridge_generation: inbound.bridge_generation,
                            session_id,
                            client_generation,
                            command_id,
                            outcome: RemoteCommandOutcome::Ok,
                            effects,
                            force_snapshot: false,
                            defer_result: true,
                        };
                    }
                    Err(message) => Err(message),
                }
            }
        }
        RemoteCommand::AcceptNewSession { .. } => {
            rejection_code = "gateway_only_command";
            Err("Session acceptance must be handled by the requesting gateway socket.".into())
        }
        command @ (RemoteCommand::EditQueuedPrompt { .. }
        | RemoteCommand::SteerQueuedPrompt { .. }
        | RemoteCommand::CancelQueuedPrompt { .. }) => {
            match remote_queue_control_effects(app, &target, command) {
                Ok(resolved) => {
                    effects = resolved;
                    Ok(())
                }
                Err((code, message, retryable)) => {
                    rejection_code = code;
                    rejection_retryable = retryable;
                    Err(message.into())
                }
            }
        }
        RemoteCommand::Cancel => crate::app::dispatch::dispatch_remote_cancel(app, target.agent_id)
            .map(|resolved| {
                effects = resolved;
            })
            .map_err(|message| {
                rejection_code = "interaction_required";
                message
            }),
        RemoteCommand::SetModel {
            model_id,
            reasoning_effort,
        } => {
            let agent = app
                .agents
                .get(&target.agent_id)
                .expect("remote target was validated above");
            if agent.session.models.fast_mode_pending() {
                rejection_code = "fast_mode_pending";
                rejection_retryable = true;
                Err("Wait for the Fast Mode change to finish, then retry the model switch.".into())
            } else {
                crate::app::dispatch::dispatch_remote_switch_model(
                    app,
                    target.agent_id,
                    &model_id,
                    reasoning_effort.as_deref(),
                )
                .map(|resolved| effects = resolved)
            }
        }
        RemoteCommand::SetFastMode { enabled } => {
            let agent = app
                .agents
                .get(&target.agent_id)
                .expect("remote target was validated above");
            if agent.session.model_switch_pending {
                rejection_code = "model_switch_pending";
                rejection_retryable = true;
                Err("Wait for the current model switch to finish, then retry Fast Mode.".into())
            } else if !crate::forge::fast_mode::is_supported(&agent.session.models) {
                rejection_code = "fast_mode_unsupported";
                Err("The current model does not support Fast Mode.".into())
            } else if agent.session.models.fast_mode_pending() {
                rejection_code = "fast_mode_pending";
                rejection_retryable = true;
                Err("A Fast Mode change is already in progress.".into())
            } else {
                crate::forge::fast_mode::dispatch_set_fast_mode_for_agent(
                    app,
                    target.agent_id,
                    enabled,
                )
                .map(|resolved| effects = resolved)
            }
        }
        RemoteCommand::Btw { question } => {
            if question.trim().is_empty() {
                Err("A BTW question cannot be empty.".into())
            } else {
                crate::app::dispatch::dispatch_remote_btw(app, target.agent_id, question)
                    .map(|resolved| effects = resolved)
                    .map_err(|message| {
                        rejection_code = "btw_in_progress";
                        message
                    })
            }
        }
        RemoteCommand::ResolveInteraction {
            interaction_id: _,
            response,
        } => resolve_interaction(app, target.agent_id, interaction_key.as_deref(), response)
            .map(|resolved| effects = resolved),
        RemoteCommand::RefreshUsage => {
            let agent = app
                .agents
                .get_mut(&target.agent_id)
                .expect("remote target was validated above");
            let provider = agent.session.models.current_provider_id();
            let refresh_generation = agent.remote_usage.next_refresh_generation();
            let identity = RemoteUsageRequestIdentity {
                agent_id: target.agent_id,
                session_id: target.session_id.clone(),
                session_binding_epoch: target.session_binding_epoch,
                bridge_generation: inbound.bridge_generation,
                gateway_generation,
                request_id: command_id.clone(),
                refresh_generation,
                provider,
            };
            let immediate_context = agent.context_state.clone();
            agent
                .remote_usage
                .begin_refresh(identity.clone(), immediate_context.as_ref());
            effects.push(Effect::FetchRemoteUsage { identity });
            Ok(())
        }
        RemoteCommand::Resync => {
            force_snapshot = true;
            Ok(())
        }
        RemoteCommand::Ping => Ok(()),
    };

    RemoteExecution {
        bridge_generation: inbound.bridge_generation,
        session_id,
        client_generation,
        command_id,
        outcome: match result {
            Ok(()) => RemoteCommandOutcome::Ok,
            Err(message) => command_error(rejection_code, message, rejection_retryable),
        },
        effects,
        force_snapshot,
        defer_result: false,
    }
}

fn resolve_interaction(
    app: &mut AppView,
    agent_id: AgentId,
    interaction_key: Option<&str>,
    response: RemoteInteractionResponse,
) -> Result<Vec<Effect>, String> {
    let Some(interaction_key) = interaction_key else {
        return Err("This interaction was already resolved.".into());
    };
    let Some((kind, tool_call_id)) = interaction_key.split_once(':') else {
        return Err("This interaction is no longer available.".into());
    };
    match (kind, response) {
        ("permission", RemoteInteractionResponse::Permission { option_id }) => {
            crate::app::dispatch::dispatch_remote_permission_select(
                app,
                agent_id,
                tool_call_id,
                acp::PermissionOptionId::new(option_id),
            )
        }
        ("permission", RemoteInteractionResponse::PermissionFollowup { text }) => {
            crate::app::dispatch::dispatch_remote_permission_followup(
                app,
                agent_id,
                tool_call_id,
                text,
            )
        }
        ("permission", RemoteInteractionResponse::Cancel) => {
            crate::app::dispatch::dispatch_remote_permission_cancel(app, agent_id, tool_call_id)
        }
        ("question", RemoteInteractionResponse::Question { answers }) => {
            let answers = answers
                .into_iter()
                .map(|answer| {
                    (
                        answer.question_index,
                        answer.option_indices,
                        answer.freeform,
                    )
                })
                .collect::<Vec<_>>();
            let Some(agent) = app.agents.get_mut(&agent_id) else {
                return Err("The armed Forge session is no longer available.".into());
            };
            agent.resolve_remote_question(tool_call_id, &answers, false)?;
            Ok(Vec::new())
        }
        ("question", RemoteInteractionResponse::Cancel) => {
            let Some(agent) = app.agents.get_mut(&agent_id) else {
                return Err("The armed Forge session is no longer available.".into());
            };
            agent.resolve_remote_question(tool_call_id, &[], true)?;
            Ok(Vec::new())
        }
        ("plan", RemoteInteractionResponse::Plan { outcome, feedback }) => {
            let outcome = match outcome {
                RemotePlanOutcome::Approved => "approved",
                RemotePlanOutcome::Cancelled => "cancelled",
                RemotePlanOutcome::Abandoned => "abandoned",
            };
            let interjection = {
                let Some(agent) = app.agents.get_mut(&agent_id) else {
                    return Err("The armed Forge session is no longer available.".into());
                };
                agent.resolve_remote_plan(tool_call_id, outcome, feedback)?
            };
            Ok(interjection
                .map(|text| crate::app::dispatch::dispatch_remote_interject(app, agent_id, text))
                .unwrap_or_default())
        }
        ("plan", RemoteInteractionResponse::Cancel) => {
            let Some(agent) = app.agents.get_mut(&agent_id) else {
                return Err("The armed Forge session is no longer available.".into());
            };
            agent.resolve_remote_plan(tool_call_id, "cancelled", None)?;
            Ok(Vec::new())
        }
        _ => Err("That response does not match this interaction.".into()),
    }
}

/// Publish the authoritative target state. The first projection seeds the
/// reconnectable watch snapshot; later changes are monotonic, gap-safe deltas.
/// Transcript-only streaming changes use compact tail splices, while every
/// other change falls back to a full state replacement.
pub(crate) async fn publish_state(app: &AppView) {
    let mut guard = bridges().lock().await;
    let binding_generations = guard.keys().copied().collect::<Vec<_>>();
    let mut closed_gateways = Vec::new();
    for binding_generation in binding_generations {
        let valid = guard.get(&binding_generation).is_some_and(|active| {
            app.agents
                .get(&active.target.agent_id)
                .is_some_and(|agent| {
                    agent.session.session_id.as_ref() == Some(&active.target.session_id)
                        && agent.session_binding_epoch == active.target.session_binding_epoch
                })
        });
        if !valid {
            let Some(active) = guard.remove(&binding_generation) else {
                continue;
            };
            let _ = active.events.send(RemotePagerEvent::Revoked {
                session_id: active.target.session_id.0.to_string(),
                reason: RemoteRevocationReason::SessionClosed,
            });
            if let Some(gateway_generation) = active.target.gateway_generation {
                closed_gateways.push((binding_generation, gateway_generation));
            }
            continue;
        }
        if let Some(active) = guard.get_mut(&binding_generation)
            && !active.suspended
        {
            publish_bridge_state(app, active);
        }
    }
    drop(guard);
    for (binding_generation, gateway_generation) in closed_gateways {
        let _ = xai_grok_shell::remote_control::stop_gateway_generation_checked(
            binding_generation,
            gateway_generation,
            RemoteRevocationReason::SessionClosed,
        )
        .await;
    }
}

fn publish_bridge_state(app: &AppView, active: &mut ActiveBridge) {
    let agent = app
        .agents
        .get(&active.target.agent_id)
        .expect("target was validated above");
    let Some(snapshot) = project_session(agent, &mut active.interaction_ids) else {
        return;
    };
    let Ok(session) = serde_json::to_value(snapshot) else {
        let _ = active.events.send(RemotePagerEvent::Error {
            session_id: active.target.session_id.0.to_string(),
            error: RemoteError::new(
                "snapshot_failed",
                "Forge Remote could not project the current session.",
                true,
            ),
        });
        return;
    };
    if active.last_session.as_ref() == Some(&session) {
        return;
    }

    let base_revision = active.revision;
    active.revision = active.revision.saturating_add(1).max(1);
    let session_id = active.target.session_id.0.to_string();
    let delta_event = active
        .last_session
        .as_ref()
        .and_then(|previous| transcript_splice_event(previous, &session, &session_id))
        .unwrap_or_else(|| {
            serde_json::json!({
                "kind": "stateReplaced",
                "session": session.clone(),
            })
        });
    active.last_session = Some(session.clone());
    active.snapshot_tx.send_replace(Some(RemoteSnapshot {
        session_id: session_id.clone(),
        revision: active.revision,
        session: session.clone(),
    }));
    if base_revision > 0 {
        let _ = active.events.send(RemotePagerEvent::Delta {
            session_id,
            base_revision,
            revision: active.revision,
            event: delta_event,
        });
    }
}

/// Build a compact live transcript update when every non-transcript field is
/// unchanged. Reconnects still read the complete authoritative watch snapshot;
/// this event only avoids retransmitting and replacing that full snapshot for
/// each streamed assistant, reasoning, or tool chunk.
fn transcript_splice_event(
    previous: &serde_json::Value,
    next: &serde_json::Value,
    session_id: &str,
) -> Option<serde_json::Value> {
    let previous_object = previous.as_object()?;
    let next_object = next.as_object()?;
    let previous_transcript = previous_object.get("transcript")?.as_array()?;
    let next_transcript = next_object.get("transcript")?.as_array()?;

    let same_non_transcript_fields = previous_object
        .iter()
        .filter(|(key, _)| key.as_str() != "transcript")
        .all(|(key, value)| next_object.get(key) == Some(value))
        && next_object
            .iter()
            .filter(|(key, _)| key.as_str() != "transcript")
            .all(|(key, value)| previous_object.get(key) == Some(value));
    if !same_non_transcript_fields {
        return None;
    }

    let start = previous_transcript
        .iter()
        .zip(next_transcript)
        .take_while(|(previous_item, next_item)| previous_item == next_item)
        .count();
    Some(serde_json::json!({
        "kind": "transcriptSpliced",
        "sessionId": session_id,
        "start": start,
        "deleteCount": previous_transcript.len().saturating_sub(start),
        "items": next_transcript[start..].to_vec(),
    }))
}

/// Send the command acknowledgement after its canonical effects have been
/// scheduled. The event loop projects state immediately afterward; this order
/// ensures a session-closed rejection is not lost when projection revokes an
/// invalid target.
pub(crate) async fn finish_execution(execution: RemoteExecution) {
    if execution.defer_result {
        return;
    }
    let mut guard = bridges().lock().await;
    let Some(active) = guard.get_mut(&execution.bridge_generation) else {
        return;
    };
    if active.suspended || active.target.session_id.0.as_ref() != execution.session_id {
        return;
    }
    if execution.force_snapshot
        && let Some(session) = active.last_session.clone()
    {
        active.snapshot_tx.send_replace(Some(RemoteSnapshot {
            session_id: execution.session_id.clone(),
            revision: active.revision,
            session,
        }));
    }
    let _ = active.events.send(RemotePagerEvent::CommandResult {
        session_id: execution.session_id,
        client_generation: execution.client_generation,
        command_id: execution.command_id,
        outcome: execution.outcome,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bridge_test_lock() -> &'static Mutex<()> {
        static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_LOCK.get_or_init(|| Mutex::new(()))
    }

    async fn arm_test_app(
        app: &crate::app::app_view::AppView,
        gateway_generation: u64,
    ) -> RemoteTransport {
        revoke_all(RemoteRevocationReason::Stopped).await;
        let agent = app.agents.get(&AgentId(0)).unwrap();
        let session_id = agent.session.session_id.clone().unwrap();
        let transport = arm(AgentId(0), session_id.clone(), agent.session_binding_epoch).await;
        assert!(
            bind_gateway_generation(
                AgentId(0),
                &session_id,
                agent.session_binding_epoch,
                gateway_generation,
            )
            .await
        );
        transport
    }

    async fn receive_command(
        transport: &RemoteTransport,
        gateway_generation: u64,
        command_id: &str,
        command: RemoteCommand,
    ) -> RemoteInbound {
        transport
            .commands
            .send(RemoteCommandRequest {
                session_id: "test-session".into(),
                gateway_generation,
                client_generation: 1,
                command_id: command_id.into(),
                command,
            })
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), next_request())
            .await
            .expect("command must wake the pager")
    }

    fn outcome_error(execution: &RemoteExecution) -> &RemoteError {
        let RemoteCommandOutcome::Error { error } = &execution.outcome else {
            panic!("expected command error")
        };
        error
    }

    #[tokio::test]
    async fn remote_new_session_is_same_cwd_sibling_and_defers_for_fresh_pairing() {
        let _test = bridge_test_lock().lock().await;
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let source_id = AgentId(0);
        app.next_agent_id = 1;
        let source_session = app.agents[&source_id].session.session_id.clone().unwrap();
        let source_epoch = app.agents[&source_id].session_binding_epoch;
        let source_cwd = app.agents[&source_id].session.cwd.clone();
        let transport = arm_test_app(&app, 81).await;

        let execution = execute_request(
            &mut app,
            receive_command(&transport, 81, "new-1", RemoteCommand::NewSession {}).await,
        )
        .await;
        assert!(execution.defer_result);
        assert!(matches!(execution.outcome, RemoteCommandOutcome::Ok));
        let new_agent_id = match execution.effects.as_slice() {
            [Effect::CreateSession { agent_id, cwd, .. }] => {
                assert_eq!(cwd, &source_cwd);
                *agent_id
            }
            other => panic!("expected only canonical CreateSession, got {other:?}"),
        };
        assert_ne!(new_agent_id, source_id);
        assert!(app.agents.contains_key(&source_id));
        assert!(app.agents.contains_key(&new_agent_id));
        assert!(
            binding_for_target(source_id, &source_session, source_epoch)
                .await
                .is_some()
        );
        assert_eq!(
            app.agents[&source_id].session.session_id,
            Some(source_session.clone())
        );

        let duplicate = execute_request(
            &mut app,
            receive_command(&transport, 81, "new-2", RemoteCommand::NewSession {}).await,
        )
        .await;
        assert_eq!(outcome_error(&duplicate).code, "new_session_pending");
        assert!(outcome_error(&duplicate).retryable);

        let pending = take_pending_new_session(new_agent_id).unwrap();
        assert_eq!(pending.command_id, "new-1");
        assert_eq!(pending.source_session_id, source_session.0.as_ref());
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn undelivered_child_pairing_is_generation_checked_and_removed() {
        let _test = bridge_test_lock().lock().await;
        revoke_all(RemoteRevocationReason::Stopped).await;
        let app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = AgentId(0);
        let agent = &app.agents[&agent_id];
        let session_id = agent.session.session_id.clone().unwrap();
        let epoch = agent.session_binding_epoch;
        let transport = arm(agent_id, session_id.clone(), epoch).await;
        let arm =
            xai_grok_shell::remote_control::arm_active_gateway(session_id.0.to_string(), transport)
                .await
                .unwrap();
        assert!(
            bind_gateway_generation(agent_id, &session_id, epoch, arm.gateway_generation,).await
        );
        assert!(
            xai_grok_shell::remote_control::active_gateway_arm(arm.binding_generation)
                .await
                .is_some()
        );

        assert!(
            revoke_undelivered_new_session(
                agent_id,
                &session_id,
                epoch,
                arm.binding_generation,
                arm.gateway_generation,
            )
            .await
        );
        assert!(
            xai_grok_shell::remote_control::active_gateway_arm(arm.binding_generation)
                .await
                .is_none()
        );
        assert!(
            binding_for_target(agent_id, &session_id, epoch)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn failed_session_created_socket_write_revokes_the_fresh_child() {
        let _test = bridge_test_lock().lock().await;
        revoke_all(RemoteRevocationReason::Stopped).await;
        let app = crate::app::app_view::tests::test_app_with_agent();
        let child_id = AgentId(0);
        let child = &app.agents[&child_id];
        let child_session = child.session.session_id.clone().unwrap();
        let child_epoch = child.session_binding_epoch;

        let source_session = acp::SessionId::new("source-session");
        let source_transport = arm(AgentId(91), source_session.clone(), 1).await;
        assert!(bind_gateway_generation(AgentId(91), &source_session, 1, 401).await);
        let mut source_events = source_transport.events.subscribe();
        let pending = PendingRemoteNewSession {
            source_binding_generation: source_transport.binding_generation,
            source_gateway_generation: 401,
            source_client_generation: 17,
            source_session_id: source_session.0.to_string(),
            command_id: "new-write-fails".into(),
        };

        let child_transport = arm(child_id, child_session.clone(), child_epoch).await;
        let child_arm = xai_grok_shell::remote_control::arm_active_gateway(
            child_session.0.to_string(),
            child_transport,
        )
        .await
        .unwrap();
        assert!(
            bind_gateway_generation(
                child_id,
                &child_session,
                child_epoch,
                child_arm.gateway_generation,
            )
            .await
        );

        let pending_for_task = pending.clone();
        let child_session_for_task = child_session.clone();
        let binding_generation = child_arm.binding_generation;
        let gateway_generation = child_arm.gateway_generation;
        let finalize = tokio::spawn(async move {
            finalize_new_session_handoff(
                &pending_for_task,
                child_id,
                &child_session_for_task,
                child_epoch,
                binding_generation,
                gateway_generation,
                "https://device.tail.example/forge/fresh/".into(),
                "2030-01-02T03:04:05Z".into(),
            )
            .await
        });
        let event = tokio::time::timeout(Duration::from_secs(1), source_events.recv())
            .await
            .expect("source did not receive sessionCreated")
            .expect("source event channel closed");
        assert!(matches!(event, RemotePagerEvent::SessionCreated { .. }));
        // Keep the event (and therefore its acceptance sender) alive without
        // acknowledging it. This models a client that received the provisional
        // route but never completed secure persistence/validation.
        assert!(!finalize.await.unwrap());
        drop(event);
        assert!(
            xai_grok_shell::remote_control::active_gateway_arm(binding_generation)
                .await
                .is_none()
        );
        assert!(
            binding_for_target(child_id, &child_session, child_epoch)
                .await
                .is_none()
        );
        let terminal = tokio::time::timeout(Duration::from_secs(1), source_events.recv())
            .await
            .expect("acceptance timeout did not produce a terminal result")
            .expect("source event channel closed");
        assert!(matches!(
            terminal,
            RemotePagerEvent::CommandResult {
                client_generation: 17,
                command_id,
                outcome: RemoteCommandOutcome::Error { ref error },
                ..
            } if command_id == "new-write-fails"
                && error.code == "new_session_acceptance_failed"
        ));
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    fn attach_permission(
        app: &mut AppView,
        tool_call_id: &str,
        request_id: usize,
    ) -> tokio::sync::oneshot::Receiver<acp::Result<acp::RequestPermissionResponse>> {
        use crate::views::permission_view::{PermissionFocus, PermissionViewState};

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let options = vec![
            acp::PermissionOption::new(
                acp::PermissionOptionId::new("allow-once"),
                "Allow once",
                acp::PermissionOptionKind::AllowOnce,
            ),
            acp::PermissionOption::new(
                acp::PermissionOptionId::new("reject-once"),
                "Reject",
                acp::PermissionOptionKind::RejectOnce,
            ),
        ];
        let request = acp::RequestPermissionRequest::new(
            acp::SessionId::new("test-session"),
            acp::ToolCallUpdate::new(
                acp::ToolCallId::new(tool_call_id),
                acp::ToolCallUpdateFields::default(),
            ),
            options.clone(),
        );
        app.agents
            .get_mut(&AgentId(0))
            .unwrap()
            .permission_queue
            .push_back(PermissionViewState {
                request: xai_acp_lib::AcpArgs {
                    request,
                    response_tx,
                },
                id: request_id,
                focus: PermissionFocus::Options,
                options,
                active_idx: 0,
                bash_highlights: None,
                bash_selection_count: 0,
                bash_command_raw: None,
                mcp_scope: None,
                title: "Run the acceptance command?".into(),
                description: vec!["This is the live ACP permission sender.".into()],
                args_expanded: false,
                desc_scroll: 0,
                subagent_label: None,
                options_area_height: 0,
                options_scroll_offset: 0,
            });
        response_rx
    }

    fn attach_question(
        app: &mut AppView,
        tool_call_id: &str,
    ) -> tokio::sync::oneshot::Receiver<xai_acp_lib::AcpResult<acp::ExtResponse>> {
        use crate::views::prompt_widget::StashedPrompt;
        use crate::views::question_view::{AskUserQuestionMode, QuestionViewState};
        use xai_grok_tools::implementations::grok_build::ask_user_question::{
            Question, QuestionOption,
        };

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        app.agents.get_mut(&AgentId(0)).unwrap().question_view =
            Some(QuestionViewState::with_response_tx(
                tool_call_id.into(),
                vec![Question {
                    question: "Continue the acceptance story?".into(),
                    options: vec![QuestionOption {
                        label: "Continue".into(),
                        description: "Exercise the terminal-first race.".into(),
                        preview: None,
                        id: None,
                    }],
                    multi_select: Some(false),
                    id: None,
                }],
                StashedPrompt::default(),
                Some(response_tx),
                AskUserQuestionMode::Default,
            ));
        response_rx
    }

    fn attach_plan(
        app: &mut AppView,
        tool_call_id: &str,
    ) -> tokio::sync::oneshot::Receiver<xai_acp_lib::AcpResult<acp::ExtResponse>> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let request = crate::views::plan_approval_view::ExitPlanModeExtRequest {
            session_id: "test-session".into(),
            tool_call_id: tool_call_id.into(),
            plan_content: Some("# Acceptance plan\n\nExercise dropped receivers safely.".into()),
        };
        let prompt = app.agents.get_mut(&AgentId(0)).unwrap().prompt.stash();
        app.agents.get_mut(&AgentId(0)).unwrap().plan_approval_view = Some(
            crate::views::plan_approval_view::PlanApprovalViewState::new(
                request,
                prompt,
                response_tx,
            ),
        );
        response_rx
    }

    fn interaction_id(snapshot: &RemoteSnapshot, kind: &str) -> String {
        snapshot.session["activeInteractions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|interaction| interaction["kind"] == kind)
            .and_then(|interaction| interaction["interactionId"].as_str())
            .unwrap_or_else(|| panic!("missing {kind} interaction in {snapshot:?}"))
            .to_owned()
    }

    async fn next_delta(
        events: &mut broadcast::Receiver<RemotePagerEvent>,
    ) -> (u64, u64, serde_json::Value) {
        loop {
            let event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
                .await
                .expect("pager event timed out")
                .expect("pager event channel closed");
            if let RemotePagerEvent::Delta {
                base_revision,
                revision,
                event,
                ..
            } = event
            {
                return (base_revision, revision, event);
            }
        }
    }

    async fn next_revocation(
        events: &mut broadcast::Receiver<RemotePagerEvent>,
    ) -> RemoteRevocationReason {
        loop {
            let event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
                .await
                .expect("pager revocation timed out")
                .expect("pager event channel closed");
            if let RemotePagerEvent::Revoked { reason, .. } = event {
                return reason;
            }
        }
    }

    #[test]
    fn command_errors_are_typed_and_non_retryable() {
        let RemoteCommandOutcome::Error { error } = command_error("bad_command", "Nope", false)
        else {
            panic!("expected typed error")
        };
        assert_eq!(error.code, "bad_command");
        assert_eq!(error.message, "Nope");
        assert!(!error.retryable);
    }

    #[tokio::test]
    async fn shared_bounded_ingress_wakes_for_independent_exact_targets() {
        let _test = bridge_test_lock().lock().await;
        revoke_all(RemoteRevocationReason::Stopped).await;
        let session = acp::SessionId::new("bridge-test-session");
        let transport = arm(AgentId(10), session.clone(), 1).await;
        assert_eq!(transport.commands.max_capacity(), COMMAND_CAPACITY);
        assert!(bind_gateway_generation(AgentId(10), &session, 1, 4).await);
        let request = RemoteCommandRequest {
            session_id: session.0.to_string(),
            gateway_generation: 4,
            client_generation: 1,
            command_id: "command-1".into(),
            command: RemoteCommand::Ping,
        };
        transport.commands.send(request).await.unwrap();
        let inbound = tokio::time::timeout(std::time::Duration::from_secs(1), next_request())
            .await
            .expect("request must wake event loop");
        assert_eq!(inbound.request.command_id, "command-1");

        let independent = arm(AgentId(11), session.clone(), 2).await;
        assert_ne!(independent.binding_generation, transport.binding_generation);
        assert!(bind_gateway_generation(AgentId(11), &session, 2, 5).await);
        independent
            .commands
            .send(RemoteCommandRequest {
                session_id: session.0.to_string(),
                gateway_generation: 5,
                client_generation: 1,
                command_id: "command-2".into(),
                command: RemoteCommand::Ping,
            })
            .await
            .unwrap();
        let inbound = tokio::time::timeout(std::time::Duration::from_secs(1), next_request())
            .await
            .expect("independent request must wake event loop");
        assert_eq!(inbound.request.command_id, "command-2");
        assert_eq!(inbound.bridge_generation, independent.binding_generation);
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn binding_is_idempotent_only_for_the_exact_target() {
        let _test = bridge_test_lock().lock().await;
        revoke_all(RemoteRevocationReason::Stopped).await;
        let session = acp::SessionId::new("same-browser-session");
        let first = arm(AgentId(30), session.clone(), 7).await;
        let first_generation = first.binding_generation;

        let projected = serde_json::json!({"sessionId": "same-browser-session"});
        {
            let mut guard = bridges().lock().await;
            let active = guard.get_mut(&first_generation).unwrap();
            active.revision = 9;
            active.last_session = Some(projected.clone());
            active.snapshot_tx.send_replace(Some(RemoteSnapshot {
                session_id: session.0.to_string(),
                revision: 9,
                session: projected,
            }));
        }
        assert_eq!(first.snapshots.borrow().as_ref().unwrap().revision, 9);

        let identical = arm(AgentId(30), session.clone(), 7).await;
        assert_eq!(identical.binding_generation, first_generation);
        assert_eq!(identical.snapshots.borrow().as_ref().unwrap().revision, 9);

        let duplicate_id = arm(AgentId(31), session.clone(), 8).await;
        assert_ne!(duplicate_id.binding_generation, first_generation);
        assert!(duplicate_id.snapshots.borrow().is_none());
        assert_eq!(first.snapshots.borrow().as_ref().unwrap().revision, 9);
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn duplicate_session_ids_keep_commands_stop_and_close_exactly_isolated() {
        let _test = bridge_test_lock().lock().await;
        revoke_all(RemoteRevocationReason::Stopped).await;
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let duplicate_session = acp::SessionId::new("test-session");
        let mut duplicate = crate::app::agent_view::test_agent_view(
            Some("test-session"),
            std::path::PathBuf::from("/tmp/duplicate"),
        );
        duplicate.session.id = AgentId(1);
        app.agents.insert(AgentId(1), duplicate);

        let first_epoch = app.agents[&AgentId(0)].session_binding_epoch;
        let second_epoch = app.agents[&AgentId(1)].session_binding_epoch;
        let first = arm(AgentId(0), duplicate_session.clone(), first_epoch).await;
        let second = arm(AgentId(1), duplicate_session.clone(), second_epoch).await;
        assert_ne!(first.binding_generation, second.binding_generation);
        assert!(bind_gateway_generation(AgentId(0), &duplicate_session, first_epoch, 201).await);
        assert!(bind_gateway_generation(AgentId(1), &duplicate_session, second_epoch, 202).await);
        publish_state(&app).await;
        let mut first_events = first.events.subscribe();
        let mut second_events = second.events.subscribe();

        let stopped = suspend_target(
            AgentId(0),
            &duplicate_session,
            first_epoch,
            RemoteRevocationReason::Stopped,
        )
        .await
        .expect("first exact target must be armed");
        assert_eq!(stopped.binding_generation, first.binding_generation);
        assert_eq!(stopped.gateway_generation, Some(201));
        assert_eq!(
            next_revocation(&mut first_events).await,
            RemoteRevocationReason::Stopped
        );
        assert!(matches!(
            second_events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));

        second
            .commands
            .send(RemoteCommandRequest {
                session_id: duplicate_session.0.to_string(),
                gateway_generation: 202,
                client_generation: 1,
                command_id: "duplicate-second".into(),
                command: RemoteCommand::RefreshUsage,
            })
            .await
            .unwrap();
        let inbound = tokio::time::timeout(std::time::Duration::from_secs(1), next_request())
            .await
            .expect("second exact target must remain wakeable");
        assert_eq!(inbound.bridge_generation, second.binding_generation);
        let execution = execute_request(&mut app, inbound).await;
        assert!(matches!(execution.outcome, RemoteCommandOutcome::Ok));
        let [Effect::FetchRemoteUsage { identity }] = execution.effects.as_slice() else {
            panic!("refreshUsage must schedule one targeted usage fetch")
        };
        assert_eq!(identity.agent_id, AgentId(1));
        assert_eq!(identity.session_id, duplicate_session);
        assert_eq!(identity.session_binding_epoch, second_epoch);
        assert_eq!(identity.bridge_generation, second.binding_generation);
        assert_eq!(identity.gateway_generation, 202);
        assert_eq!(identity.request_id, "duplicate-second");
        assert_eq!(
            app.agents[&AgentId(1)]
                .remote_usage
                .snapshot()
                .unwrap()
                .status,
            crate::forge::remote_usage::RemoteUsageStatus::Loading
        );
        assert!(usage_request_is_current(identity).await);

        assert!(forget_binding(first.binding_generation).await);
        app.agents.shift_remove(&AgentId(1));
        publish_state(&app).await;
        assert_eq!(
            next_revocation(&mut second_events).await,
            RemoteRevocationReason::SessionClosed
        );
        assert!(
            binding_for_target(AgentId(1), &duplicate_session, second_epoch)
                .await
                .is_none()
        );
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn revoking_one_binding_cannot_revoke_another_pairing() {
        let _test = bridge_test_lock().lock().await;
        revoke_all(RemoteRevocationReason::Stopped).await;
        let old_session = acp::SessionId::new("old-session");
        let new_session = acp::SessionId::new("new-session");
        let old_transport = arm(AgentId(20), old_session.clone(), 1).await;
        assert!(bind_gateway_generation(AgentId(20), &old_session, 1, 10).await);
        let new_transport = arm(AgentId(21), new_session.clone(), 1).await;
        assert!(bind_gateway_generation(AgentId(21), &new_session, 1, 11).await);

        assert!(
            revoke_if_binding(
                old_transport.binding_generation,
                Some(10),
                RemoteRevocationReason::Stopped,
            )
            .await
        );
        new_transport
            .commands
            .send(RemoteCommandRequest {
                session_id: new_session.0.to_string(),
                gateway_generation: 11,
                client_generation: 1,
                command_id: "still-live".into(),
                command: RemoteCommand::Ping,
            })
            .await
            .expect("newer pairing must remain live");
        let inbound = tokio::time::timeout(std::time::Duration::from_secs(1), next_request())
            .await
            .expect("newer pairing must still wake the pager");
        assert_eq!(inbound.request.command_id, "still-live");
        assert_eq!(inbound.bridge_generation, new_transport.binding_generation);
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn lifecycle_lock_serializes_start_and_stop_transactions_across_awaits() {
        let _test = bridge_test_lock().lock().await;
        revoke_all(RemoteRevocationReason::Stopped).await;
        let first = lock_lifecycle().await;
        let second = tokio::spawn(async {
            let _guard = lock_lifecycle().await;
            true
        });
        tokio::task::yield_now().await;
        assert!(!second.is_finished());
        drop(first);
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), second)
                .await
                .unwrap()
                .unwrap()
        );

        let start_one = tokio::spawn(async {
            let _lifecycle = lock_lifecycle().await;
            arm(AgentId(40), acp::SessionId::new("parallel-one"), 1).await
        });
        let start_two = tokio::spawn(async {
            let _lifecycle = lock_lifecycle().await;
            arm(AgentId(41), acp::SessionId::new("parallel-two"), 1).await
        });
        let (one, two) = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            (start_one.await.unwrap(), start_two.await.unwrap())
        })
        .await
        .expect("independent lifecycle transactions must not deadlock");
        assert_ne!(one.binding_generation, two.binding_generation);
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn exact_target_prompt_isolated_from_the_current_terminal_tab() {
        let _test = bridge_test_lock().lock().await;
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let mut other = crate::app::agent_view::test_agent_view(
            Some("other-session"),
            std::path::PathBuf::from("/tmp/other"),
        );
        other.session.id = AgentId(1);
        app.agents.insert(AgentId(1), other);
        app.active_view = crate::app::app_view::ActiveView::Agent(AgentId(1));
        let transport = arm_test_app(&app, 71).await;
        let inbound = receive_command(
            &transport,
            71,
            "prompt",
            RemoteCommand::Prompt {
                text: "from phone".into(),
                images: Vec::new(),
            },
        )
        .await;

        let execution = execute_request(&mut app, inbound).await;
        assert!(matches!(execution.outcome, RemoteCommandOutcome::Ok));
        assert!(matches!(
            execution.effects.as_slice(),
            [Effect::SendPrompt { agent_id, .. }] if *agent_id == AgentId(0)
        ));
        assert_eq!(app.agents[&AgentId(1)].scrollback.len(), 0);
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn phone_ready_closes_the_pairing_qr_without_sending_a_prompt() {
        let _test = bridge_test_lock().lock().await;
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        crate::app::dispatch::dispatch(
            crate::app::actions::Action::ForgeRemoteControl(
                crate::forge::remote_control::RemoteControlCommand::Start,
            ),
            &mut app,
        );
        assert!(matches!(
            app.agents[&AgentId(0)].active_modal,
            Some(crate::views::modal::ActiveModal::ForgeRemote { .. })
        ));
        let transport = arm_test_app(&app, 81).await;
        let execution = execute_request(
            &mut app,
            receive_command(&transport, 81, "phone-ready-1", RemoteCommand::PhoneReady).await,
        )
        .await;
        assert!(matches!(execution.outcome, RemoteCommandOutcome::Ok));
        assert!(execution.effects.is_empty());
        assert!(app.agents[&AgentId(0)].active_modal.is_none());
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn prompt_images_travel_as_content_blocks() {
        let _test = bridge_test_lock().lock().await;
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let transport = arm_test_app(&app, 82).await;
        let execution = execute_request(
            &mut app,
            receive_command(
                &transport,
                82,
                "photo",
                RemoteCommand::Prompt {
                    text: "look at this".into(),
                    images: vec![xai_grok_shell::remote_control::RemotePromptImage {
                        name: "shot.png".into(),
                        mime_type: "image/png".into(),
                        data: "aaaa".into(),
                    }],
                },
            )
            .await,
        )
        .await;
        assert!(matches!(execution.outcome, RemoteCommandOutcome::Ok));
        assert!(matches!(
            execution.effects.as_slice(),
            [Effect::SendPromptBlocks { blocks, .. }]
                if blocks.iter().any(|block| matches!(block, acp::ContentBlock::Image(_)))
        ));
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[test]
    fn queued_prompt_controls_are_versioned_exact_target_effects() {
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let target = RemoteTarget {
            agent_id: AgentId(0),
            session_id: acp::SessionId::new("test-session"),
            session_binding_epoch: app.agents[&AgentId(0)].session_binding_epoch,
            bridge_generation: 91,
            gateway_generation: Some(92),
        };
        {
            let agent = app.agents.get_mut(&AgentId(0)).unwrap();
            agent.session.state = crate::app::agent::AgentState::TurnRunning;
            agent
                .shared_queue
                .push(crate::app::prompt_queue::QueueEntryWire {
                    id: "queued-1".into(),
                    version: 4,
                    owner: Some("phone".into()),
                    last_editor: None,
                    kind: "prompt".into(),
                    text: "follow up".into(),
                    combined_texts: None,
                    position: 0,
                });
        }

        assert!(matches!(
            remote_queue_control_effects(
                &app,
                &target,
                RemoteCommand::EditQueuedPrompt {
                    queue_item_id: "queued-1".into(),
                    expected_version: 4,
                    text: "edited".into(),
                },
            )
            .unwrap()
            .as_slice(),
            [Effect::QueueEdit {
                session_id,
                id,
                new_text,
                expected_version: Some(4),
            }] if session_id.0.as_ref() == "test-session" && id == "queued-1" && new_text == "edited"
        ));
        assert!(matches!(
            remote_queue_control_effects(
                &app,
                &target,
                RemoteCommand::SteerQueuedPrompt {
                    queue_item_id: "queued-1".into(),
                    expected_version: 4,
                },
            )
            .unwrap()
            .as_slice(),
            [Effect::QueueInterject {
                session_id,
                id,
                expected_version: 4,
                new_text: None,
            }] if session_id.0.as_ref() == "test-session" && id == "queued-1"
        ));
        assert!(matches!(
            remote_queue_control_effects(
                &app,
                &target,
                RemoteCommand::CancelQueuedPrompt {
                    queue_item_id: "queued-1".into(),
                    expected_version: 4,
                },
            )
            .unwrap()
            .as_slice(),
            [Effect::QueueRemove {
                session_id,
                id,
                expected_version: 4,
            }] if session_id.0.as_ref() == "test-session" && id == "queued-1"
        ));

        let stale = remote_queue_control_effects(
            &app,
            &target,
            RemoteCommand::CancelQueuedPrompt {
                queue_item_id: "queued-1".into(),
                expected_version: 3,
            },
        )
        .unwrap_err();
        assert_eq!(stale.0, "queue_item_stale");
        assert!(stale.2);

        app.agents.get_mut(&AgentId(0)).unwrap().session.state =
            crate::app::agent::AgentState::Idle;
        let idle = remote_queue_control_effects(
            &app,
            &target,
            RemoteCommand::SteerQueuedPrompt {
                queue_item_id: "queued-1".into(),
                expected_version: 4,
            },
        )
        .unwrap_err();
        assert_eq!(idle.0, "queue_action_unavailable");
    }

    #[tokio::test]
    async fn stale_binding_and_reconnecting_prompt_are_typed_rejections() {
        let _test = bridge_test_lock().lock().await;
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let transport = arm_test_app(&app, 72).await;
        let wrong_generation =
            receive_command(&transport, 999, "wrong-generation", RemoteCommand::Ping).await;
        let execution = execute_request(&mut app, wrong_generation).await;
        assert_eq!(outcome_error(&execution).code, "stale_binding");

        let reconnect = receive_command(
            &transport,
            72,
            "reconnect",
            RemoteCommand::Prompt {
                text: "wait".into(),
                images: Vec::new(),
            },
        )
        .await;
        app.reconnect_pending = true;
        let execution = execute_request(&mut app, reconnect).await;
        let error = outcome_error(&execution);
        assert_eq!(error.code, "session_reconnecting");
        assert!(error.retryable);
        assert!(execution.effects.is_empty());

        app.reconnect_pending = false;
        let stale = receive_command(&transport, 72, "stale", RemoteCommand::Ping).await;
        app.agents
            .get_mut(&AgentId(0))
            .unwrap()
            .session_binding_epoch += 1;
        let execution = execute_request(&mut app, stale).await;
        assert_eq!(outcome_error(&execution).code, "session_closed");
        let mut events = transport.events.subscribe();
        finish_execution(execution).await;
        assert!(matches!(
            events.recv().await.unwrap(),
            RemotePagerEvent::CommandResult {
                outcome: RemoteCommandOutcome::Error { ref error },
                ..
            } if error.code == "session_closed"
        ));
        publish_state(&app).await;
        assert!(matches!(
            events.recv().await.unwrap(),
            RemotePagerEvent::Revoked {
                reason: RemoteRevocationReason::SessionClosed,
                ..
            }
        ));
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn graceful_shutdown_revokes_all_transports_and_rejects_queued_commands() {
        let _test = bridge_test_lock().lock().await;
        let app = crate::app::app_view::tests::test_app_with_agent();
        let transport = arm_test_app(&app, 75).await;
        let mut events = transport.events.subscribe();

        revoke_all(RemoteRevocationReason::Stopped).await;
        assert!(matches!(
            events.recv().await.unwrap(),
            RemotePagerEvent::Revoked {
                reason: RemoteRevocationReason::Stopped,
                ..
            }
        ));
        transport
            .commands
            .send(RemoteCommandRequest {
                session_id: "test-session".into(),
                gateway_generation: 75,
                client_generation: 1,
                command_id: "after-shutdown".into(),
                command: RemoteCommand::Ping,
            })
            .await
            .unwrap();
        let inbound = tokio::time::timeout(std::time::Duration::from_secs(1), next_request())
            .await
            .expect("shared ingress remains wakeable");
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        assert_eq!(
            outcome_error(&execute_request(&mut app, inbound).await).code,
            "remote_not_armed"
        );
    }

    #[tokio::test]
    async fn snapshot_and_state_replacement_delta_are_monotonic_and_reconnectable() {
        let _test = bridge_test_lock().lock().await;
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let transport = arm_test_app(&app, 73).await;
        let mut events = transport.events.subscribe();
        publish_state(&app).await;
        let first = transport.snapshots.borrow().clone().unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(first.session["sessionId"], "test-session");

        app.agents.get_mut(&AgentId(0)).unwrap().session.state =
            crate::app::agent::AgentState::TurnRunning;
        publish_state(&app).await;
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        let RemotePagerEvent::Delta {
            base_revision,
            revision,
            event,
            ..
        } = event
        else {
            panic!("expected state replacement delta")
        };
        assert_eq!((base_revision, revision), (1, 2));
        assert_eq!(event["kind"], "stateReplaced");
        assert_eq!(event["session"]["sessionId"], "test-session");

        publish_state(&app).await;
        assert!(matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        let reconnect = transport.snapshots.clone();
        assert_eq!(reconnect.borrow().as_ref().unwrap().revision, 2);
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn streamed_assistant_reasoning_and_tool_changes_publish_transcript_splices() {
        use crate::scrollback::block::RenderBlock;

        let _test = bridge_test_lock().lock().await;
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let id = AgentId(0);
        app.agents.get_mut(&id).unwrap().session.state = crate::app::agent::AgentState::TurnRunning;
        let transport = arm_test_app(&app, 731).await;
        let mut events = transport.events.subscribe();
        publish_state(&app).await;
        assert_eq!(transport.snapshots.borrow().as_ref().unwrap().revision, 1);
        assert!(
            transport.snapshots.borrow().as_ref().unwrap().session["transcript"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let assistant_id = {
            let agent = app.agents.get_mut(&id).unwrap();
            let entry_id = agent
                .scrollback
                .push_block(RenderBlock::agent_message("stream"));
            agent.scrollback.set_entry_running(entry_id, true);
            entry_id
        };
        publish_state(&app).await;
        let (base, revision, assistant) = next_delta(&mut events).await;
        assert_eq!((base, revision), (1, 2));
        assert_eq!(assistant["kind"], "transcriptSpliced");
        assert_eq!(assistant["sessionId"], "test-session");
        assert_eq!(assistant["start"], 0);
        assert_eq!(assistant["deleteCount"], 0);
        assert_eq!(assistant["items"][0]["kind"], "assistant");
        assert_eq!(assistant["items"][0]["text"], "stream");
        assert_eq!(assistant["items"][0]["status"], "running");

        app.agents
            .get_mut(&id)
            .unwrap()
            .scrollback
            .push_chunk_to_agent(assistant_id, "ed");
        publish_state(&app).await;
        let (base, revision, assistant_chunk) = next_delta(&mut events).await;
        assert_eq!((base, revision), (2, 3));
        assert_eq!(assistant_chunk["kind"], "transcriptSpliced");
        assert_eq!(assistant_chunk["start"], 0);
        assert_eq!(assistant_chunk["deleteCount"], 1);
        assert_eq!(assistant_chunk["items"][0]["text"], "streamed");

        {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.scrollback.set_entry_running(assistant_id, false);
            let reasoning_id = agent
                .scrollback
                .push_block(RenderBlock::thinking("checking the stream"));
            agent.scrollback.set_entry_running(reasoning_id, true);
        }
        publish_state(&app).await;
        let (base, revision, reasoning) = next_delta(&mut events).await;
        assert_eq!((base, revision), (3, 4));
        assert_eq!(reasoning["kind"], "transcriptSpliced");
        assert_eq!(reasoning["sessionId"], "test-session");
        assert!(
            reasoning["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["kind"] == "reasoning"
                    && item["text"] == "checking the stream"
                    && item["status"] == "running")
        );

        let tool_id = {
            let agent = app.agents.get_mut(&id).unwrap();
            let reasoning_id = agent.scrollback.last().unwrap().id;
            agent.scrollback.set_entry_running(reasoning_id, false);
            let entry_id = agent
                .scrollback
                .push_block(RenderBlock::execute_with_output(
                    "cargo check",
                    "",
                    None::<String>,
                ));
            agent.scrollback.set_entry_running(entry_id, true);
            entry_id
        };
        publish_state(&app).await;
        let (base, revision, tool) = next_delta(&mut events).await;
        assert_eq!((base, revision), (4, 5));
        assert_eq!(tool["kind"], "transcriptSpliced");
        assert_eq!(tool["sessionId"], "test-session");
        assert!(
            tool["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["kind"] == "tool"
                    && item["title"] == "Run"
                    && item["status"] == "running")
        );

        app.agents
            .get_mut(&id)
            .unwrap()
            .scrollback
            .push_chunk_to_execute(tool_id, "Checking forge\n");
        publish_state(&app).await;
        let (base, revision, tool_chunk) = next_delta(&mut events).await;
        assert_eq!((base, revision), (5, 6));
        assert_eq!(tool_chunk["kind"], "transcriptSpliced");
        assert_eq!(tool_chunk["sessionId"], "test-session");
        assert_eq!(
            tool_chunk["items"].as_array().unwrap().last().unwrap()["kind"],
            "tool"
        );
        assert!(
            tool_chunk["items"].as_array().unwrap().last().unwrap()["output"]
                .as_str()
                .unwrap()
                .contains("Checking forge")
        );

        let reconnect = transport.snapshots.borrow().clone().unwrap();
        assert_eq!(reconnect.revision, 6);
        assert_eq!(reconnect.session["sessionId"], "test-session");
        assert_eq!(reconnect.session["status"], "running");
        assert_eq!(
            reconnect.session["transcript"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["kind"],
            "tool"
        );
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn fast_mode_is_capability_gated_pending_and_exact_targeted() {
        let _test = bridge_test_lock().lock().await;
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let id = AgentId(0);
        let fast_id = acp::ModelId::new("remote-fast-model");
        let fast_info = acp::ModelInfo::new(fast_id.clone(), "Remote Fast Model").meta(
            serde_json::json!({"supportsFastMode": true})
                .as_object()
                .cloned(),
        );
        {
            let agent = app.agents.get_mut(&id).unwrap();
            agent
                .session
                .models
                .available
                .insert(fast_id.clone(), fast_info);
            agent.session.models.set_current(fast_id, None);
        }
        let mut other = crate::app::agent_view::test_agent_view(
            Some("other-session"),
            std::path::PathBuf::from("/tmp/other"),
        );
        other.session.id = AgentId(1);
        app.agents.insert(AgentId(1), other);
        app.active_view = crate::app::app_view::ActiveView::Agent(AgentId(1));
        let transport = arm_test_app(&app, 76).await;

        let stale = execute_request(
            &mut app,
            receive_command(
                &transport,
                999,
                "stale-fast",
                RemoteCommand::SetFastMode { enabled: true },
            )
            .await,
        )
        .await;
        assert_eq!(outcome_error(&stale).code, "stale_binding");
        assert!(!app.agents[&id].session.models.fast_mode_pending());

        let accepted = execute_request(
            &mut app,
            receive_command(
                &transport,
                76,
                "enable-fast",
                RemoteCommand::SetFastMode { enabled: true },
            )
            .await,
        )
        .await;
        assert!(matches!(accepted.outcome, RemoteCommandOutcome::Ok));
        let (session_binding_epoch, request_id) = match accepted.effects.as_slice() {
            [
                Effect::SetFastMode {
                    agent_id,
                    session_id,
                    session_binding_epoch,
                    request_id,
                    enabled: true,
                },
            ] => {
                assert_eq!(*agent_id, id);
                assert_eq!(session_id.0.as_ref(), "test-session");
                (*session_binding_epoch, *request_id)
            }
            other => panic!("expected exact-target Fast Mode effect, got {other:?}"),
        };
        assert!(app.agents[&id].session.models.fast_mode_pending());
        assert!(!app.agents[&AgentId(1)].session.models.fast_mode);

        publish_state(&app).await;
        let pending_snapshot = transport.snapshots.borrow().clone().unwrap();
        assert_eq!(pending_snapshot.session["capabilities"]["fastMode"], true);
        assert_eq!(pending_snapshot.session["fastMode"]["enabled"], false);
        assert_eq!(pending_snapshot.session["fastMode"]["pending"], true);

        let model_during_fast = execute_request(
            &mut app,
            receive_command(
                &transport,
                76,
                "model-during-fast",
                RemoteCommand::SetModel {
                    model_id: "remote-fast-model".into(),
                    reasoning_effort: None,
                },
            )
            .await,
        )
        .await;
        let model_error = outcome_error(&model_during_fast);
        assert_eq!(model_error.code, "fast_mode_pending");
        assert!(model_error.retryable);
        assert!(model_during_fast.effects.is_empty());

        let duplicate = execute_request(
            &mut app,
            receive_command(
                &transport,
                76,
                "duplicate-fast",
                RemoteCommand::SetFastMode { enabled: true },
            )
            .await,
        )
        .await;
        let duplicate_error = outcome_error(&duplicate);
        assert_eq!(duplicate_error.code, "fast_mode_pending");
        assert!(duplicate_error.retryable);
        assert!(duplicate.effects.is_empty());

        crate::forge::fast_mode::handle_complete(
            &mut app,
            id,
            acp::SessionId::new("test-session"),
            session_binding_epoch,
            request_id,
            true,
            Ok(()),
        );
        publish_state(&app).await;
        let enabled_snapshot = transport.snapshots.borrow().clone().unwrap();
        assert_eq!(enabled_snapshot.session["fastMode"]["enabled"], true);
        assert!(
            enabled_snapshot.session["fastMode"]
                .get("pending")
                .is_none()
        );

        let standard_id = acp::ModelId::new("standard-model");
        {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.session.models.available.insert(
                standard_id.clone(),
                acp::ModelInfo::new(standard_id.clone(), "Standard Model"),
            );
            agent.session.models.set_current(standard_id, None);
        }
        let unsupported = execute_request(
            &mut app,
            receive_command(
                &transport,
                76,
                "unsupported-fast",
                RemoteCommand::SetFastMode { enabled: true },
            )
            .await,
        )
        .await;
        assert_eq!(outcome_error(&unsupported).code, "fast_mode_unsupported");
        assert!(unsupported.effects.is_empty());
        assert!(!app.agents[&id].session.models.fast_mode);
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn cancel_model_and_btw_commands_use_targeted_canonical_reducers() {
        let _test = bridge_test_lock().lock().await;
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let id = AgentId(0);
        app.current_ui.cancel_subagents_on_turn_cancel = Some("always_continue".into());
        app.agents.get_mut(&id).unwrap().session.state = crate::app::agent::AgentState::TurnRunning;
        let model_id = acp::ModelId::new("remote-model");
        app.agents
            .get_mut(&id)
            .unwrap()
            .session
            .models
            .available
            .insert(
                model_id.clone(),
                acp::ModelInfo::new(model_id, "Remote Model"),
            );
        let transport = arm_test_app(&app, 74).await;

        let cancel = execute_request(
            &mut app,
            receive_command(&transport, 74, "cancel", RemoteCommand::Cancel).await,
        )
        .await;
        assert!(matches!(cancel.outcome, RemoteCommandOutcome::Ok));
        assert!(matches!(
            cancel.effects.as_slice(),
            [Effect::CancelTurn { session_id, cancel_subagents: false, .. }]
                if session_id.0.as_ref() == "test-session"
        ));

        app.agents.get_mut(&id).unwrap().session.state = crate::app::agent::AgentState::Idle;
        let model = execute_request(
            &mut app,
            receive_command(
                &transport,
                74,
                "model",
                RemoteCommand::SetModel {
                    model_id: "remote-model".into(),
                    reasoning_effort: None,
                },
            )
            .await,
        )
        .await;
        assert!(matches!(
            model.effects.as_slice(),
            [Effect::SwitchModel { agent_id, .. }] if *agent_id == id
        ));

        let first_btw = execute_request(
            &mut app,
            receive_command(
                &transport,
                74,
                "btw-1",
                RemoteCommand::Btw {
                    question: "first question".into(),
                },
            )
            .await,
        )
        .await;
        assert!(matches!(
            first_btw.effects.as_slice(),
            [Effect::SendBtw { agent_id, question, .. }]
                if *agent_id == id && question == "first question"
        ));
        let second_btw = execute_request(
            &mut app,
            receive_command(
                &transport,
                74,
                "btw-2",
                RemoteCommand::Btw {
                    question: "second question".into(),
                },
            )
            .await,
        )
        .await;
        assert_eq!(outcome_error(&second_btw).code, "btw_in_progress");
        assert!(second_btw.effects.is_empty());
        assert_eq!(
            app.agents[&id].btw_state.as_ref().unwrap().question(),
            "first question"
        );
        revoke_all(RemoteRevocationReason::Stopped).await;
    }

    #[tokio::test]
    async fn in_memory_lifecycle_keeps_phone_and_terminal_on_one_authoritative_session() {
        use crate::acp::meta::NotificationMeta;
        use crate::app::actions::Action;
        use crate::scrollback::block::RenderBlock;

        let _test = bridge_test_lock().lock().await;
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let id = AgentId(0);
        {
            let agent = app.agents.get_mut(&id).unwrap();
            agent
                .scrollback
                .push_block(RenderBlock::user_prompt("pre-existing question"));
            agent
                .scrollback
                .push_block(RenderBlock::agent_message("pre-existing answer"));
            agent
                .scrollback
                .push_block(RenderBlock::execute_with_output(
                    "pwd",
                    "/tmp",
                    None::<String>,
                ));

            let model_id = acp::ModelId::new("acceptance-model");
            agent.session.models.available.insert(
                model_id.clone(),
                acp::ModelInfo::new(model_id.clone(), "Acceptance Model").meta(
                    serde_json::json!({
                        "supportsReasoningEffort": true,
                        "reasoningEffort": "high",
                        "reasoningEfforts": [
                            {"id": "high", "value": "high", "label": "High"},
                            {"id": "deep", "value": "xhigh", "label": "Deep"}
                        ]
                    })
                    .as_object()
                    .cloned(),
                ),
            );
            agent.session.models.set_current(
                model_id,
                Some(xai_grok_shell::sampling::types::ReasoningEffort::High),
            );
        }

        let transport = arm_test_app(&app, 101).await;
        let mut events = transport.events.subscribe();
        publish_state(&app).await;
        let initial = transport.snapshots.borrow().clone().unwrap();
        assert_eq!(initial.revision, 1);
        assert_eq!(initial.session["sessionId"], "test-session");
        assert_eq!(initial.session["currentModel"]["id"], "acceptance-model");
        assert_eq!(
            initial.session["transcript"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["kind"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["user", "assistant", "tool"]
        );

        let phone_prompt = execute_request(
            &mut app,
            receive_command(
                &transport,
                101,
                "phone-prompt",
                RemoteCommand::Prompt {
                    text: "prompt from phone".into(),
                    images: Vec::new(),
                },
            )
            .await,
        )
        .await;
        assert!(matches!(phone_prompt.outcome, RemoteCommandOutcome::Ok));
        assert!(matches!(
            phone_prompt.effects.as_slice(),
            [Effect::SendPrompt { agent_id, session_id, text, .. }]
                if *agent_id == id
                    && session_id.0.as_ref() == "test-session"
                    && text == "prompt from phone"
        ));
        assert!(app.agents[&id].session.state.is_turn_running());
        finish_execution(phone_prompt).await;
        publish_state(&app).await;
        let (base, revision, delta) = next_delta(&mut events).await;
        assert_eq!((base, revision), (1, 2));
        assert_eq!(delta["kind"], "stateReplaced");
        assert!(
            delta["session"]["transcript"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["kind"] == "user" && item["text"] == "prompt from phone")
        );

        {
            let agent = app.agents.get_mut(&id).unwrap();
            let meta = NotificationMeta::default();
            assert!(agent.session.handle_update(
                acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                    acp::ContentBlock::Text(acp::TextContent::new("streamed from ACP")),
                )),
                &meta,
                &mut agent.scrollback,
            ));
            assert!(
                agent.session.handle_update(
                    acp::SessionUpdate::ToolCall(
                        acp::ToolCall::new(acp::ToolCallId::new("acceptance-tool"), "Execute")
                            .kind(acp::ToolKind::Execute)
                            .status(acp::ToolCallStatus::Completed)
                            .content(vec![])
                            .raw_input(Some(serde_json::json!({"command": "cargo check"})))
                            .locations(vec![]),
                    ),
                    &meta,
                    &mut agent.scrollback,
                )
            );
        }
        publish_state(&app).await;
        let (_, authoritative_revision, _) = next_delta(&mut events).await;
        let reconnect = transport.snapshots.clone();
        let reconnect_snapshot = reconnect.borrow().clone().unwrap();
        assert_eq!(reconnect_snapshot.revision, authoritative_revision);
        assert_eq!(reconnect_snapshot.session["status"], "running");
        assert!(
            reconnect_snapshot.session["transcript"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["kind"] == "assistant"
                    && item["text"].as_str().unwrap().contains("streamed from ACP"))
        );
        assert!(
            reconnect_snapshot.session["transcript"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["kind"] == "tool" && item["input"] == "cargo check")
        );

        app.current_ui.cancel_subagents_on_turn_cancel = Some("always_continue".into());
        let cancel = execute_request(
            &mut app,
            receive_command(&transport, 101, "cancel", RemoteCommand::Cancel).await,
        )
        .await;
        assert!(matches!(cancel.outcome, RemoteCommandOutcome::Ok));
        assert!(matches!(
            cancel.effects.as_slice(),
            [Effect::CancelTurn { session_id, cancel_subagents: false, .. }]
                if session_id.0.as_ref() == "test-session"
        ));
        finish_execution(cancel).await;
        publish_state(&app).await;
        let _ = next_delta(&mut events).await;

        app.agents.get_mut(&id).unwrap().session.state = crate::app::agent::AgentState::Idle;
        let model = execute_request(
            &mut app,
            receive_command(
                &transport,
                101,
                "model-and-reasoning",
                RemoteCommand::SetModel {
                    model_id: "acceptance-model".into(),
                    reasoning_effort: Some("deep".into()),
                },
            )
            .await,
        )
        .await;
        assert!(matches!(model.outcome, RemoteCommandOutcome::Ok));
        assert!(matches!(
            model.effects.as_slice(),
            [Effect::SwitchModel { agent_id, model_id, effort, .. }]
                if *agent_id == id
                    && model_id.0.as_ref() == "acceptance-model"
                    && *effort == Some(xai_grok_shell::sampling::types::ReasoningEffort::Xhigh)
        ));
        finish_execution(model).await;
        publish_state(&app).await;
        let _ = next_delta(&mut events).await;

        let btw = execute_request(
            &mut app,
            receive_command(
                &transport,
                101,
                "btw",
                RemoteCommand::Btw {
                    question: "what changed?".into(),
                },
            )
            .await,
        )
        .await;
        assert!(matches!(btw.outcome, RemoteCommandOutcome::Ok));
        assert!(matches!(
            btw.effects.as_slice(),
            [Effect::SendBtw { agent_id, question, .. }]
                if *agent_id == id && question == "what changed?"
        ));
        finish_execution(btw).await;
        publish_state(&app).await;
        let _ = next_delta(&mut events).await;
        assert!(
            transport.snapshots.borrow().as_ref().unwrap().session["transcript"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["kind"] == "btw"
                    && item["question"] == "what changed?"
                    && item["status"] == "running")
        );

        {
            let agent = app.agents.get_mut(&id).unwrap();
            agent.btw_state = None;
            agent.session.finish_turn(&mut agent.scrollback);
            // Model switching is optimistic until the shell confirms it. The
            // acceptance harness supplies that acknowledgement explicitly so
            // the next terminal prompt exercises the normal idle send path.
            agent.session.model_switch_pending = false;
        }
        let terminal_effects = crate::app::dispatch::dispatch(
            Action::SendPrompt("prompt from terminal".into()),
            &mut app,
        );
        assert!(terminal_effects.iter().any(|effect| matches!(
            effect,
            Effect::SendPrompt { agent_id, text, .. }
                if *agent_id == id && text == "prompt from terminal"
        )));
        publish_state(&app).await;
        let (_, terminal_revision, terminal_delta) = next_delta(&mut events).await;
        assert_eq!(terminal_delta["kind"], "stateReplaced");
        assert!(
            terminal_delta["session"]["transcript"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["kind"] == "user" && item["text"] == "prompt from terminal")
        );

        let mut permission_response = attach_permission(&mut app, "permission-phone", 1);
        publish_state(&app).await;
        let permission_snapshot = transport.snapshots.borrow().clone().unwrap();
        let permission_id = interaction_id(&permission_snapshot, "permission");
        let permission = execute_request(
            &mut app,
            receive_command(
                &transport,
                101,
                "permission-phone-first",
                RemoteCommand::ResolveInteraction {
                    interaction_id: permission_id,
                    response: RemoteInteractionResponse::Permission {
                        option_id: "allow-once".into(),
                    },
                },
            )
            .await,
        )
        .await;
        assert!(matches!(permission.outcome, RemoteCommandOutcome::Ok));
        assert!(permission.effects.is_empty());
        assert!(matches!(
            permission_response.try_recv(),
            Ok(Ok(acp::RequestPermissionResponse {
                outcome: acp::RequestPermissionOutcome::Selected(_),
                ..
            }))
        ));
        finish_execution(permission).await;
        publish_state(&app).await;
        assert!(
            transport.snapshots.borrow().as_ref().unwrap().session["activeInteractions"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let mut question_response = attach_question(&mut app, "question-terminal");
        publish_state(&app).await;
        let question_id =
            interaction_id(transport.snapshots.borrow().as_ref().unwrap(), "question");
        let _ = app
            .agents
            .get_mut(&id)
            .unwrap()
            .submit_question_answers_for_test(true);
        assert!(question_response.try_recv().is_ok());
        publish_state(&app).await;
        let terminal_first = execute_request(
            &mut app,
            receive_command(
                &transport,
                101,
                "question-terminal-first",
                RemoteCommand::ResolveInteraction {
                    interaction_id: question_id,
                    response: RemoteInteractionResponse::Cancel,
                },
            )
            .await,
        )
        .await;
        assert_eq!(outcome_error(&terminal_first).code, "command_rejected");
        assert!(
            outcome_error(&terminal_first)
                .message
                .contains("already resolved")
        );

        let plan_response = attach_plan(&mut app, "plan-dropped");
        publish_state(&app).await;
        let plan_id = interaction_id(transport.snapshots.borrow().as_ref().unwrap(), "plan");
        drop(plan_response);
        let dropped_first = execute_request(
            &mut app,
            receive_command(
                &transport,
                101,
                "plan-dropped-first",
                RemoteCommand::ResolveInteraction {
                    interaction_id: plan_id,
                    response: RemoteInteractionResponse::Plan {
                        outcome: RemotePlanOutcome::Approved,
                        feedback: None,
                    },
                },
            )
            .await,
        )
        .await;
        assert_eq!(outcome_error(&dropped_first).code, "command_rejected");
        assert!(
            outcome_error(&dropped_first)
                .message
                .contains("already resolved")
        );
        assert!(app.agents[&id].plan_approval_view.is_some());

        app.agents.get_mut(&id).unwrap().plan_approval_view = None;
        publish_state(&app).await;
        let reconnect_after_races = transport.snapshots.clone();
        assert!(reconnect_after_races.borrow().as_ref().unwrap().revision > terminal_revision);
        assert!(
            reconnect_after_races.borrow().as_ref().unwrap().session["transcript"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["text"] == "pre-existing answer")
        );

        let mut old_events = transport.events.subscribe();
        let mut other = crate::app::agent_view::test_agent_view(
            Some("replacement-session"),
            std::path::PathBuf::from("/tmp/replacement"),
        );
        other.session.id = AgentId(1);
        other
            .scrollback
            .push_block(RenderBlock::user_prompt("replacement-only history"));
        app.agents.insert(AgentId(1), other);
        let replacement_session = acp::SessionId::new("replacement-session");
        let replacement = arm(AgentId(1), replacement_session.clone(), 0).await;
        assert_ne!(replacement.binding_generation, transport.binding_generation);
        assert!(matches!(
            old_events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        assert!(
            bind_gateway_generation(AgentId(1), &replacement_session, 0, 102).await,
            "new session must own its gateway generation"
        );
        publish_state(&app).await;
        let replacement_snapshot = replacement.snapshots.borrow().clone().unwrap();
        assert_eq!(replacement_snapshot.session_id, "replacement-session");
        assert_eq!(replacement_snapshot.revision, 1);
        assert_eq!(
            replacement_snapshot.session["transcript"][0]["text"],
            "replacement-only history"
        );
        assert!(
            !replacement_snapshot.session["transcript"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["text"] == "pre-existing answer")
        );
        assert!(
            transport
                .snapshots
                .borrow()
                .as_ref()
                .is_some_and(|snapshot| {
                    snapshot.session_id == "test-session"
                        && snapshot.session["transcript"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|item| item["text"] == "pre-existing answer")
                })
        );
        transport
            .commands
            .send(RemoteCommandRequest {
                session_id: "test-session".into(),
                gateway_generation: 101,
                client_generation: 1,
                command_id: "old-session-still-live".into(),
                command: RemoteCommand::Ping,
            })
            .await
            .unwrap();
        let old_inbound = tokio::time::timeout(std::time::Duration::from_secs(1), next_request())
            .await
            .expect("old session command must remain wakeable");
        assert_eq!(old_inbound.bridge_generation, transport.binding_generation);
        assert!(matches!(
            execute_request(&mut app, old_inbound).await.outcome,
            RemoteCommandOutcome::Ok
        ));
        revoke_all(RemoteRevocationReason::Stopped).await;
    }
}
