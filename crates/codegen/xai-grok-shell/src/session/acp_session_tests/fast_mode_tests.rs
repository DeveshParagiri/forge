use super::*;

fn fast_capable_entry(catalog_slug: &str) -> crate::agent::config::ModelEntry {
    let mut entry = crate::agent::config::ModelEntry::fallback(
        catalog_slug,
        &crate::agent::config::EndpointsConfig::default(),
    );
    entry.info.supports_fast_mode = true;
    entry
}

#[tokio::test(flavor = "current_thread")]
async fn fast_mode_changes_only_the_live_sampling_flag() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor =
                support::create_test_actor(0, 200_000, 85, gateway_tx, persistence_tx).await;
            actor
                .models_manager
                .insert_test_entry("catalog-id", fast_capable_entry("test"));

            let before = actor
                .chat_state_handle
                .get_sampling_config()
                .await
                .expect("test actor has sampling config");
            let live_model = actor
                .handle_set_sampling_fast_mode(true)
                .await
                .expect("capable live model accepts Fast Mode");
            let after = actor
                .chat_state_handle
                .get_sampling_config()
                .await
                .expect("sampling config remains available");
            let mut expected = before.clone();
            expected.fast_mode = Some(true);

            assert_eq!(live_model, "test");
            assert_eq!(after.model, before.model, "model identity must not change");
            assert_eq!(
                serde_json::to_value(after).unwrap(),
                serde_json::to_value(expected).unwrap(),
                "Fast Mode must be the only changed sampling field"
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn unsupported_live_model_is_rejected_without_mutation() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor =
                support::create_test_actor(0, 200_000, 85, gateway_tx, persistence_tx).await;
            let entry = crate::agent::config::ModelEntry::fallback(
                "test",
                &crate::agent::config::EndpointsConfig::default(),
            );
            actor.models_manager.insert_test_entry("test", entry);
            let before = actor
                .chat_state_handle
                .get_sampling_config()
                .await
                .expect("test actor has sampling config");

            assert!(actor.handle_set_sampling_fast_mode(true).await.is_err());
            let after = actor
                .chat_state_handle
                .get_sampling_config()
                .await
                .expect("sampling config remains available");
            assert_eq!(
                serde_json::to_value(after).unwrap(),
                serde_json::to_value(before).unwrap(),
                "rejected Fast Mode request must not mutate sampling config"
            );
        })
        .await;
}
