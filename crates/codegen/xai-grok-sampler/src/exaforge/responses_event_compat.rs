/// Returns whether a typed-deserialization failure represents a syntactically
/// valid additive Responses event or recognized transport-liveness event that
/// can be ignored without hiding malformed payloads for known event types.
pub(crate) fn ignorable_unknown_event(
    event_type: Option<&str>,
    deserialization_error: &serde_json::Error,
) -> bool {
    event_type.is_some_and(|kind| {
        let is_additive_or_liveness =
            kind.starts_with("response.") || matches!(kind, "keepalive" | "heartbeat" | "ping");
        is_additive_or_liveness
            && deserialization_error
                .to_string()
                .starts_with(&format!("unknown variant `{kind}`"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_sampling_types::rs;

    fn response_event_error(data: &str) -> serde_json::Error {
        serde_json::from_str::<rs::ResponseStreamEvent>(data).expect_err("event must be unknown")
    }

    #[test]
    fn accepts_additive_response_event_unknown_to_async_openai() {
        let data = r#"{"type":"response.metadata","sequence_number":7}"#;
        let error = response_event_error(data);
        assert!(ignorable_unknown_event(Some("response.metadata"), &error));
    }

    #[test]
    fn accepts_recognized_liveness_event_unknown_to_async_openai() {
        let data = r#"{"type":"keepalive","sequence_number":2}"#;
        let error = response_event_error(data);
        assert!(ignorable_unknown_event(Some("keepalive"), &error));
    }

    #[test]
    fn rejects_malformed_known_event() {
        let data = r#"{"type":"response.output_text.delta","sequence_number":8}"#;
        let error = serde_json::from_str::<rs::ResponseStreamEvent>(data)
            .expect_err("known event is malformed");
        assert!(!ignorable_unknown_event(
            Some("response.output_text.delta"),
            &error
        ));
    }

    #[test]
    fn rejects_arbitrary_unknown_event() {
        let data = r#"{"type":"vendor.unknown","sequence_number":3}"#;
        let error = response_event_error(data);
        assert!(!ignorable_unknown_event(Some("vendor.unknown"), &error));
    }
}
