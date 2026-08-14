//! Private, tailnet-only Forge Remote gateway and protocol.
//!
//! The HTTP server is loopback-only. Tailscale is changed only after explicit
//! `/rc` activation, using one opaque path that Forge can later remove
//! without disturbing any unrelated Serve configuration.

use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::{Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use qrcodegen::{QrCode, QrCodeEcc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::sync::{Mutex, broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub const REMOTE_PROTOCOL_VERSION: u16 = 1;

const PAIRING_BYTES: usize = 32;
const MAX_COMMAND_OUTPUT: usize = 64 * 1024;
const MAX_REMOTE_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_COMMAND_ID_BYTES: usize = 128;
const MAX_PROMPT_BYTES: usize = 64 * 1024;
const MAX_PROMPT_IMAGES: usize = 8;
const MAX_PROMPT_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_SHORT_FIELD_BYTES: usize = 4 * 1024;
const PAIRING_TTL: Duration = Duration::from_secs(8 * 60 * 60);
const TAILSCALE_TIMEOUT: Duration = Duration::from_secs(15);
const GATEWAY_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
const SNAPSHOT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const SESSION_ACCEPT_RESULT_WRITE_TIMEOUT: Duration = Duration::from_secs(3);
const REMOTE_CLIENT_SUPERSEDED_CLOSE_CODE: u16 = 4410;
const REMOTE_CLIENT_SUPERSEDED_REASON: &str =
    "Forge Remote was superseded by a newer active client.";

const INDEX_HTML: &str = include_str!("../remote-ui/dist/index.html");
const APP_JS: &str = include_str!("../remote-ui/dist/assets/app.js");
const APP_CSS: &str = include_str!("../remote-ui/dist/assets/app.css");
const BASIER_REGULAR_WOFF2: &[u8] =
    include_bytes!("../remote-ui/dist/assets/basier-square-regular.woff2");
const BASIER_SEMIBOLD_WOFF2: &[u8] =
    include_bytes!("../remote-ui/dist/assets/basier-square-semibold.woff2");
const MANIFEST: &str = include_str!("../remote-ui/dist/manifest.webmanifest");
const ICON_SVG: &str = include_str!("../remote-ui/dist/icon.svg");
const THIRD_PARTY_NOTICES: &str = include_str!("../remote-ui/dist/THIRD_PARTY_NOTICES.txt");

const REMOTE_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self' wss: ws:; manifest-src 'self'; font-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

static ACTIVE_GATEWAYS: OnceLock<Mutex<HashMap<u64, RemoteGateway>>> = OnceLock::new();
static PENDING_ROUTE_CLEANUPS: OnceLock<Mutex<HashMap<u64, RemoteArm>>> = OnceLock::new();
static GATEWAY_OPERATIONS: OnceLock<Mutex<()>> = OnceLock::new();
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

fn active_gateways() -> &'static Mutex<HashMap<u64, RemoteGateway>> {
    ACTIVE_GATEWAYS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn gateway_operations() -> &'static Mutex<()> {
    GATEWAY_OPERATIONS.get_or_init(|| Mutex::new(()))
}

fn pending_route_cleanups() -> &'static Mutex<HashMap<u64, RemoteArm>> {
    PENDING_ROUTE_CLEANUPS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TailscalePrerequisite {
    Ready { dns_name: String },
    MissingCli,
    NotRunning,
    NotSignedIn,
    MissingDnsName,
    Unsupported(String),
}

impl TailscalePrerequisite {
    pub fn user_message(&self) -> String {
        match self {
            Self::Ready { dns_name } => format!(
                "Tailscale is ready on this Mac ({dns_name}). Forge has not changed its configuration."
            ),
            Self::MissingCli => "Forge Remote needs Tailscale on this Mac and your phone. Install and sign in to the official Tailscale app on both devices, enable MagicDNS + HTTPS for your tailnet, then retry `/rc`. Forge did not change your Tailscale configuration.".into(),
            Self::NotRunning => "Forge Remote needs Tailscale running and signed in on this Mac and your phone. Open Tailscale on this Mac, sign in, enable MagicDNS + HTTPS for your tailnet, then retry `/rc`. Forge did not change your Tailscale configuration.".into(),
            Self::NotSignedIn => "Forge Remote needs this Mac signed in to Tailscale. Sign in on this Mac and your phone, enable MagicDNS + HTTPS for your tailnet, then retry `/rc`. Forge did not change your Tailscale configuration.".into(),
            Self::MissingDnsName => "Forge Remote needs Tailscale MagicDNS + HTTPS enabled for this tailnet. Enable them in Tailscale's admin console, then retry `/rc`. Forge did not change your Tailscale configuration.".into(),
            Self::Unsupported(error) => format!(
                "Forge Remote could not verify Tailscale safely: {error}. Forge did not change your Tailscale configuration."
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TailscaleStatus {
    #[serde(rename = "BackendState")]
    backend_state: Option<String>,
    #[serde(rename = "MagicDNSSuffix")]
    magic_dns_suffix: Option<String>,
    #[serde(rename = "Self")]
    self_node: Option<TailscaleSelf>,
}

#[derive(Debug, Deserialize)]
struct TailscaleSelf {
    #[serde(rename = "DNSName")]
    dns_name: Option<String>,
}

pub fn parse_tailscale_status(json: &str) -> TailscalePrerequisite {
    let Ok(status) = serde_json::from_str::<TailscaleStatus>(json) else {
        return TailscalePrerequisite::Unsupported(
            "`tailscale status --json` returned invalid data".into(),
        );
    };
    if status.backend_state.as_deref() != Some("Running") {
        return TailscalePrerequisite::NotRunning;
    }
    let Some(dns_name) = status.self_node.and_then(|node| node.dns_name) else {
        return TailscalePrerequisite::NotSignedIn;
    };
    if status
        .magic_dns_suffix
        .as_deref()
        .unwrap_or_default()
        .is_empty()
    {
        return TailscalePrerequisite::MissingDnsName;
    }
    TailscalePrerequisite::Ready { dns_name }
}

pub async fn check_tailscale() -> TailscalePrerequisite {
    match run_command_with_timeout("tailscale", &["status", "--json"]).await {
        Ok(output) if output.status.success() => {
            let stdout = bounded_output(&output.stdout);
            parse_tailscale_status(&stdout)
        }
        Ok(_) => TailscalePrerequisite::NotRunning,
        Err(CommandFailure::Missing) => TailscalePrerequisite::MissingCli,
        Err(error) => TailscalePrerequisite::Unsupported(error.to_string()),
    }
}

#[derive(Clone)]
pub struct RemoteArm {
    /// Opaque pager bridge incarnation. This is process-local and never appears
    /// in a URL or browser message.
    pub binding_generation: u64,
    pub session_id: String,
    pub gateway_generation: u64,
    pairing_token: String,
    local_url: String,
    pub remote_url: Option<String>,
    route_may_exist: bool,
    pub expires_at: Instant,
    expires_at_rfc3339: String,
}

impl fmt::Debug for RemoteArm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RemoteArm")
            .field("binding_generation", &self.binding_generation)
            .field("session_id", &self.session_id)
            .field("gateway_generation", &self.gateway_generation)
            .field("pairing_token", &"[redacted]")
            .field("local_url", &self.local_url)
            .field("remote_enabled", &self.remote_url.is_some())
            .field("route_may_exist", &self.route_may_exist)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl RemoteArm {
    pub fn path(&self) -> String {
        format!("/forge/{}", self.pairing_token)
    }

    fn path_with_slash(&self) -> String {
        format!("{}/", self.path())
    }

    fn loopback_mount_target(&self) -> String {
        format!("{}{}", self.local_url, self.path_with_slash())
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    pub fn expires_at_rfc3339(&self) -> &str {
        &self.expires_at_rfc3339
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RemoteCommand {
    Prompt {
        text: String,
        #[serde(default)]
        images: Vec<RemotePromptImage>,
    },
    /// Synthesized by the gateway after an authenticated client receives its
    /// first session snapshot. The pager uses this to dismiss the `/rc` QR.
    PhoneReady,
    /// Create a normal session in the cwd owned by this exact remote binding.
    /// No path is accepted from the client.
    NewSession {},
    /// Confirm that the provisional child pairing was persisted and validated
    /// by the exact client that requested it.
    AcceptNewSession {
        session_id: String,
    },
    EditQueuedPrompt {
        queue_item_id: String,
        expected_version: u64,
        text: String,
    },
    SteerQueuedPrompt {
        queue_item_id: String,
        expected_version: u64,
    },
    CancelQueuedPrompt {
        queue_item_id: String,
        expected_version: u64,
    },
    Cancel,
    SetModel {
        model_id: String,
        reasoning_effort: Option<String>,
    },
    SetFastMode {
        enabled: bool,
    },
    Btw {
        question: String,
    },
    ResolveInteraction {
        interaction_id: String,
        response: RemoteInteractionResponse,
    },
    RefreshUsage,
    Resync,
    Ping,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RemoteInteractionResponse {
    Permission {
        option_id: String,
    },
    PermissionFollowup {
        text: String,
    },
    Question {
        answers: Vec<RemoteQuestionAnswer>,
    },
    Plan {
        outcome: RemotePlanOutcome,
        feedback: Option<String>,
    },
    Cancel,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteQuestionAnswer {
    pub question_index: usize,
    pub option_indices: Vec<usize>,
    pub freeform: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemotePlanOutcome {
    Approved,
    Cancelled,
    Abandoned,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl RemoteError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RemoteCommandOutcome {
    Ok,
    Error { error: RemoteError },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RemoteRevocationReason {
    Stopped,
    Expired,
    SessionClosed,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemotePromptImage {
    pub name: String,
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Clone)]
pub struct RemoteCommandRequest {
    pub session_id: String,
    pub gateway_generation: u64,
    /// Exact WebSocket ownership lease that submitted the command. Terminal
    /// results must never migrate to a newer client on the same bearer.
    pub client_generation: u64,
    pub command_id: String,
    pub command: RemoteCommand,
}

#[derive(Debug, Clone)]
pub struct RemoteSnapshot {
    pub session_id: String,
    pub revision: u64,
    pub session: serde_json::Value,
}

#[derive(Debug)]
pub enum RemoteSessionAcceptance {
    Begin {
        granted: tokio::sync::oneshot::Sender<()>,
    },
    Commit,
    Abort,
}

/// Typed metadata attached to a successful turn-completion timeline marker.
///
/// The pager derives these IDs from the same canonical scrollback entries it
/// projects onto the wire. Clients can therefore present the completed turn's
/// work without parsing the marker's human-readable text.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkDisclosure {
    pub duration_ms: u64,
    pub final_response_item_id: Option<String>,
    pub work_item_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum RemotePagerEvent {
    Delta {
        session_id: String,
        base_revision: u64,
        revision: u64,
        event: serde_json::Value,
    },
    CommandResult {
        session_id: String,
        client_generation: u64,
        command_id: String,
        outcome: RemoteCommandOutcome,
    },
    SessionCreated {
        /// Source session whose authenticated socket requested the handoff.
        session_id: String,
        client_generation: u64,
        command_id: String,
        new_session_id: String,
        pairing_url: String,
        expires_at: String,
        /// Two-phase application acceptance transaction for the exact source
        /// client. The child remains provisional until secure persistence and
        /// validation complete and the bounded OK write is committed.
        delivery_ack: mpsc::UnboundedSender<RemoteSessionAcceptance>,
    },
    Error {
        session_id: String,
        error: RemoteError,
    },
    Revoked {
        session_id: String,
        reason: RemoteRevocationReason,
    },
}

#[derive(Clone)]
pub struct RemoteTransport {
    /// Identifies one pager bridge incarnation. A rebind must use a fresh value
    /// even when the canonical session ID is unchanged.
    pub binding_generation: u64,
    pub commands: mpsc::Sender<RemoteCommandRequest>,
    pub events: broadcast::Sender<RemotePagerEvent>,
    pub snapshots: watch::Receiver<Option<RemoteSnapshot>>,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum ClientMessage {
    Hello {
        protocol_version: u16,
    },
    Command {
        protocol_version: u16,
        command_id: String,
        command: RemoteCommand,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum ServerMessage {
    Connected {
        protocol_version: u16,
        session_id: String,
        expires_at: String,
    },
    Snapshot {
        protocol_version: u16,
        revision: u64,
        session: serde_json::Value,
    },
    Delta {
        protocol_version: u16,
        base_revision: u64,
        revision: u64,
        event: serde_json::Value,
    },
    CommandResult {
        protocol_version: u16,
        command_id: String,
        outcome: RemoteCommandOutcome,
    },
    SessionCreated {
        protocol_version: u16,
        command_id: String,
        session_id: String,
        pairing_url: String,
        expires_at: String,
    },
    ResyncRequired {
        protocol_version: u16,
        reason: String,
    },
    Pong {
        protocol_version: u16,
        command_id: String,
    },
    Revoked {
        protocol_version: u16,
        reason: RemoteRevocationReason,
    },
    Error {
        protocol_version: u16,
        error: RemoteError,
    },
}

#[derive(Clone)]
struct GatewayState {
    session_id: Arc<str>,
    generation: u64,
    pairing_token: Arc<str>,
    expires_at: Instant,
    expires_at_rfc3339: Arc<str>,
    client_ownership: RemoteClientOwnership,
    commands: mpsc::Sender<RemoteCommandRequest>,
    events: broadcast::Sender<RemotePagerEvent>,
    snapshots: watch::Receiver<Option<RemoteSnapshot>>,
    lifecycle: broadcast::Sender<RemoteRevocationReason>,
}

#[derive(Clone, Default)]
struct RemoteClientOwnership {
    inner: Arc<std::sync::Mutex<RemoteClientOwnershipState>>,
}

#[derive(Default)]
struct RemoteClientOwnershipState {
    next_generation: u64,
    active: Option<ActiveRemoteClient>,
}

struct ActiveRemoteClient {
    generation: u64,
    superseded: CancellationToken,
}

struct RemoteClientLease {
    ownership: RemoteClientOwnership,
    generation: u64,
    superseded: CancellationToken,
}

impl RemoteClientOwnership {
    fn claim(&self) -> RemoteClientLease {
        let superseded = CancellationToken::new();
        let previous = {
            let mut state = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.next_generation = state.next_generation.wrapping_add(1).max(1);
            let generation = state.next_generation;
            let previous = state.active.replace(ActiveRemoteClient {
                generation,
                superseded: superseded.clone(),
            });
            (generation, previous)
        };
        let (generation, previous) = previous;
        if let Some(previous) = previous {
            previous.superseded.cancel();
        }
        RemoteClientLease {
            ownership: self.clone(),
            generation,
            superseded,
        }
    }

    fn current_generation(&self) -> Option<u64> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active
            .as_ref()
            .map(|active| active.generation)
    }

    fn with_current<R>(&self, generation: u64, operation: impl FnOnce() -> R) -> Option<R> {
        let state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.active.as_ref().map(|active| active.generation) != Some(generation) {
            return None;
        }
        Some(operation())
    }

    fn release(&self, generation: u64) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.active.as_ref().map(|active| active.generation) == Some(generation) {
            state.active = None;
        }
    }
}

impl RemoteClientLease {
    fn is_current(&self) -> bool {
        self.ownership.current_generation() == Some(self.generation)
    }

    fn with_current<R>(&self, operation: impl FnOnce() -> R) -> Option<R> {
        self.ownership.with_current(self.generation, operation)
    }
}

impl Drop for RemoteClientLease {
    fn drop(&mut self) {
        self.ownership.release(self.generation);
    }
}

impl GatewayState {
    fn accepts_token(&self, token: &str) -> bool {
        !self.is_expired() && constant_time_token_eq(token, &self.pairing_token)
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    fn current_snapshot(&self) -> Option<RemoteSnapshot> {
        self.snapshots
            .borrow()
            .clone()
            .filter(|snapshot| snapshot.session_id == self.session_id.as_ref())
            .filter(|snapshot| payload_session_matches(&snapshot.session, &self.session_id))
    }
}

pub struct RemoteGateway {
    pub arm: RemoteArm,
    binding_generation: u64,
    cancel: CancellationToken,
    lifecycle: broadcast::Sender<RemoteRevocationReason>,
    server_task: JoinHandle<()>,
    expiry_task: JoinHandle<()>,
}

impl RemoteGateway {
    async fn start(session_id: String, transport: RemoteTransport) -> Result<Self, String> {
        let binding_generation = transport.binding_generation;
        let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
        let mut secret = [0_u8; PAIRING_BYTES];
        rand::rng().fill_bytes(&mut secret);
        let pairing_token = hex_token(&secret);
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .map_err(|error| {
                format!("couldn't start the private Forge Remote listener: {error}")
            })?;
        let address = listener.local_addr().map_err(|error| {
            format!("couldn't read the private Forge Remote listener address: {error}")
        })?;
        let expires_at = Instant::now() + PAIRING_TTL;
        let expires_at_rfc3339 = (chrono::Utc::now()
            + chrono::Duration::from_std(PAIRING_TTL)
                .unwrap_or_else(|_| chrono::Duration::hours(8)))
        .to_rfc3339();
        let (lifecycle, _) = broadcast::channel(8);
        let state = GatewayState {
            session_id: Arc::from(session_id.as_str()),
            generation,
            pairing_token: Arc::from(pairing_token.as_str()),
            expires_at,
            expires_at_rfc3339: Arc::from(expires_at_rfc3339.as_str()),
            client_ownership: RemoteClientOwnership::default(),
            commands: transport.commands,
            events: transport.events,
            snapshots: transport.snapshots,
            lifecycle: lifecycle.clone(),
        };
        let app = gateway_router(state);
        let cancel = CancellationToken::new();
        let server_cancel = cancel.clone();
        let server_task = tokio::spawn(async move {
            let result = axum::serve(listener, app)
                .with_graceful_shutdown(server_cancel.cancelled_owned())
                .await;
            if let Err(error) = result {
                tracing::warn!(%error, "Forge Remote loopback gateway stopped unexpectedly");
            }
        });
        let expiry_cancel = cancel.clone();
        let expiry_task = tokio::spawn(async move {
            tokio::select! {
                _ = expiry_cancel.cancelled() => {}
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(expires_at)) => {
                    tokio::spawn(async move {
                        if let Err(error) = stop_matching_gateway(
                            binding_generation,
                            generation,
                            RemoteRevocationReason::Expired,
                        ).await {
                            tracing::warn!(%error, "Forge Remote expiry cleanup was incomplete");
                        }
                    });
                }
            }
        });
        Ok(Self {
            arm: RemoteArm {
                binding_generation,
                session_id,
                gateway_generation: generation,
                pairing_token,
                local_url: format!("http://{address}"),
                remote_url: None,
                route_may_exist: false,
                expires_at,
                expires_at_rfc3339,
            },
            binding_generation,
            cancel,
            lifecycle,
            server_task,
            expiry_task,
        })
    }

    async fn shutdown(mut self, reason: RemoteRevocationReason) {
        let _ = self.lifecycle.send(reason);
        self.cancel.cancel();
        self.expiry_task.abort();
        if tokio::time::timeout(GATEWAY_SHUTDOWN_TIMEOUT, &mut self.server_task)
            .await
            .is_err()
        {
            self.server_task.abort();
        }
    }
}

pub async fn arm_active_gateway(
    session_id: String,
    transport: RemoteTransport,
) -> Result<RemoteArm, String> {
    let binding_generation = transport.binding_generation;
    let _operation = gateway_operations().lock().await;
    retry_pending_route_cleanup(binding_generation).await?;
    {
        let active = active_gateways().lock().await;
        if let Some(gateway) = active.get(&binding_generation)
            && same_remote_binding(
                &gateway.arm.session_id,
                gateway.binding_generation,
                &session_id,
                transport.binding_generation,
            )
            && !gateway.arm.is_expired()
        {
            return Ok(gateway.arm.clone());
        }
    }
    if let Some(old) = active_gateways().lock().await.remove(&binding_generation) {
        let old_arm = old.arm.clone();
        old.shutdown(RemoteRevocationReason::Stopped).await;
        cleanup_route_or_retain(&old_arm).await?;
    }
    let gateway = RemoteGateway::start(session_id, transport).await?;
    let arm = gateway.arm.clone();
    active_gateways()
        .lock()
        .await
        .insert(binding_generation, gateway);
    Ok(arm)
}

fn same_remote_binding(
    active_session_id: &str,
    active_binding_generation: u64,
    requested_session_id: &str,
    requested_binding_generation: u64,
) -> bool {
    active_session_id == requested_session_id
        && active_binding_generation == requested_binding_generation
}

pub async fn active_gateway_arm(binding_generation: u64) -> Option<RemoteArm> {
    let active = active_gateways().lock().await;
    active
        .get(&binding_generation)
        .filter(|gateway| !gateway.arm.is_expired())
        .map(|gateway| gateway.arm.clone())
}

pub async fn enable_private_tailscale_route(
    binding_generation: u64,
    dns_name: &str,
    expected_session_id: &str,
    expected_generation: u64,
) -> Result<RemoteArm, String> {
    let _operation = gateway_operations().lock().await;
    let arm = {
        let active = active_gateways().lock().await;
        let gateway = active
            .get(&binding_generation)
            .ok_or_else(|| "Forge Remote is not armed. Run `/rc` first.".to_string())?;
        if gateway.arm.session_id != expected_session_id
            || gateway.arm.gateway_generation != expected_generation
        {
            return Err("Forge Remote pairing changed before it could be enabled. Run `/rc` again from the session you want to share.".into());
        }
        if gateway.arm.is_expired() {
            return Err(
                "Forge Remote pairing expired. Run `/rc` to create a fresh private link.".into(),
            );
        }
        if gateway.arm.remote_url.is_some() {
            return Ok(gateway.arm.clone());
        }
        gateway.arm.clone()
    };
    let path = arm.path();
    let target = arm.loopback_mount_target();
    let args = tailscale_enable_args(&path, &target);
    {
        let mut active = active_gateways().lock().await;
        let Some(gateway) = active.get_mut(&binding_generation) else {
            return Err("Forge Remote was stopped before Tailscale could enable its route.".into());
        };
        if gateway.arm.gateway_generation != arm.gateway_generation {
            return Err(
                "Forge Remote pairing changed before Tailscale could enable its route.".into(),
            );
        }
        mark_route_attempt(&mut gateway.arm);
    }
    let output = match run_command_with_timeout_owned("tailscale", &args).await {
        Ok(output) => output,
        Err(error) => {
            return Err(enable_failure_with_cleanup(
                binding_generation,
                &arm,
                format!("couldn't configure the private Tailscale route: {error}"),
            )
            .await);
        }
    };
    if !output.status.success() {
        let error = redact_secret(&bounded_output(&output.stderr), &arm.pairing_token);
        return Err(enable_failure_with_cleanup(
            binding_generation,
            &arm,
            format!(
                "Tailscale did not confirm the private Forge Remote route: {}",
                error.trim()
            ),
        )
        .await);
    }
    let host = dns_name.trim_end_matches('.');
    let remote_url = format!("https://{host}{}/", arm.path());
    let mut active = active_gateways().lock().await;
    let Some(gateway) = active.get_mut(&binding_generation) else {
        let _ = disable_tailscale_path(&arm).await;
        return Err("Forge Remote was stopped while Tailscale was enabling its route.".into());
    };
    if gateway.arm.gateway_generation != arm.gateway_generation {
        let _ = disable_tailscale_path(&arm).await;
        return Err("Forge Remote pairing changed while Tailscale was enabling its route.".into());
    }
    gateway.arm.remote_url = Some(remote_url);
    mark_route_attempt(&mut gateway.arm);
    Ok(gateway.arm.clone())
}

/// Stops one exact pager binding. Other Forge Remote sessions in this process
/// retain their listeners, browser connections, and Tailscale Serve paths.
pub async fn stop_gateway_binding_checked(
    binding_generation: u64,
    reason: RemoteRevocationReason,
) -> Result<bool, String> {
    let _operation = gateway_operations().lock().await;
    stop_gateway_binding_locked(binding_generation, reason).await
}

/// Stops only the pairing identity captured by the caller. This is safe for
/// stale asynchronous effects because it cannot revoke a newer generation.
pub async fn stop_gateway_generation_checked(
    binding_generation: u64,
    generation: u64,
    reason: RemoteRevocationReason,
) -> Result<bool, String> {
    stop_matching_gateway(binding_generation, generation, reason).await
}

/// Stops every process-local Forge Remote binding. This is reserved for pager
/// shutdown; session-scoped `/rc stop` must use `stop_gateway_binding_checked`.
pub async fn stop_all_gateways_checked(reason: RemoteRevocationReason) -> Result<bool, String> {
    let _operation = gateway_operations().lock().await;
    let gateways = active_gateways()
        .lock()
        .await
        .drain()
        .map(|(_, gateway)| gateway)
        .collect::<Vec<_>>();
    let pending = pending_route_cleanups()
        .lock()
        .await
        .drain()
        .map(|(_, arm)| arm)
        .collect::<Vec<_>>();
    let stopped = !gateways.is_empty() || !pending.is_empty();
    let mut errors = Vec::new();
    for gateway in gateways {
        let arm = gateway.arm.clone();
        gateway.shutdown(reason).await;
        if let Err(error) = cleanup_route_or_retain(&arm).await {
            errors.push(error);
        }
    }
    for arm in pending {
        if let Err(error) = cleanup_route_or_retain(&arm).await {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(stopped)
    } else {
        Err(errors.join(" "))
    }
}

async fn stop_gateway_binding_locked(
    binding_generation: u64,
    reason: RemoteRevocationReason,
) -> Result<bool, String> {
    if let Some(gateway) = active_gateways().lock().await.remove(&binding_generation) {
        let arm = gateway.arm.clone();
        gateway.shutdown(reason).await;
        cleanup_route_or_retain(&arm).await?;
        return Ok(true);
    }
    let pending = pending_route_cleanups()
        .lock()
        .await
        .remove(&binding_generation);
    let Some(arm) = pending else {
        return Ok(false);
    };
    cleanup_route_or_retain(&arm).await?;
    Ok(true)
}

async fn stop_matching_gateway(
    binding_generation: u64,
    generation: u64,
    reason: RemoteRevocationReason,
) -> Result<bool, String> {
    let _operation = gateway_operations().lock().await;
    let gateway = {
        let mut active = active_gateways().lock().await;
        if !gateway_generation_matches(active.get(&binding_generation), generation) {
            return Ok(false);
        }
        active
            .remove(&binding_generation)
            .expect("gateway checked above")
    };
    let arm = gateway.arm.clone();
    gateway.shutdown(reason).await;
    cleanup_route_or_retain(&arm).await.map(|()| true)
}

fn gateway_generation_matches(active: Option<&RemoteGateway>, generation: u64) -> bool {
    active.is_some_and(|gateway| gateway.arm.gateway_generation == generation)
}

async fn cleanup_route_or_retain(arm: &RemoteArm) -> Result<(), String> {
    match disable_private_tailscale_route(arm).await {
        Ok(()) => {
            let mut pending = pending_route_cleanups().lock().await;
            if pending
                .get(&arm.binding_generation)
                .is_some_and(|stored| stored.gateway_generation == arm.gateway_generation)
            {
                pending.remove(&arm.binding_generation);
            }
            Ok(())
        }
        Err(error) => {
            pending_route_cleanups()
                .lock()
                .await
                .insert(arm.binding_generation, arm.clone());
            Err(error)
        }
    }
}

async fn enable_failure_with_cleanup(
    binding_generation: u64,
    arm: &RemoteArm,
    original: String,
) -> String {
    match disable_tailscale_path(arm).await {
        Ok(()) => {
            clear_route_attempt(binding_generation, arm.gateway_generation).await;
            format!("{original}. Forge removed the exact attempted route as a precaution.")
        }
        Err(cleanup) => {
            let mut tombstone = arm.clone();
            mark_route_attempt(&mut tombstone);
            pending_route_cleanups()
                .lock()
                .await
                .insert(binding_generation, tombstone);
            format!("{original}. Precautionary cleanup also failed: {cleanup}")
        }
    }
}

async fn clear_route_attempt(binding_generation: u64, generation: u64) {
    if let Some(gateway) = active_gateways().lock().await.get_mut(&binding_generation)
        && gateway.arm.gateway_generation == generation
    {
        gateway.arm.route_may_exist = false;
    }
}

fn mark_route_attempt(arm: &mut RemoteArm) {
    arm.route_may_exist = true;
}

async fn retry_pending_route_cleanup(binding_generation: u64) -> Result<(), String> {
    let pending = pending_route_cleanups()
        .lock()
        .await
        .remove(&binding_generation);
    let Some(arm) = pending else {
        return Ok(());
    };
    cleanup_route_or_retain(&arm).await
}

async fn disable_private_tailscale_route(arm: &RemoteArm) -> Result<(), String> {
    if !arm.route_may_exist {
        return Ok(());
    }
    disable_tailscale_path(arm).await
}

async fn disable_tailscale_path(arm: &RemoteArm) -> Result<(), String> {
    let args = tailscale_disable_args(&arm.path());
    let output = run_command_with_timeout_owned("tailscale", &args)
        .await
        .map_err(|error| format!("couldn't remove Forge Remote's private route: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let error = redact_secret(&bounded_output(&output.stderr), &arm.pairing_token);
    Err(format!(
        "Forge Remote stopped locally, but Tailscale could not remove its exact private route: {}. Retry `/rc stop` or remove only the Forge path shown by `tailscale serve status`.",
        error.trim()
    ))
}

fn tailscale_enable_args(path: &str, target: &str) -> Vec<String> {
    vec![
        "serve".into(),
        "--bg".into(),
        "--https=443".into(),
        "--set-path".into(),
        path.into(),
        target.into(),
    ]
}

fn tailscale_disable_args(path: &str) -> Vec<String> {
    vec![
        "serve".into(),
        "--https=443".into(),
        "--set-path".into(),
        path.into(),
        "off".into(),
    ]
}

#[derive(Debug)]
enum CommandFailure {
    Missing,
    TimedOut,
    Io(std::io::Error),
}

impl fmt::Display for CommandFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => formatter.write_str("the Tailscale CLI was not found"),
            Self::TimedOut => formatter.write_str("the Tailscale command timed out"),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

async fn run_command_with_timeout(program: &str, args: &[&str]) -> Result<Output, CommandFailure> {
    let owned = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    run_command_with_timeout_owned(program, &owned).await
}

async fn run_command_with_timeout_owned(
    program: &str,
    args: &[String],
) -> Result<Output, CommandFailure> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true);
    match tokio::time::timeout(TAILSCALE_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(CommandFailure::Missing)
        }
        Ok(Err(error)) => Err(CommandFailure::Io(error)),
        Err(_) => Err(CommandFailure::TimedOut),
    }
}

fn gateway_router(state: GatewayState) -> Router {
    Router::new()
        .route("/forge/{token}", get(remote_shell_redirect))
        .route("/forge/{token}/", get(remote_shell))
        .route("/forge/{token}/assets/app.js", get(remote_app_js))
        .route("/forge/{token}/assets/app.css", get(remote_app_css))
        .route(
            "/forge/{token}/assets/basier-square-regular.woff2",
            get(remote_basier_regular),
        )
        .route(
            "/forge/{token}/assets/basier-square-semibold.woff2",
            get(remote_basier_semibold),
        )
        .route("/forge/{token}/manifest.webmanifest", get(remote_manifest))
        .route("/forge/{token}/icon.svg", get(remote_icon))
        .route(
            "/forge/{token}/THIRD_PARTY_NOTICES.txt",
            get(remote_notices),
        )
        .route("/forge/{token}/events", get(remote_socket))
        .with_state(state)
}

async fn remote_shell_redirect(
    Path(token): Path<String>,
    State(state): State<GatewayState>,
) -> Response {
    if !state.accepts_token(&token) {
        return not_found();
    }
    let mut response = Redirect::temporary(&format!("/forge/{token}/")).into_response();
    apply_security_headers(&mut response, "text/plain; charset=utf-8", false);
    response
}

async fn remote_shell(Path(token): Path<String>, State(state): State<GatewayState>) -> Response {
    protected_asset(&token, &state, INDEX_HTML, "text/html; charset=utf-8", true)
}

async fn remote_app_js(Path(token): Path<String>, State(state): State<GatewayState>) -> Response {
    protected_asset(
        &token,
        &state,
        APP_JS,
        "text/javascript; charset=utf-8",
        false,
    )
}

async fn remote_app_css(Path(token): Path<String>, State(state): State<GatewayState>) -> Response {
    protected_asset(&token, &state, APP_CSS, "text/css; charset=utf-8", false)
}

async fn remote_basier_regular(
    Path(token): Path<String>,
    State(state): State<GatewayState>,
) -> Response {
    protected_binary_asset(&token, &state, BASIER_REGULAR_WOFF2, "font/woff2")
}

async fn remote_basier_semibold(
    Path(token): Path<String>,
    State(state): State<GatewayState>,
) -> Response {
    protected_binary_asset(&token, &state, BASIER_SEMIBOLD_WOFF2, "font/woff2")
}

async fn remote_manifest(Path(token): Path<String>, State(state): State<GatewayState>) -> Response {
    protected_asset(&token, &state, MANIFEST, "application/manifest+json", false)
}

async fn remote_icon(Path(token): Path<String>, State(state): State<GatewayState>) -> Response {
    protected_asset(&token, &state, ICON_SVG, "image/svg+xml", false)
}

async fn remote_notices(Path(token): Path<String>, State(state): State<GatewayState>) -> Response {
    protected_asset(
        &token,
        &state,
        THIRD_PARTY_NOTICES,
        "text/plain; charset=utf-8",
        false,
    )
}

fn protected_asset(
    token: &str,
    state: &GatewayState,
    body: &'static str,
    content_type: &'static str,
    html: bool,
) -> Response {
    if !state.accepts_token(token) {
        return not_found();
    }
    let mut response = body.into_response();
    apply_security_headers(&mut response, content_type, html);
    response
}

fn protected_binary_asset(
    token: &str,
    state: &GatewayState,
    body: &'static [u8],
    content_type: &'static str,
) -> Response {
    if !state.accepts_token(token) {
        return not_found();
    }
    let mut response = Response::new(axum::body::Body::from(body));
    apply_security_headers(&mut response, content_type, false);
    response
}

fn apply_security_headers(response: &mut Response, content_type: &'static str, html: bool) {
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(REMOTE_CSP),
    );
    if html {
        headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    }
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

async fn remote_socket(
    Path(token): Path<String>,
    State(state): State<GatewayState>,
    websocket: WebSocketUpgrade,
) -> Response {
    if !state.accepts_token(&token) {
        return not_found();
    }
    websocket
        .max_message_size(MAX_REMOTE_MESSAGE_BYTES)
        .max_frame_size(MAX_REMOTE_MESSAGE_BYTES)
        .on_upgrade(move |socket| remote_socket_session(socket, state))
}

async fn send_superseded_close(writer: &mut futures_util::stream::SplitSink<WebSocket, Message>) {
    let _ = writer
        .send(Message::Close(Some(axum::extract::ws::CloseFrame {
            code: REMOTE_CLIENT_SUPERSEDED_CLOSE_CODE,
            reason: REMOTE_CLIENT_SUPERSEDED_REASON.into(),
        })))
        .await;
}

async fn remote_socket_session(socket: WebSocket, state: GatewayState) {
    // Ownership changes only after the HTTP upgrade succeeds. A malformed or
    // abandoned upgrade can therefore never evict the currently active phone.
    let client = state.client_ownership.claim();
    let (mut writer, mut reader) = socket.split();
    let mut events = state.events.subscribe();
    let mut lifecycle = state.lifecycle.subscribe();
    if send_server_message(
        &mut writer,
        &ServerMessage::Connected {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            session_id: state.session_id.to_string(),
            expires_at: state.expires_at_rfc3339.to_string(),
        },
    )
    .await
    .is_err()
    {
        return;
    }
    let mut hello_received = false;
    let mut revision = 0_u64;
    let mut pending_session_acceptance: Option<(
        String,
        mpsc::UnboundedSender<RemoteSessionAcceptance>,
    )> = None;
    let expiry = tokio::time::sleep_until(tokio::time::Instant::from_std(state.expires_at));
    tokio::pin!(expiry);
    loop {
        tokio::select! {
            biased;
            _ = client.superseded.cancelled() => {
                send_superseded_close(&mut writer).await;
                break;
            }
            _ = &mut expiry => {
                let _ = send_server_message(&mut writer, &revoked(RemoteRevocationReason::Expired)).await;
                break;
            }
            reason = lifecycle.recv() => {
                let reason = reason.unwrap_or(RemoteRevocationReason::Stopped);
                let _ = send_server_message(&mut writer, &revoked(reason)).await;
                break;
            }
            event = events.recv(), if hello_received => match event {
                Ok(RemotePagerEvent::Delta { session_id, base_revision, revision: next, event }) if session_id == state.session_id.as_ref() => {
                    if !delta_payload_session_matches(&event, &state.session_id) {
                        let _ = send_server_message(&mut writer, &ServerMessage::Error {
                            protocol_version: REMOTE_PROTOCOL_VERSION,
                            error: RemoteError::new("sessionMismatch", "Forge rejected a remote update for a different session.", false),
                        }).await;
                        break;
                    }
                    // Deltas published before hello sit in the broadcast
                    // buffer. The snapshot already contains those revisions, so
                    // they must be skipped. Asking for a resync here stalls
                    // the live stream.
                    if next <= revision {
                        continue;
                    }
                    if base_revision != revision || next <= base_revision {
                        let _ = send_server_message(&mut writer, &ServerMessage::ResyncRequired {
                            protocol_version: REMOTE_PROTOCOL_VERSION,
                            reason: "Session revisions no longer form a continuous sequence.".into(),
                        }).await;
                        continue;
                    }
                    let message = ServerMessage::Delta {
                        protocol_version: REMOTE_PROTOCOL_VERSION,
                        base_revision,
                        revision: next,
                        event,
                    };
                    if send_server_message(&mut writer, &message).await.is_err() { break; }
                    revision = next;
                }
                Ok(RemotePagerEvent::CommandResult { session_id, client_generation, command_id, outcome })
                    if session_id == state.session_id.as_ref()
                        && client_generation == client.generation => {
                    if send_server_message(&mut writer, &ServerMessage::CommandResult {
                        protocol_version: REMOTE_PROTOCOL_VERSION,
                        command_id,
                        outcome,
                    }).await.is_err() { break; }
                }
                Ok(RemotePagerEvent::SessionCreated {
                    session_id,
                    client_generation,
                    command_id,
                    new_session_id,
                    pairing_url,
                    expires_at,
                    delivery_ack,
                }) if session_id == state.session_id.as_ref()
                    && client_generation == client.generation => {
                    let written = send_server_message(&mut writer, &ServerMessage::SessionCreated {
                        protocol_version: REMOTE_PROTOCOL_VERSION,
                        command_id,
                        session_id: new_session_id.clone(),
                        pairing_url,
                        expires_at,
                    }).await.is_ok();
                    if written {
                        if let Some((_, stale)) = pending_session_acceptance
                            .replace((new_session_id, delivery_ack))
                        {
                            let _ = stale.send(RemoteSessionAcceptance::Abort);
                        }
                    } else {
                        let _ = delivery_ack.send(RemoteSessionAcceptance::Abort);
                        break;
                    }
                }
                Ok(RemotePagerEvent::Error { session_id, error }) if session_id == state.session_id.as_ref() => {
                    if send_server_message(&mut writer, &ServerMessage::Error {
                        protocol_version: REMOTE_PROTOCOL_VERSION,
                        error,
                    }).await.is_err() { break; }
                }
                Ok(RemotePagerEvent::Revoked { session_id, reason }) if session_id == state.session_id.as_ref() => {
                    let _ = send_server_message(&mut writer, &revoked(reason)).await;
                    break;
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let _ = send_server_message(&mut writer, &ServerMessage::ResyncRequired {
                        protocol_version: REMOTE_PROTOCOL_VERSION,
                        reason: "The remote display fell behind the live session.".into(),
                    }).await;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    let _ = send_server_message(&mut writer, &revoked(RemoteRevocationReason::SessionClosed)).await;
                    break;
                }
            },
            incoming = reader.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    if !client.is_current() {
                        send_superseded_close(&mut writer).await;
                        break;
                    }
                    if text.len() > MAX_REMOTE_MESSAGE_BYTES {
                        let _ = send_protocol_error(&mut writer, "messageTooLarge", "Remote command exceeded the message-size limit.", false).await;
                        break;
                    }
                    let message = match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(message) => message,
                        Err(_) => {
                            let _ = send_protocol_error(&mut writer, "invalidMessage", "Remote command did not match protocol v1.", false).await;
                            continue;
                        }
                    };
                    match message {
                        ClientMessage::Hello { protocol_version } => {
                            if protocol_version != REMOTE_PROTOCOL_VERSION {
                                let _ = send_protocol_error(&mut writer, "protocolMismatch", "This Forge Remote page is incompatible with the running Forge version. Reopen the remote link.", false).await;
                                break;
                            }
                            match send_current_snapshot(&mut writer, &state).await {
                                Ok(Some(next)) => {
                                    revision = next;
                                    hello_received = true;
                                    notify_phone_ready(&state, client.generation);
                                }
                                Ok(None) => {
                                    let _ = send_protocol_error(&mut writer, "snapshotUnavailable", "Forge is still preparing the authoritative session snapshot. Retry hello shortly.", true).await;
                                }
                                Err(_) => break,
                            }
                        }
                        ClientMessage::Command { protocol_version, command_id, command } => {
                            if !hello_received && !matches!(command, RemoteCommand::Resync) {
                                let _ = send_protocol_error(&mut writer, "helloRequired", "Send protocol hello before remote commands.", false).await;
                                continue;
                            }
                            if protocol_version != REMOTE_PROTOCOL_VERSION {
                                let _ = send_command_error(&mut writer, command_id, "protocolMismatch", "The command protocol version is incompatible.", false).await;
                                continue;
                            }
                            if !valid_command_id(&command_id) || !valid_remote_command(&command) {
                                let _ = send_command_error(&mut writer, command_id, "invalidCommand", "The remote command contains invalid or oversized fields.", false).await;
                                continue;
                            }
                            match command {
                                RemoteCommand::AcceptNewSession { session_id } => {
                                    let accepted = pending_session_acceptance
                                        .as_ref()
                                        .is_some_and(|(pending, _)| pending == &session_id);
                                    if !accepted {
                                        let _ = send_command_error(&mut writer, command_id, "newSessionMismatch", "This socket has no matching provisional session to accept.", false).await;
                                        continue;
                                    }
                                    let (_, acceptance) = pending_session_acceptance
                                        .take()
                                        .expect("matching provisional session checked above");
                                    let (grant, granted) = tokio::sync::oneshot::channel();
                                    let began = client.with_current(|| {
                                        acceptance.send(RemoteSessionAcceptance::Begin {
                                            granted: grant,
                                        })
                                    });
                                    if !matches!(began, Some(Ok(()))) || granted.await.is_err() {
                                        let _ = acceptance.send(RemoteSessionAcceptance::Abort);
                                        let _ = send_command_error(&mut writer, command_id, "newSessionExpired", "The provisional session expired before it was accepted.", false).await;
                                        continue;
                                    }
                                    let result_written = tokio::time::timeout(
                                        SESSION_ACCEPT_RESULT_WRITE_TIMEOUT,
                                        send_server_message(&mut writer, &ServerMessage::CommandResult {
                                            protocol_version: REMOTE_PROTOCOL_VERSION,
                                            command_id,
                                            outcome: RemoteCommandOutcome::Ok,
                                        }),
                                    ).await.is_ok_and(|result| result.is_ok());
                                    if !result_written {
                                        let _ = acceptance.send(RemoteSessionAcceptance::Abort);
                                        break;
                                    }
                                    // The atomic Begin above is the ownership
                                    // linearization point. Once pager grants, this
                                    // exact requester has already persisted and
                                    // validated the child. A takeover racing the
                                    // bounded OK write must not turn a visible OK
                                    // into a revoked child.
                                    if acceptance.send(RemoteSessionAcceptance::Commit).is_err() {
                                        break;
                                    }
                                }
                                RemoteCommand::Ping => {
                                    if send_server_message(&mut writer, &ServerMessage::Pong {
                                        protocol_version: REMOTE_PROTOCOL_VERSION,
                                        command_id,
                                    }).await.is_err() { break; }
                                }
                                RemoteCommand::Resync => {
                                    match send_current_snapshot(&mut writer, &state).await {
                                        Ok(Some(next)) => {
                                            revision = next;
                                            hello_received = true;
                                            notify_phone_ready(&state, client.generation);
                                            let _ = send_server_message(&mut writer, &ServerMessage::CommandResult {
                                                protocol_version: REMOTE_PROTOCOL_VERSION,
                                                command_id,
                                                outcome: RemoteCommandOutcome::Ok,
                                            }).await;
                                        }
                                        Ok(None) => {
                                            let _ = send_command_error(&mut writer, command_id, "snapshotUnavailable", "Forge is still preparing the authoritative session snapshot. Retry shortly.", true).await;
                                        }
                                        Err(_) => break,
                                    }
                                }
                                command => {
                                    let request = RemoteCommandRequest {
                                        session_id: state.session_id.to_string(),
                                        gateway_generation: state.generation,
                                        client_generation: client.generation,
                                        command_id: command_id.clone(),
                                        command,
                                    };
                                    let delivery = client.with_current(|| state.commands.try_send(request));
                                    let Some(delivery) = delivery else {
                                        send_superseded_close(&mut writer).await;
                                        break;
                                    };
                                    match delivery {
                                        Ok(()) => {}
                                        Err(mpsc::error::TrySendError::Full(_)) => {
                                            let _ = send_command_error(&mut writer, command_id, "commandQueueFull", "Forge is still processing earlier remote input. Retry shortly.", true).await;
                                        }
                                        Err(mpsc::error::TrySendError::Closed(_)) => {
                                            let _ = send_command_error(&mut writer, command_id, "sessionClosed", "The Forge session connection closed.", false).await;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Ok(Message::Ping(payload))) => {
                    if writer.send(Message::Pong(payload)).await.is_err() { break; }
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(Message::Binary(_))) => {
                    let _ = send_protocol_error(&mut writer, "unsupportedFrame", "Forge Remote accepts text JSON messages only.", false).await;
                    break;
                }
                Some(Ok(Message::Pong(_))) => {}
            }
        }
    }
}

async fn send_current_snapshot(
    writer: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    state: &GatewayState,
) -> Result<Option<u64>, ()> {
    let snapshot = if let Some(snapshot) = state.current_snapshot() {
        Some(snapshot)
    } else {
        let mut snapshots = state.snapshots.clone();
        tokio::time::timeout(SNAPSHOT_WAIT_TIMEOUT, async {
            loop {
                if snapshots.changed().await.is_err() {
                    return None;
                }
                if let Some(snapshot) = snapshots.borrow().clone()
                    && snapshot.session_id == state.session_id.as_ref()
                    && payload_session_matches(&snapshot.session, &state.session_id)
                {
                    return Some(snapshot);
                }
            }
        })
        .await
        .ok()
        .flatten()
    };
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    let revision = snapshot.revision;
    send_server_message(
        writer,
        &ServerMessage::Snapshot {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            revision,
            session: snapshot.session,
        },
    )
    .await?;
    Ok(Some(revision))
}

async fn send_server_message(
    writer: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &ServerMessage,
) -> Result<(), ()> {
    let payload = serde_json::to_string(message).map_err(|_| ())?;
    writer
        .send(Message::Text(payload.into()))
        .await
        .map_err(|_| ())
}

async fn send_protocol_error(
    writer: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    code: &str,
    message: &str,
    retryable: bool,
) -> Result<(), ()> {
    send_server_message(
        writer,
        &ServerMessage::Error {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            error: RemoteError::new(code, message, retryable),
        },
    )
    .await
}

async fn send_command_error(
    writer: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    command_id: String,
    code: &str,
    message: &str,
    retryable: bool,
) -> Result<(), ()> {
    send_server_message(
        writer,
        &ServerMessage::CommandResult {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            command_id,
            outcome: RemoteCommandOutcome::Error {
                error: RemoteError::new(code, message, retryable),
            },
        },
    )
    .await
}

fn revoked(reason: RemoteRevocationReason) -> ServerMessage {
    ServerMessage::Revoked {
        protocol_version: REMOTE_PROTOCOL_VERSION,
        reason,
    }
}

fn notify_phone_ready(state: &GatewayState, client_generation: u64) {
    let _ = state.commands.try_send(RemoteCommandRequest {
        session_id: state.session_id.to_string(),
        gateway_generation: state.generation,
        client_generation,
        command_id: format!("phone-ready-{client_generation}"),
        command: RemoteCommand::PhoneReady,
    });
}

fn valid_command_id(command_id: &str) -> bool {
    !command_id.is_empty()
        && command_id.len() <= MAX_COMMAND_ID_BYTES
        && !command_id.chars().any(char::is_control)
}

fn valid_remote_prompt(text: &str, images: &[RemotePromptImage]) -> bool {
    if images.len() > MAX_PROMPT_IMAGES {
        return false;
    }
    if text.trim().is_empty() && images.is_empty() {
        return false;
    }
    if !text.is_empty() && (text.len() > MAX_PROMPT_BYTES || text.chars().any(char::is_control)) {
        return false;
    }
    images.iter().all(|image| {
        valid_nonempty(&image.name, MAX_SHORT_FIELD_BYTES)
            && valid_nonempty(&image.mime_type, MAX_SHORT_FIELD_BYTES)
            && image.mime_type.starts_with("image/")
            && !image.data.is_empty()
            && image.data.len() <= MAX_PROMPT_IMAGE_BYTES * 2
    })
}

fn valid_remote_command(command: &RemoteCommand) -> bool {
    match command {
        RemoteCommand::Prompt { text, images } => valid_remote_prompt(text, images),
        RemoteCommand::PhoneReady => true,
        RemoteCommand::NewSession {} => true,
        RemoteCommand::AcceptNewSession { session_id } => {
            valid_nonempty(session_id, MAX_SHORT_FIELD_BYTES)
        }
        RemoteCommand::EditQueuedPrompt {
            queue_item_id,
            text,
            ..
        } => {
            valid_nonempty(queue_item_id, MAX_SHORT_FIELD_BYTES)
                && valid_nonempty(text, MAX_PROMPT_BYTES)
        }
        RemoteCommand::SteerQueuedPrompt { queue_item_id, .. }
        | RemoteCommand::CancelQueuedPrompt { queue_item_id, .. } => {
            valid_nonempty(queue_item_id, MAX_SHORT_FIELD_BYTES)
        }
        RemoteCommand::SetModel {
            model_id,
            reasoning_effort,
        } => {
            valid_nonempty(model_id, MAX_SHORT_FIELD_BYTES)
                && reasoning_effort
                    .as_ref()
                    .is_none_or(|effort| valid_nonempty(effort, MAX_SHORT_FIELD_BYTES))
        }
        RemoteCommand::SetFastMode { .. } => true,
        RemoteCommand::Btw { question } => valid_nonempty(question, MAX_PROMPT_BYTES),
        RemoteCommand::ResolveInteraction {
            interaction_id,
            response,
        } => {
            valid_nonempty(interaction_id, MAX_SHORT_FIELD_BYTES)
                && valid_interaction_response(response)
        }
        RemoteCommand::Cancel
        | RemoteCommand::RefreshUsage
        | RemoteCommand::Resync
        | RemoteCommand::Ping => true,
    }
}

fn valid_interaction_response(response: &RemoteInteractionResponse) -> bool {
    match response {
        RemoteInteractionResponse::Permission { option_id } => {
            valid_nonempty(option_id, MAX_SHORT_FIELD_BYTES)
        }
        RemoteInteractionResponse::PermissionFollowup { text } => {
            valid_nonempty(text, MAX_PROMPT_BYTES)
        }
        RemoteInteractionResponse::Question { answers } => {
            !answers.is_empty()
                && answers.len() <= 32
                && answers.iter().all(|answer| {
                    answer.option_indices.len() <= 64
                        && answer
                            .freeform
                            .as_ref()
                            .is_none_or(|text| text.len() <= MAX_PROMPT_BYTES)
                })
        }
        RemoteInteractionResponse::Plan { feedback, .. } => feedback
            .as_ref()
            .is_none_or(|text| text.len() <= MAX_PROMPT_BYTES),
        RemoteInteractionResponse::Cancel => true,
    }
}

fn valid_nonempty(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.contains('\0')
}

fn payload_session_matches(payload: &serde_json::Value, expected: &str) -> bool {
    payload.get("sessionId").and_then(serde_json::Value::as_str) == Some(expected)
}

fn delta_payload_session_matches(event: &serde_json::Value, expected: &str) -> bool {
    match event.get("kind").and_then(serde_json::Value::as_str) {
        Some("stateReplaced") => event
            .get("session")
            .is_some_and(|session| payload_session_matches(session, expected)),
        Some("transcriptSpliced") => {
            event.get("sessionId").and_then(serde_json::Value::as_str) == Some(expected)
        }
        _ => false,
    }
}

fn constant_time_token_eq(candidate: &str, expected: &str) -> bool {
    let mut difference = candidate.len() ^ expected.len();
    for index in 0..PAIRING_BYTES * 2 {
        let left = candidate.as_bytes().get(index).copied().unwrap_or_default();
        let right = expected.as_bytes().get(index).copied().unwrap_or_default();
        difference |= usize::from(left ^ right);
    }
    difference == 0
}

fn hex_token(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn bounded_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_COMMAND_OUTPUT)]).into_owned()
}

fn redact_secret(message: &str, secret: &str) -> String {
    message.replace(secret, "[redacted]")
}

fn remote_qr_text(url: &str) -> Option<String> {
    let qr = QrCode::encode_text(url, QrCodeEcc::Medium).ok()?;
    let border = 4;
    let mut output = String::new();
    for y in (-border..qr.size() + border).step_by(2) {
        for x in -border..qr.size() + border {
            let upper = x >= 0 && y >= 0 && x < qr.size() && y < qr.size() && qr.get_module(x, y);
            let lower_y = y + 1;
            let lower = x >= 0
                && lower_y >= 0
                && x < qr.size()
                && lower_y < qr.size()
                && qr.get_module(x, lower_y);
            output.push(match (upper, lower) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        output.push('\n');
    }
    Some(output)
}

pub fn remote_pairing_notice(url: &str) -> String {
    let qr = remote_qr_text(url).unwrap_or_default();
    format!(
        "Forge Remote is live for this exact session until you run `/rc stop` (or for up to 8 hours). Scan the QR code below on your phone or open this private Tailscale link: {url}\n\n{qr}\nIt is not public and is not visible outside your tailnet."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio_tungstenite::tungstenite::Message as ClientWebSocketMessage;
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
    use tower::ServiceExt;

    type TestWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

    struct LiveGatewayFixture {
        address: SocketAddr,
        token: String,
        websocket_url: String,
        state: GatewayState,
        command_rx: mpsc::Receiver<RemoteCommandRequest>,
        event_tx: broadcast::Sender<RemotePagerEvent>,
        snapshot_tx: watch::Sender<Option<RemoteSnapshot>>,
        lifecycle_tx: broadcast::Sender<RemoteRevocationReason>,
        server_task: JoinHandle<()>,
    }

    impl LiveGatewayFixture {
        async fn start(revision: u64, marker: &str) -> Self {
            let token = "c".repeat(PAIRING_BYTES * 2);
            let session_id = "session-live";
            let (command_tx, command_rx) = mpsc::channel(8);
            let (event_tx, _) = broadcast::channel(8);
            let (snapshot_tx, snapshot_rx) = watch::channel(Some(RemoteSnapshot {
                session_id: session_id.into(),
                revision,
                session: live_snapshot(session_id, marker),
            }));
            let (lifecycle_tx, _) = broadcast::channel(4);
            let state = GatewayState {
                session_id: Arc::from(session_id),
                generation: 77,
                pairing_token: Arc::from(token.as_str()),
                expires_at: Instant::now() + Duration::from_secs(60),
                expires_at_rfc3339: Arc::from("2030-01-01T00:00:00Z"),
                client_ownership: RemoteClientOwnership::default(),
                commands: command_tx,
                events: event_tx.clone(),
                snapshots: snapshot_rx,
                lifecycle: lifecycle_tx.clone(),
            };
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let address = listener.local_addr().unwrap();
            let app = gateway_router(state.clone());
            let server_task = tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            Self {
                address,
                token: token.clone(),
                websocket_url: format!("ws://{address}/forge/{token}/events"),
                state,
                command_rx,
                event_tx,
                snapshot_tx,
                lifecycle_tx,
                server_task,
            }
        }

        async fn connect(&self) -> TestWebSocket {
            tokio_tungstenite::connect_async(&self.websocket_url)
                .await
                .unwrap()
                .0
        }

        async fn drain_phone_ready(&mut self) {
            while let Ok(Some(request)) =
                tokio::time::timeout(Duration::from_millis(50), self.command_rx.recv()).await
            {
                assert!(
                    matches!(request.command, RemoteCommand::PhoneReady),
                    "unexpected command while draining phone-ready: {:?}",
                    request.command
                );
            }
        }

        async fn recv_user_command(&mut self) -> RemoteCommandRequest {
            loop {
                let request = self
                    .command_rx
                    .recv()
                    .await
                    .expect("command channel closed");
                if !matches!(request.command, RemoteCommand::PhoneReady) {
                    return request;
                }
            }
        }

        fn publish_snapshot(&self, revision: u64, marker: &str) {
            self.snapshot_tx
                .send(Some(RemoteSnapshot {
                    session_id: self.state.session_id.to_string(),
                    revision,
                    session: live_snapshot(&self.state.session_id, marker),
                }))
                .unwrap();
        }
    }

    impl Drop for LiveGatewayFixture {
        fn drop(&mut self) {
            self.server_task.abort();
        }
    }

    fn live_snapshot(session_id: &str, marker: &str) -> serde_json::Value {
        serde_json::json!({
            "sessionId": session_id,
            "status": "idle",
            "transcript": [{"id":"entry-1","kind":"assistant","text":marker}],
            "availableModels": [],
            "activeInteractions": [],
            "capabilities": {"prompt":true}
        })
    }

    fn state_replaced(session_id: &str, marker: &str) -> serde_json::Value {
        serde_json::json!({
            "kind": "stateReplaced",
            "session": live_snapshot(session_id, marker)
        })
    }

    async fn send_client_json(socket: &mut TestWebSocket, value: serde_json::Value) {
        socket
            .send(ClientWebSocketMessage::Text(value.to_string().into()))
            .await
            .unwrap();
    }

    async fn receive_server_json(socket: &mut TestWebSocket) -> serde_json::Value {
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(2), socket.next())
                .await
                .expect("timed out waiting for the gateway")
                .expect("gateway closed before sending JSON")
                .expect("gateway websocket error");
            match frame {
                ClientWebSocketMessage::Text(text) => {
                    return serde_json::from_str(&text).unwrap();
                }
                ClientWebSocketMessage::Ping(payload) => socket
                    .send(ClientWebSocketMessage::Pong(payload))
                    .await
                    .unwrap(),
                other => panic!("expected gateway JSON, got {other:?}"),
            }
        }
    }

    async fn send_hello(socket: &mut TestWebSocket) {
        send_client_json(
            socket,
            serde_json::json!({"type":"hello","protocolVersion":REMOTE_PROTOCOL_VERSION}),
        )
        .await;
    }

    async fn wait_for_client_owner(state: &GatewayState, expected: Option<u64>) {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if state.client_ownership.current_generation() == expected {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("gateway did not reach the expected client owner");
    }

    fn test_state(token: &str, expires_at: Instant) -> GatewayState {
        test_state_with_snapshot(token, expires_at, None)
    }

    fn test_state_with_snapshot(
        token: &str,
        expires_at: Instant,
        snapshot: Option<RemoteSnapshot>,
    ) -> GatewayState {
        let (commands, _) = mpsc::channel(4);
        let (events, _) = broadcast::channel(4);
        let (_, snapshots) = watch::channel(snapshot);
        let (lifecycle, _) = broadcast::channel(4);
        GatewayState {
            session_id: Arc::from("session-1"),
            generation: 7,
            pairing_token: Arc::from(token),
            expires_at,
            expires_at_rfc3339: Arc::from("2030-01-01T00:00:00Z"),
            client_ownership: RemoteClientOwnership::default(),
            commands,
            events,
            snapshots,
            lifecycle,
        }
    }

    fn gateway_registry_test_lock() -> &'static Mutex<()> {
        static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn registry_transport(binding_generation: u64, session_id: &str) -> RemoteTransport {
        let (commands, _) = mpsc::channel(8);
        let (events, _) = broadcast::channel(8);
        let (_, snapshots) = watch::channel(Some(RemoteSnapshot {
            session_id: session_id.into(),
            revision: 1,
            session: live_snapshot(session_id, "registry"),
        }));
        RemoteTransport {
            binding_generation,
            commands,
            events,
            snapshots,
        }
    }

    #[test]
    fn tailscale_status_requires_running_dns_and_identity() {
        assert!(matches!(
            parse_tailscale_status(
                r#"{"BackendState":"Running","MagicDNSSuffix":"tail.ts.net","Self":{"DNSName":"mac.tail.ts.net."}}"#
            ),
            TailscalePrerequisite::Ready { .. }
        ));
        assert_eq!(
            parse_tailscale_status(r#"{"BackendState":"Stopped"}"#),
            TailscalePrerequisite::NotRunning
        );
        assert_eq!(
            parse_tailscale_status(r#"{"BackendState":"Running"}"#),
            TailscalePrerequisite::NotSignedIn
        );
    }

    #[test]
    fn protocol_uses_exact_camel_case_wire_shape() {
        let command = serde_json::from_str::<ClientMessage>(
            r#"{"type":"command","protocolVersion":1,"commandId":"c1","command":{"type":"setModel","modelId":"grok-4","reasoningEffort":"high"}}"#,
        )
        .unwrap();
        assert!(matches!(
            command,
            ClientMessage::Command {
                command: RemoteCommand::SetModel { .. },
                ..
            }
        ));
        let server = serde_json::to_value(ServerMessage::Delta {
            protocol_version: 1,
            base_revision: 4,
            revision: 5,
            event: serde_json::json!({"kind":"stateReplaced"}),
        })
        .unwrap();
        assert_eq!(server["type"], "delta");
        assert_eq!(server["protocolVersion"], 1);
        assert_eq!(server["baseRevision"], 4);
    }

    #[test]
    fn work_disclosure_uses_the_exact_additive_camel_case_shape() {
        let disclosure = RemoteWorkDisclosure {
            duration_ms: 1_250,
            final_response_item_id: Some("response-9".into()),
            work_item_ids: vec!["reasoning-7".into(), "tool-8".into()],
        };
        let expected = serde_json::json!({
            "durationMs": 1250,
            "finalResponseItemId": "response-9",
            "workItemIds": ["reasoning-7", "tool-8"]
        });

        assert_eq!(serde_json::to_value(&disclosure).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<RemoteWorkDisclosure>(expected).unwrap(),
            disclosure
        );
    }

    #[tokio::test]
    async fn snapshot_waits_for_authoritative_watch_update() {
        let token = "a".repeat(PAIRING_BYTES * 2);
        let (snapshot_tx, snapshot_rx) = watch::channel(None);
        let mut state = test_state(&token, Instant::now() + Duration::from_secs(60));
        state.snapshots = snapshot_rx;
        let session_id = state.session_id.to_string();
        let waiter = tokio::spawn(async move {
            let mut snapshots = state.snapshots.clone();
            loop {
                snapshots.changed().await.unwrap();
                if let Some(snapshot) = snapshots.borrow().clone()
                    && snapshot.session_id == state.session_id.as_ref()
                    && payload_session_matches(&snapshot.session, &state.session_id)
                {
                    return snapshot;
                }
            }
        });
        snapshot_tx
            .send(Some(RemoteSnapshot {
                session_id: session_id.clone(),
                revision: 9,
                session: serde_json::json!({"sessionId": session_id}),
            }))
            .unwrap();
        assert_eq!(waiter.await.unwrap().revision, 9);
    }

    #[test]
    fn snapshot_rejects_cross_session_payload_even_if_transport_label_matches() {
        let token = "a".repeat(PAIRING_BYTES * 2);
        let state = test_state_with_snapshot(
            &token,
            Instant::now() + Duration::from_secs(60),
            Some(RemoteSnapshot {
                session_id: "session-1".into(),
                revision: 1,
                session: serde_json::json!({"sessionId":"session-2"}),
            }),
        );
        assert!(state.current_snapshot().is_none());
    }

    #[test]
    fn delta_rejects_cross_session_payloads() {
        assert!(delta_payload_session_matches(
            &serde_json::json!({
                "kind":"stateReplaced",
                "session":{"sessionId":"session-1"}
            }),
            "session-1"
        ));
        assert!(!delta_payload_session_matches(
            &serde_json::json!({
                "kind":"stateReplaced",
                "session":{"sessionId":"session-2"}
            }),
            "session-1"
        ));
        assert!(delta_payload_session_matches(
            &serde_json::json!({
                "kind":"transcriptSpliced",
                "sessionId":"session-1",
                "start":1,
                "deleteCount":1,
                "items":[]
            }),
            "session-1"
        ));
        assert!(!delta_payload_session_matches(
            &serde_json::json!({
                "kind":"transcriptSpliced",
                "sessionId":"session-2",
                "start":1,
                "deleteCount":1,
                "items":[]
            }),
            "session-1"
        ));
        assert!(!delta_payload_session_matches(
            &serde_json::json!({
                "kind":"transcriptSpliced",
                "start":1,
                "deleteCount":1,
                "items":[]
            }),
            "session-1"
        ));
        assert!(!delta_payload_session_matches(
            &serde_json::json!({"kind":"unknown"}),
            "session-1"
        ));
    }

    #[test]
    fn stale_gateway_generation_never_matches_current_owner() {
        assert!(!gateway_generation_matches(None, 4));
    }

    #[test]
    fn gateway_reuse_requires_same_session_and_same_transport_binding() {
        assert!(same_remote_binding("session-1", 7, "session-1", 7));
        assert!(!same_remote_binding("session-1", 7, "session-1", 8));
        assert!(!same_remote_binding("session-1", 7, "session-2", 7));
    }

    #[tokio::test]
    async fn gateway_registry_stops_expires_and_shuts_down_only_intended_bindings() {
        let _test = gateway_registry_test_lock().lock().await;
        let _ = stop_all_gateways_checked(RemoteRevocationReason::Stopped).await;

        let first = arm_active_gateway(
            "duplicate-session".into(),
            registry_transport(9_001, "duplicate-session"),
        )
        .await
        .unwrap();
        let second = arm_active_gateway(
            "duplicate-session".into(),
            registry_transport(9_002, "duplicate-session"),
        )
        .await
        .unwrap();
        let second_again = arm_active_gateway(
            "duplicate-session".into(),
            registry_transport(9_002, "duplicate-session"),
        )
        .await
        .unwrap();
        assert_ne!(first.binding_generation, second.binding_generation);
        assert_ne!(first.gateway_generation, second.gateway_generation);
        assert_ne!(first.path(), second.path());
        assert_ne!(first.local_url, second.local_url);
        assert_eq!(second_again.path(), second.path());
        assert_eq!(second_again.gateway_generation, second.gateway_generation);
        assert_eq!(active_gateways().lock().await.len(), 2);

        assert!(
            stop_gateway_binding_checked(9_001, RemoteRevocationReason::Stopped)
                .await
                .unwrap()
        );
        assert!(active_gateway_arm(9_001).await.is_none());
        assert_eq!(
            active_gateway_arm(9_002).await.unwrap().gateway_generation,
            second.gateway_generation
        );

        let expiring = arm_active_gateway(
            "third-session".into(),
            registry_transport(9_003, "third-session"),
        )
        .await
        .unwrap();
        assert!(
            stop_gateway_generation_checked(
                9_003,
                expiring.gateway_generation,
                RemoteRevocationReason::Expired,
            )
            .await
            .unwrap()
        );
        assert!(active_gateway_arm(9_003).await.is_none());
        assert!(active_gateway_arm(9_002).await.is_some());

        assert!(
            stop_all_gateways_checked(RemoteRevocationReason::Stopped)
                .await
                .unwrap()
        );
        assert!(active_gateways().lock().await.is_empty());
        assert!(pending_route_cleanups().lock().await.is_empty());
    }

    #[test]
    fn invalid_or_snake_case_commands_do_not_parse() {
        assert!(serde_json::from_str::<ClientMessage>(
            r#"{"type":"command","protocol_version":1,"command_id":"c1","command":{"type":"cancel"}}"#
        )
        .is_err());
        assert!(!valid_remote_command(&RemoteCommand::Prompt {
            text: " ".into(),
            images: Vec::new(),
        }));
        assert!(!valid_command_id(""));
    }

    #[test]
    fn refresh_usage_uses_the_additive_camel_case_command_contract() {
        let command: RemoteCommand =
            serde_json::from_value(serde_json::json!({"type": "refreshUsage"})).unwrap();
        assert_eq!(command, RemoteCommand::RefreshUsage);
        assert!(valid_remote_command(&command));
        assert_eq!(
            serde_json::to_value(command).unwrap(),
            serde_json::json!({"type": "refreshUsage"})
        );
    }

    #[test]
    fn new_session_has_no_client_supplied_path() {
        let command: RemoteCommand = serde_json::from_value(serde_json::json!({
            "type": "newSession"
        }))
        .unwrap();
        assert_eq!(command, RemoteCommand::NewSession {});
        assert!(valid_remote_command(&command));
        assert_eq!(
            serde_json::to_value(command).unwrap(),
            serde_json::json!({"type": "newSession"})
        );
        assert!(
            serde_json::from_value::<RemoteCommand>(serde_json::json!({
                "type": "newSession",
                "cwd": "/tmp/attacker-choice"
            }))
            .is_err()
        );
    }

    #[test]
    fn set_fast_mode_uses_the_exact_camel_case_command_contract() {
        let command: RemoteCommand = serde_json::from_value(serde_json::json!({
            "type": "setFastMode",
            "enabled": true
        }))
        .unwrap();
        assert_eq!(command, RemoteCommand::SetFastMode { enabled: true });
        assert!(valid_remote_command(&command));
        assert_eq!(
            serde_json::to_value(command).unwrap(),
            serde_json::json!({"type": "setFastMode", "enabled": true})
        );
    }

    #[test]
    fn queued_prompt_controls_use_versioned_camel_case_contracts() {
        let edit = RemoteCommand::EditQueuedPrompt {
            queue_item_id: "prompt-7".into(),
            expected_version: 3,
            text: "edited follow-up".into(),
        };
        let steer = RemoteCommand::SteerQueuedPrompt {
            queue_item_id: "prompt-7".into(),
            expected_version: 3,
        };
        let cancel = RemoteCommand::CancelQueuedPrompt {
            queue_item_id: "prompt-7".into(),
            expected_version: 3,
        };

        assert_eq!(
            serde_json::to_value(&edit).unwrap(),
            serde_json::json!({
                "type": "editQueuedPrompt",
                "queueItemId": "prompt-7",
                "expectedVersion": 3,
                "text": "edited follow-up"
            })
        );
        assert_eq!(
            serde_json::to_value(&steer).unwrap(),
            serde_json::json!({
                "type": "steerQueuedPrompt",
                "queueItemId": "prompt-7",
                "expectedVersion": 3
            })
        );
        assert_eq!(
            serde_json::to_value(&cancel).unwrap(),
            serde_json::json!({
                "type": "cancelQueuedPrompt",
                "queueItemId": "prompt-7",
                "expectedVersion": 3
            })
        );
        assert!(valid_remote_command(&edit));
        assert!(valid_remote_command(&steer));
        assert!(valid_remote_command(&cancel));
        assert!(!valid_remote_command(&RemoteCommand::EditQueuedPrompt {
            queue_item_id: "prompt-7".into(),
            expected_version: 3,
            text: "   ".into(),
        }));
    }

    #[test]
    fn secret_is_fixed_length_redacted_and_not_derived_from_session() {
        let token = hex_token(&[0xab; PAIRING_BYTES]);
        assert_eq!(token.len(), PAIRING_BYTES * 2);
        assert!(constant_time_token_eq(&token, &token));
        assert!(!constant_time_token_eq(&token, "short"));
        let arm = RemoteArm {
            binding_generation: 1,
            session_id: "session-not-in-url".into(),
            gateway_generation: 1,
            pairing_token: token.clone(),
            local_url: "http://127.0.0.1:1".into(),
            remote_url: None,
            route_may_exist: false,
            expires_at: Instant::now() + Duration::from_secs(1),
            expires_at_rfc3339: String::new(),
        };
        assert!(!format!("{arm:?}").contains(&token));
        assert!(!arm.path().contains(&arm.session_id));
        assert_eq!(
            redact_secret(&format!("failed {token}"), &token),
            "failed [redacted]"
        );
    }

    #[test]
    fn mount_target_reintroduces_prefix_stripped_by_tailscale() {
        let arm = RemoteArm {
            binding_generation: 1,
            session_id: "s".into(),
            gateway_generation: 1,
            pairing_token: "a".repeat(PAIRING_BYTES * 2),
            local_url: "http://127.0.0.1:4321".into(),
            remote_url: None,
            route_may_exist: false,
            expires_at: Instant::now() + Duration::from_secs(1),
            expires_at_rfc3339: String::new(),
        };
        assert_eq!(
            arm.loopback_mount_target(),
            format!("http://127.0.0.1:4321{}/", arm.path())
        );
    }

    #[test]
    fn attempted_enable_is_cleanup_owned_before_success_is_known() {
        let mut arm = RemoteArm {
            binding_generation: 1,
            session_id: "s".into(),
            gateway_generation: 1,
            pairing_token: "a".repeat(PAIRING_BYTES * 2),
            local_url: "http://127.0.0.1:4321".into(),
            remote_url: None,
            route_may_exist: false,
            expires_at: Instant::now() + Duration::from_secs(1),
            expires_at_rfc3339: String::new(),
        };
        assert!(!arm.route_may_exist);
        mark_route_attempt(&mut arm);
        assert!(arm.route_may_exist);
        assert_eq!(
            tailscale_disable_args(&arm.path())
                .last()
                .map(String::as_str),
            Some("off")
        );
    }

    #[test]
    fn tailscale_commands_touch_only_exact_private_path() {
        let path = "/forge/abcdef";
        let target = "http://127.0.0.1:4321/forge/abcdef/";
        assert_eq!(
            tailscale_enable_args(path, target),
            ["serve", "--bg", "--https=443", "--set-path", path, target]
        );
        assert_eq!(
            tailscale_disable_args(path),
            ["serve", "--https=443", "--set-path", path, "off"]
        );
        let joined = tailscale_enable_args(path, target).join(" ");
        assert!(!joined.contains("funnel"));
        assert!(!joined.contains("reset"));
        assert!(!joined.contains("clear"));
    }

    #[tokio::test]
    async fn every_asset_requires_secret_and_uses_strict_headers() {
        let token = "a".repeat(PAIRING_BYTES * 2);
        let app = gateway_router(test_state(&token, Instant::now() + Duration::from_secs(60)));
        for suffix in [
            "/",
            "/assets/app.js",
            "/assets/app.css",
            "/assets/basier-square-regular.woff2",
            "/assets/basier-square-semibold.woff2",
            "/manifest.webmanifest",
            "/icon.svg",
            "/THIRD_PARTY_NOTICES.txt",
        ] {
            let wrong = app
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri(format!("/forge/wrong{suffix}"))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(wrong.status(), StatusCode::NOT_FOUND);
            let response = app
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri(format!("/forge/{token}{suffix}"))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "failed {suffix}");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            let csp = response.headers()[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap();
            assert!(!csp.contains("unsafe-inline"));
            assert!(!csp.contains("unsafe-eval"));
        }
    }

    #[tokio::test]
    async fn authenticated_font_assets_are_exact_woff2_with_security_headers() {
        let token = "a".repeat(PAIRING_BYTES * 2);
        let app = gateway_router(test_state(&token, Instant::now() + Duration::from_secs(60)));
        for (name, expected) in [
            ("basier-square-regular.woff2", BASIER_REGULAR_WOFF2),
            ("basier-square-semibold.woff2", BASIER_SEMIBOLD_WOFF2),
        ] {
            let response = app
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri(format!("/forge/{token}/assets/{name}"))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CONTENT_TYPE], "font/woff2");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(
                response.headers()[header::X_CONTENT_TYPE_OPTIONS],
                "nosniff"
            );
            assert_eq!(
                response.headers()[header::CONTENT_SECURITY_POLICY],
                REMOTE_CSP
            );
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(body.as_ref(), expected);
        }
    }

    #[tokio::test]
    async fn canonical_redirect_does_not_cache_or_refer_the_bearer_path() {
        let token = "a".repeat(PAIRING_BYTES * 2);
        let app = gateway_router(test_state(&token, Instant::now() + Duration::from_secs(60)));
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/forge/{token}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(response.status().is_redirection());
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
        assert_eq!(
            response.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
        assert_eq!(
            response.headers()[header::CONTENT_SECURITY_POLICY],
            REMOTE_CSP
        );
    }

    #[test]
    fn stale_client_release_cannot_clear_the_new_owner() {
        let ownership = RemoteClientOwnership::default();
        let first = ownership.claim();
        let first_generation = first.generation;
        let second = ownership.claim();
        let second_generation = second.generation;

        assert_ne!(first_generation, second_generation);
        assert!(first.superseded.is_cancelled());
        assert_eq!(ownership.current_generation(), Some(second_generation));

        drop(first);
        assert_eq!(ownership.current_generation(), Some(second_generation));
        drop(second);
        assert_eq!(ownership.current_generation(), None);
    }

    #[tokio::test]
    async fn failed_accept_result_write_rejects_and_clears_provisional_session() {
        let (completion, mut result) = mpsc::unbounded_channel();
        completion.send(RemoteSessionAcceptance::Abort).unwrap();
        assert!(matches!(
            result.recv().await,
            Some(RemoteSessionAcceptance::Abort)
        ));
    }

    #[tokio::test]
    async fn expired_secret_is_indistinguishable_from_wrong_secret() {
        let token = "a".repeat(PAIRING_BYTES * 2);
        let app = gateway_router(test_state(&token, Instant::now() - Duration::from_secs(1)));
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/forge/{token}/"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn remote_websocket_round_trips_snapshot_command_delta_reconnect_and_stop() {
        let mut gateway = LiveGatewayFixture::start(4, "authoritative-before-connect").await;
        let mut socket = gateway.connect().await;

        let connected = receive_server_json(&mut socket).await;
        assert_eq!(connected["type"], "connected");
        assert_eq!(connected["protocolVersion"], REMOTE_PROTOCOL_VERSION);
        assert_eq!(connected["sessionId"], "session-live");

        send_hello(&mut socket).await;
        let ready = tokio::time::timeout(Duration::from_secs(2), gateway.command_rx.recv())
            .await
            .expect("hello did not notify the pager")
            .expect("command channel closed");
        assert!(matches!(ready.command, RemoteCommand::PhoneReady));
        let snapshot = receive_server_json(&mut socket).await;
        assert_eq!(snapshot["type"], "snapshot");
        assert_eq!(snapshot["revision"], 4);
        assert_eq!(snapshot["session"]["sessionId"], "session-live");
        assert_eq!(
            snapshot["session"]["transcript"][0]["text"],
            "authoritative-before-connect"
        );

        send_client_json(
            &mut socket,
            serde_json::json!({
                "type":"command",
                "protocolVersion":REMOTE_PROTOCOL_VERSION,
                "commandId":"prompt-1",
                "command":{"type":"prompt","text":"continue from my phone"}
            }),
        )
        .await;
        let request = tokio::time::timeout(Duration::from_secs(2), gateway.recv_user_command())
            .await
            .expect("gateway did not deliver the command");
        assert_eq!(request.session_id, "session-live");
        assert_eq!(request.gateway_generation, 77);
        assert_eq!(request.command_id, "prompt-1");
        assert_eq!(
            request.command,
            RemoteCommand::Prompt {
                text: "continue from my phone".into(),
                images: Vec::new(),
            }
        );

        gateway
            .event_tx
            .send(RemotePagerEvent::CommandResult {
                session_id: "session-live".into(),
                client_generation: request.client_generation,
                command_id: "prompt-1".into(),
                outcome: RemoteCommandOutcome::Ok,
            })
            .unwrap();
        let command_result = receive_server_json(&mut socket).await;
        assert_eq!(command_result["type"], "commandResult");
        assert_eq!(command_result["commandId"], "prompt-1");
        assert_eq!(command_result["outcome"]["status"], "ok");

        let (delivery_ack, mut delivery_result) = mpsc::unbounded_channel();
        gateway
            .event_tx
            .send(RemotePagerEvent::SessionCreated {
                session_id: "session-live".into(),
                client_generation: request.client_generation,
                command_id: "new-1".into(),
                new_session_id: "session-child".into(),
                pairing_url: "https://device.tail.example/forge/fresh/".into(),
                expires_at: "2030-01-02T03:04:05Z".into(),
                delivery_ack,
            })
            .unwrap();
        let created = receive_server_json(&mut socket).await;
        assert_eq!(created["type"], "sessionCreated");
        assert_eq!(created["commandId"], "new-1");
        assert_eq!(created["sessionId"], "session-child");
        assert_eq!(
            created["pairingUrl"],
            "https://device.tail.example/forge/fresh/"
        );
        assert_eq!(created["expiresAt"], "2030-01-02T03:04:05Z");
        assert!(
            tokio::time::timeout(Duration::from_millis(50), delivery_result.recv())
                .await
                .is_err(),
            "writing sessionCreated must not retain the child before app acceptance"
        );
        send_client_json(
            &mut socket,
            serde_json::json!({
                "type":"command",
                "protocolVersion":REMOTE_PROTOCOL_VERSION,
                "commandId":"accept-wrong",
                "command":{"type":"acceptNewSession","sessionId":"different-child"}
            }),
        )
        .await;
        let wrong = receive_server_json(&mut socket).await;
        assert_eq!(wrong["type"], "commandResult");
        assert_eq!(wrong["outcome"]["error"]["code"], "newSessionMismatch");
        send_client_json(
            &mut socket,
            serde_json::json!({
                "type":"command",
                "protocolVersion":REMOTE_PROTOCOL_VERSION,
                "commandId":"accept-child",
                "command":{"type":"acceptNewSession","sessionId":"session-child"}
            }),
        )
        .await;
        let begin = tokio::time::timeout(Duration::from_secs(1), delivery_result.recv())
            .await
            .expect("application acceptance begin timed out")
            .expect("application acceptance channel closed");
        let RemoteSessionAcceptance::Begin { granted } = begin else {
            panic!("expected acceptance begin")
        };
        granted.send(()).unwrap();
        let accepted = receive_server_json(&mut socket).await;
        assert_eq!(accepted["type"], "commandResult");
        assert_eq!(accepted["commandId"], "accept-child");
        assert_eq!(accepted["outcome"]["status"], "ok");
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), delivery_result.recv())
                .await
                .expect("application acceptance commit timed out"),
            Some(RemoteSessionAcceptance::Commit)
        ));

        gateway.publish_snapshot(5, "streamed-after-command");
        gateway
            .event_tx
            .send(RemotePagerEvent::Delta {
                session_id: "session-live".into(),
                base_revision: 4,
                revision: 5,
                event: state_replaced("session-live", "streamed-after-command"),
            })
            .unwrap();
        let delta = receive_server_json(&mut socket).await;
        assert_eq!(delta["type"], "delta");
        assert_eq!(delta["baseRevision"], 4);
        assert_eq!(delta["revision"], 5);
        assert_eq!(
            delta["event"]["session"]["transcript"][0]["text"],
            "streamed-after-command"
        );

        gateway
            .event_tx
            .send(RemotePagerEvent::Delta {
                session_id: "session-live".into(),
                base_revision: 3,
                revision: 6,
                event: state_replaced("session-live", "must-not-apply"),
            })
            .unwrap();
        let resync = receive_server_json(&mut socket).await;
        assert_eq!(resync["type"], "resyncRequired");
        assert_eq!(resync["protocolVersion"], REMOTE_PROTOCOL_VERSION);

        gateway.publish_snapshot(6, "continuous-after-gap");
        gateway
            .event_tx
            .send(RemotePagerEvent::Delta {
                session_id: "session-live".into(),
                base_revision: 5,
                revision: 6,
                event: state_replaced("session-live", "continuous-after-gap"),
            })
            .unwrap();
        let recovered_delta = receive_server_json(&mut socket).await;
        assert_eq!(recovered_delta["type"], "delta");
        assert_eq!(recovered_delta["baseRevision"], 5);
        assert_eq!(recovered_delta["revision"], 6);

        socket.close(None).await.unwrap();
        drop(socket);
        wait_for_client_owner(&gateway.state, None).await;

        gateway.publish_snapshot(9, "authoritative-after-reconnect");
        let mut reconnected = gateway.connect().await;
        assert_eq!(
            receive_server_json(&mut reconnected).await["type"],
            "connected"
        );
        send_hello(&mut reconnected).await;
        let fresh_snapshot = receive_server_json(&mut reconnected).await;
        assert_eq!(fresh_snapshot["type"], "snapshot");
        assert_eq!(fresh_snapshot["revision"], 9);
        assert_eq!(
            fresh_snapshot["session"]["transcript"][0]["text"],
            "authoritative-after-reconnect"
        );

        gateway
            .lifecycle_tx
            .send(RemoteRevocationReason::Stopped)
            .unwrap();
        let revoked = receive_server_json(&mut reconnected).await;
        assert_eq!(revoked["type"], "revoked");
        assert_eq!(revoked["reason"], "stopped");
        assert_websocket_ends(&mut reconnected).await;
        wait_for_client_owner(&gateway.state, None).await;
    }

    #[tokio::test]
    async fn latest_websocket_takes_over_and_superseded_client_cannot_mutate() {
        let mut gateway = LiveGatewayFixture::start(1, "only-client").await;
        let mut first = gateway.connect().await;
        assert_eq!(receive_server_json(&mut first).await["type"], "connected");
        let first_generation = gateway
            .state
            .client_ownership
            .current_generation()
            .expect("first client did not claim ownership");
        send_hello(&mut first).await;
        assert_eq!(receive_server_json(&mut first).await["type"], "snapshot");
        gateway.drain_phone_ready().await;

        let mut second = gateway.connect().await;
        assert_eq!(receive_server_json(&mut second).await["type"], "connected");
        let second_generation = gateway
            .state
            .client_ownership
            .current_generation()
            .expect("second client did not claim ownership");
        assert_ne!(first_generation, second_generation);

        let _ = first
            .send(ClientWebSocketMessage::Text(
                serde_json::json!({
                    "type":"command",
                    "protocolVersion":REMOTE_PROTOCOL_VERSION,
                    "commandId":"stale-prompt",
                    "command":{"type":"prompt","text":"must not run"}
                })
                .to_string()
                .into(),
            ))
            .await;
        assert!(
            tokio::time::timeout(Duration::from_millis(150), gateway.command_rx.recv())
                .await
                .is_err(),
            "superseded client delivered a command"
        );

        let frame = tokio::time::timeout(Duration::from_secs(2), first.next())
            .await
            .expect("timed out waiting for the superseded close")
            .expect("first websocket ended without a close frame")
            .expect("first websocket failed before the superseded close");
        let ClientWebSocketMessage::Close(Some(close)) = frame else {
            panic!("expected a websocket superseded close, got {frame:?}");
        };
        assert_eq!(u16::from(close.code), REMOTE_CLIENT_SUPERSEDED_CLOSE_CODE);
        assert!(close.reason.contains("superseded"));
        drop(first);
        assert_eq!(
            gateway.state.client_ownership.current_generation(),
            Some(second_generation),
            "stale disconnect cleared the new owner"
        );

        send_hello(&mut second).await;
        assert_eq!(receive_server_json(&mut second).await["type"], "snapshot");
        send_client_json(
            &mut second,
            serde_json::json!({
                "type":"command",
                "protocolVersion":REMOTE_PROTOCOL_VERSION,
                "commandId":"current-prompt",
                "command":{"type":"prompt","text":"run this"}
            }),
        )
        .await;
        let request = tokio::time::timeout(Duration::from_secs(2), gateway.recv_user_command())
            .await
            .expect("current owner command timed out");
        assert_eq!(request.command_id, "current-prompt");

        second.close(None).await.unwrap();
        drop(second);
        wait_for_client_owner(&gateway.state, None).await;
    }

    #[tokio::test]
    async fn session_created_never_migrates_to_a_takeover_client() {
        let mut gateway = LiveGatewayFixture::start(1, "source-owner").await;
        let mut first = gateway.connect().await;
        assert_eq!(receive_server_json(&mut first).await["type"], "connected");
        send_hello(&mut first).await;
        assert_eq!(receive_server_json(&mut first).await["type"], "snapshot");
        send_client_json(
            &mut first,
            serde_json::json!({
                "type":"command",
                "protocolVersion":REMOTE_PROTOCOL_VERSION,
                "commandId":"new-before-takeover",
                "command":{"type":"newSession"}
            }),
        )
        .await;
        let request = tokio::time::timeout(Duration::from_secs(2), gateway.recv_user_command())
            .await
            .expect("new-session command timed out");

        let mut second = gateway.connect().await;
        assert_eq!(receive_server_json(&mut second).await["type"], "connected");
        send_hello(&mut second).await;
        assert_eq!(receive_server_json(&mut second).await["type"], "snapshot");
        let frame = tokio::time::timeout(Duration::from_secs(2), first.next())
            .await
            .expect("superseded source did not close")
            .expect("superseded source ended without close")
            .expect("superseded source failed before close");
        assert!(matches!(frame, ClientWebSocketMessage::Close(_)));
        drop(first);

        let (delivery_ack, mut delivery_result) = mpsc::unbounded_channel();
        gateway
            .event_tx
            .send(RemotePagerEvent::SessionCreated {
                session_id: "session-live".into(),
                client_generation: request.client_generation,
                command_id: request.command_id,
                new_session_id: "must-not-reach-takeover".into(),
                pairing_url: "https://device.tail.example/forge/fresh/".into(),
                expires_at: "2030-01-02T03:04:05Z".into(),
                delivery_ack,
            })
            .unwrap();
        send_client_json(
            &mut second,
            serde_json::json!({
                "type":"command",
                "protocolVersion":REMOTE_PROTOCOL_VERSION,
                "commandId":"takeover-tries-accept",
                "command":{
                    "type":"acceptNewSession",
                    "sessionId":"must-not-reach-takeover"
                }
            }),
        )
        .await;
        let rejected = receive_server_json(&mut second).await;
        assert_eq!(rejected["type"], "commandResult");
        assert_eq!(rejected["outcome"]["error"]["code"], "newSessionMismatch");
        gateway
            .event_tx
            .send(RemotePagerEvent::CommandResult {
                session_id: "session-live".into(),
                client_generation: request.client_generation,
                command_id: "new-before-takeover".into(),
                outcome: RemoteCommandOutcome::Error {
                    error: RemoteError::new(
                        "new_session_failed",
                        "must not migrate to takeover client",
                        true,
                    ),
                },
            })
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(150), second.next())
                .await
                .is_err(),
            "takeover client received the original client's failure"
        );
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), delivery_result.recv())
                .await
                .expect("superseded handoff cleanup timed out"),
            None
        ));
        second.close(None).await.unwrap();
    }

    #[tokio::test]
    async fn takeover_after_acceptance_begin_survives_delayed_grant() {
        let mut gateway = LiveGatewayFixture::start(1, "source-owner").await;
        let mut first = gateway.connect().await;
        assert_eq!(receive_server_json(&mut first).await["type"], "connected");
        let first_generation = gateway.state.client_ownership.current_generation().unwrap();
        send_hello(&mut first).await;
        assert_eq!(receive_server_json(&mut first).await["type"], "snapshot");

        let (delivery_ack, mut delivery_result) = mpsc::unbounded_channel();
        gateway
            .event_tx
            .send(RemotePagerEvent::SessionCreated {
                session_id: "session-live".into(),
                client_generation: first_generation,
                command_id: "new-racing".into(),
                new_session_id: "racing-child".into(),
                pairing_url: "https://device.tail.example/forge/fresh/".into(),
                expires_at: "2030-01-02T03:04:05Z".into(),
                delivery_ack,
            })
            .unwrap();
        assert_eq!(
            receive_server_json(&mut first).await["type"],
            "sessionCreated"
        );
        send_client_json(
            &mut first,
            serde_json::json!({
                "type":"command",
                "protocolVersion":REMOTE_PROTOCOL_VERSION,
                "commandId":"accept-racing",
                "command":{"type":"acceptNewSession","sessionId":"racing-child"}
            }),
        )
        .await;
        let begin = delivery_result
            .recv()
            .await
            .expect("missing acceptance begin");
        let RemoteSessionAcceptance::Begin { granted } = begin else {
            panic!("expected acceptance begin")
        };

        let mut second = gateway.connect().await;
        assert_eq!(receive_server_json(&mut second).await["type"], "connected");
        granted.send(()).unwrap();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), delivery_result.recv())
                .await
                .expect("acceptance commit timed out"),
            Some(RemoteSessionAcceptance::Commit)
        ));
        let first_terminal = tokio::time::timeout(Duration::from_secs(1), first.next())
            .await
            .expect("reserved requester produced no terminal frame")
            .expect("reserved requester ended before its terminal frame")
            .expect("reserved requester websocket failed");
        match first_terminal {
            ClientWebSocketMessage::Text(payload) => {
                let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
                assert_eq!(value["type"], "commandResult");
                assert_eq!(value["outcome"]["status"], "ok");
            }
            ClientWebSocketMessage::Close(_) => {}
            other => panic!("unexpected requester terminal frame: {other:?}"),
        }
        second.close(None).await.unwrap();
    }

    #[tokio::test]
    async fn takeover_after_acceptance_grant_does_not_revoke_a_visible_ok() {
        let gateway = LiveGatewayFixture::start(1, "source-owner").await;
        let mut first = gateway.connect().await;
        assert_eq!(receive_server_json(&mut first).await["type"], "connected");
        let first_generation = gateway.state.client_ownership.current_generation().unwrap();
        send_hello(&mut first).await;
        assert_eq!(receive_server_json(&mut first).await["type"], "snapshot");
        let (delivery_ack, mut accepted) = mpsc::unbounded_channel();
        gateway
            .event_tx
            .send(RemotePagerEvent::SessionCreated {
                session_id: "session-live".into(),
                client_generation: first_generation,
                command_id: "new-racing".into(),
                new_session_id: "racing-child".into(),
                pairing_url: "https://device.tail.example/forge/fresh/".into(),
                expires_at: "2030-01-02T03:04:05Z".into(),
                delivery_ack,
            })
            .unwrap();
        assert_eq!(
            receive_server_json(&mut first).await["type"],
            "sessionCreated"
        );
        send_client_json(
            &mut first,
            serde_json::json!({
                "type":"command",
                "protocolVersion":REMOTE_PROTOCOL_VERSION,
                "commandId":"accept-racing",
                "command":{"type":"acceptNewSession","sessionId":"racing-child"}
            }),
        )
        .await;
        let begin = accepted.recv().await.unwrap();
        let RemoteSessionAcceptance::Begin {
            granted: pager_grant,
        } = begin
        else {
            panic!("expected acceptance begin")
        };
        pager_grant.send(()).unwrap();
        let mut second = gateway.connect().await;
        assert_eq!(receive_server_json(&mut second).await["type"], "connected");
        assert!(matches!(
            accepted.recv().await,
            Some(RemoteSessionAcceptance::Commit)
        ));
        let first_terminal = tokio::time::timeout(Duration::from_secs(1), first.next())
            .await
            .expect("original requester produced no terminal frame")
            .expect("original requester ended before its terminal frame")
            .expect("original requester websocket failed");
        match first_terminal {
            ClientWebSocketMessage::Text(payload) => {
                let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
                assert_eq!(value["type"], "commandResult");
                assert_eq!(value["outcome"]["status"], "ok");
            }
            ClientWebSocketMessage::Close(_) => {}
            other => panic!("unexpected requester terminal frame: {other:?}"),
        }
        second.close(None).await.unwrap();
    }

    #[tokio::test]
    async fn failed_websocket_upgrade_does_not_evict_current_owner() {
        let gateway = LiveGatewayFixture::start(1, "current-owner").await;
        let mut current = gateway.connect().await;
        assert_eq!(receive_server_json(&mut current).await["type"], "connected");
        let current_generation = gateway
            .state
            .client_ownership
            .current_generation()
            .expect("current client did not claim ownership");

        let mut failed = TcpStream::connect(gateway.address).await.unwrap();
        failed
            .write_all(
                format!(
                    "GET /forge/{}/events HTTP/1.1\r\nHost: {}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 12\r\n\r\n",
                    gateway.token, gateway.address
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut response = [0_u8; 1024];
        let response_len = tokio::time::timeout(Duration::from_secs(2), failed.read(&mut response))
            .await
            .expect("failed upgrade response timed out")
            .unwrap();
        let response = String::from_utf8_lossy(&response[..response_len]);
        assert!(!response.starts_with("HTTP/1.1 101"));
        assert_eq!(
            gateway.state.client_ownership.current_generation(),
            Some(current_generation)
        );

        send_hello(&mut current).await;
        assert_eq!(receive_server_json(&mut current).await["type"], "snapshot");
        current.close(None).await.unwrap();
        drop(current);
        wait_for_client_owner(&gateway.state, None).await;
    }

    #[tokio::test]
    async fn hello_snapshot_ignores_buffered_stale_deltas_and_keeps_live_updates() {
        let gateway = LiveGatewayFixture::start(4, "before-hello").await;
        let mut socket = gateway.connect().await;
        assert_eq!(receive_server_json(&mut socket).await["type"], "connected");
        gateway
            .event_tx
            .send(RemotePagerEvent::Delta {
                session_id: "session-live".into(),
                base_revision: 3,
                revision: 4,
                event: state_replaced("session-live", "already-in-snapshot"),
            })
            .unwrap();
        gateway
            .event_tx
            .send(RemotePagerEvent::Delta {
                session_id: "session-live".into(),
                base_revision: 4,
                revision: 5,
                event: state_replaced("session-live", "live-after-hello"),
            })
            .unwrap();

        send_hello(&mut socket).await;
        assert_eq!(receive_server_json(&mut socket).await["type"], "snapshot");
        let live = receive_server_json(&mut socket).await;
        assert_eq!(live["type"], "delta");
        assert_eq!(live["baseRevision"], 4);
        assert_eq!(live["revision"], 5);
        assert_eq!(
            live["event"]["session"]["transcript"][0]["text"],
            "live-after-hello"
        );
        socket.close(None).await.unwrap();
    }

    #[tokio::test]
    async fn remote_websocket_fails_closed_on_cross_session_delta_payload() {
        let gateway = LiveGatewayFixture::start(12, "correct-session").await;
        let mut socket = gateway.connect().await;
        assert_eq!(receive_server_json(&mut socket).await["type"], "connected");
        send_hello(&mut socket).await;
        assert_eq!(receive_server_json(&mut socket).await["type"], "snapshot");

        gateway
            .event_tx
            .send(RemotePagerEvent::Delta {
                session_id: "session-live".into(),
                base_revision: 12,
                revision: 13,
                event: state_replaced("different-session", "must-never-render"),
            })
            .unwrap();
        let error = receive_server_json(&mut socket).await;
        assert_eq!(error["type"], "error");
        assert_eq!(error["error"]["code"], "sessionMismatch");
        assert_eq!(error["error"]["retryable"], false);
        assert_websocket_ends(&mut socket).await;
        wait_for_client_owner(&gateway.state, None).await;
    }

    async fn assert_websocket_ends(socket: &mut TestWebSocket) {
        let terminal = tokio::time::timeout(Duration::from_secs(2), socket.next())
            .await
            .expect("gateway did not fail closed");
        match terminal {
            None | Some(Ok(ClientWebSocketMessage::Close(_))) | Some(Err(_)) => {}
            Some(Ok(other)) => panic!("gateway sent data after fail-close: {other:?}"),
        }
    }

    #[test]
    fn embedded_client_has_no_inline_script_or_style_and_keeps_mit_notice() {
        assert!(!INDEX_HTML.contains("<script>"));
        assert!(!INDEX_HTML.contains("<style>"));
        assert!(INDEX_HTML.contains("./assets/app.js"));
        assert!(INDEX_HTML.contains("./assets/app.css"));
        assert!(THIRD_PARTY_NOTICES.contains("MIT"));
        assert!(THIRD_PARTY_NOTICES.contains("T3 Tools Inc."));
        assert!(APP_JS.contains("snapshotUnavailable"));
        assert!(APP_JS.contains("4410"));
    }

    #[test]
    fn pairing_notice_has_qr_and_exact_private_url() {
        let url = "https://mac.tail.ts.net/forge/secret/";
        let notice = remote_pairing_notice(url);
        assert!(notice.contains(url));
        assert!(
            notice
                .chars()
                .any(|character| matches!(character, '█' | '▀' | '▄'))
        );
    }

    #[test]
    fn pairing_qr_uses_compact_half_block_cells_with_full_quiet_zone() {
        let token = "ab".repeat(PAIRING_BYTES);
        let url = format!("https://forge-mac.example-tailnet.ts.net/forge/{token}/");
        let encoded = QrCode::encode_text(&url, QrCodeEcc::Medium).unwrap();
        let rendered = remote_qr_text(&url).unwrap();
        let lines = rendered.lines().collect::<Vec<_>>();
        let module_span = encoded.size() + 8;

        assert_eq!(lines.len(), ((module_span + 1) / 2) as usize);
        assert!(
            module_span <= 80,
            "pairing QR should not wrap at 80 columns"
        );
        assert!(
            lines
                .iter()
                .all(|line| line.chars().count() == module_span as usize)
        );

        for (row, line) in lines.iter().enumerate() {
            let upper_y = -4 + row as i32 * 2;
            for (column, character) in line.chars().enumerate() {
                let x = -4 + column as i32;
                let expected_module = |y| {
                    x >= 0
                        && y >= 0
                        && x < encoded.size()
                        && y < encoded.size()
                        && encoded.get_module(x, y)
                };
                let actual = match character {
                    '█' => (true, true),
                    '▀' => (true, false),
                    '▄' => (false, true),
                    ' ' => (false, false),
                    other => panic!("unexpected QR cell {other:?}"),
                };
                assert_eq!(
                    actual,
                    (expected_module(upper_y), expected_module(upper_y + 1)),
                    "QR cell mismatch at column {column}, row {row}",
                );
            }
        }
    }
}
