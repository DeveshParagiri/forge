use super::*;

#[tokio::test]
async fn set_fast_mode_request_has_no_model_selection_fields() {
    use std::sync::Arc;
    use xai_acp_lib::AcpAgentMessage;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let message = rx.recv().await.expect("Fast Mode extension request");
        let AcpAgentMessage::ExtMethod(args) = message else {
            panic!("expected extension request");
        };
        assert_eq!(args.request.method.as_ref(), "x.ai/session/fast_mode");
        let params: serde_json::Value =
            serde_json::from_str(args.request.params.get()).expect("valid Fast Mode params");
        assert_eq!(params["sessionId"], "session-fast");
        assert_eq!(params["enabled"], true);
        assert!(params.get("modelId").is_none());
        assert!(params.get("model").is_none());
        assert!(params.get("_meta").is_none());
        let raw = serde_json::value::RawValue::from_string(r#"{"enabled":true}"#.into())
            .expect("serialize Fast Mode response");
        let _ = args
            .response_tx
            .send(Ok(acp::ExtResponse::new(Arc::from(raw))));
    });

    let session_id = acp::SessionId::new("session-fast");
    let mut tasks = JoinSet::new();
    let (progress_tx, _progress_rx) = tokio::sync::mpsc::unbounded_channel();
    execute(
        Effect::SetFastMode {
            agent_id: AgentId(4),
            session_id: session_id.clone(),
            enabled: true,
        },
        &mut tasks,
        &tx,
        Path::new("."),
        &SessionFlags::default(),
        &progress_tx,
    );

    match tasks.join_next().await.expect("task").expect("no panic") {
        TaskResult::SetFastModeComplete {
            agent_id,
            session_id: completed_session,
            enabled,
            result,
        } => {
            assert_eq!(agent_id, AgentId(4));
            assert_eq!(completed_session, session_id);
            assert!(enabled);
            assert!(result.is_ok());
        }
        other => panic!("expected SetFastModeComplete, got {other:?}"),
    }
}
