use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use axum::Router;
use axum::response::sse::{Event, Sse};
use axum::routing::post;
use futures_util::stream;
use indexmap::IndexMap;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};

use xai_grok_sampler::{
    ApiBackend, RequestId, RetryPolicy, SamplerActor, SamplerConfig, SamplingEvent,
};
use xai_grok_sampling_types::{ContentPart, ConversationItem, ConversationRequest, UserItem};
use xai_grok_test_support::{SseEvent, sse};

struct MockServer {
    addr: SocketAddr,
    shutdown_tx: oneshot::Sender<()>,
}

impl MockServer {
    async fn spawn(app: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        Self { addr, shutdown_tx }
    }

    fn base_url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

fn responses_config(base_url: String) -> SamplerConfig {
    SamplerConfig {
        api_key: Some("test-key".into()),
        base_url,
        model: "test-model".into(),
        max_completion_tokens: Some(1024),
        temperature: None,
        top_p: None,
        api_backend: ApiBackend::Responses,
        auth_scheme: Default::default(),
        extra_headers: IndexMap::new(),
        context_window: 128_000,
        force_http1: false,
        max_retries: Some(2),
        stream_tool_calls: false,
        idle_timeout_secs: Some(30),
        reasoning_effort: None,
        origin_client: None,
        client_identifier: None,
        deployment_id: None,
        user_id: None,
        client_version: None,
        attribution_callback: None,
        bearer_resolver: None,
        supports_backend_search: false,
        compactions_remaining: None,
        compaction_at_tokens: None,
        doom_loop_recovery: None,
        header_injector: None,
    }
}

fn user_request(text: &str) -> ConversationRequest {
    ConversationRequest {
        items: vec![ConversationItem::User(UserItem {
            content: vec![ContentPart::Text {
                text: Arc::<str>::from(text),
            }],
            synthetic_reason: None,
            ..Default::default()
        })],
        ..Default::default()
    }
}

fn sse_events_to_axum(events: Vec<SseEvent>) -> Vec<Event> {
    events
        .into_iter()
        .map(|event| {
            let axum_event = Event::default().data(event.data);
            match event.event {
                Some(name) => axum_event.event(name),
                None => axum_event,
            }
        })
        .collect()
}

/// Codex can add metadata-only and keepalive SSE frames that land after visible
/// text but before `response.completed`. Such non-content events must not fail
/// or retry the turn, and the already-streamed text must remain the terminal
/// response.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_metadata_and_keepalive_events_are_ignored_without_retry() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_handler = Arc::clone(&counter);
    let app = Router::new().route(
        "/v1/responses",
        post(move || {
            let counter = Arc::clone(&counter_handler);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                let mut events = sse::responses_api_reasoning_and_text_events(
                    "brief thought",
                    "metadata-safe",
                    "test-model",
                );
                let terminal_index = events
                    .iter()
                    .position(|event| event.data.contains("response.completed"))
                    .expect("generated stream has a terminal event");
                events.insert(
                    terminal_index,
                    SseEvent::data(
                        json!({
                            "type": "response.metadata",
                            "sequence_number": 99,
                            "metadata": {"service_tier": "priority"}
                        })
                        .to_string(),
                    ),
                );
                events.insert(
                    terminal_index + 1,
                    SseEvent::data(
                        json!({"type": "keepalive", "sequence_number": 100}).to_string(),
                    ),
                );
                let events = sse_events_to_axum(events);
                Sse::new(stream::iter(
                    events.into_iter().map(Ok::<_, std::convert::Infallible>),
                ))
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel::<SamplingEvent>();
    let handle = SamplerActor::spawn(
        responses_config(server.base_url()),
        RetryPolicy::default(),
        event_tx,
    );

    let result = handle
        .submit_and_collect(RequestId::from("req-metadata"), user_request("hi"))
        .await;
    server.shutdown();

    let (response, _metrics) = result.expect("additive metadata must not fail the turn");
    assert_eq!(counter.load(Ordering::SeqCst), 1, "must not retry");
    assert_eq!(response.assistant_text(), "metadata-safe");
    assert!(response.empty_reason().is_none());
}
