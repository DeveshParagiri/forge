use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponsesBackend {
    Standard,
    Codex,
}

impl ResponsesBackend {
    pub(crate) fn detect(base_url: &str) -> Self {
        if super::endpoint_policy::is_codex_endpoint(base_url) {
            Self::Codex
        } else {
            Self::Standard
        }
    }

    pub(crate) fn accepts_xai_extensions(self) -> bool {
        matches!(self, Self::Standard)
    }

    pub(crate) fn prepare_request_body(self, body: Value, fast_mode: bool) -> Value {
        match self {
            Self::Standard => body,
            Self::Codex => sanitize_body_for_codex_backend(body, fast_mode),
        }
    }
}

/// OpenAI / ChatGPT Responses rejects item `id` / `call_id` strings longer
/// than this (error: `Invalid 'input[N].id': string too long`).
const RESPONSES_ITEM_ID_MAX_LEN: usize = 64;

/// Hosted / server-side tool item types that Grok may have persisted from an
/// xAI Responses turn. Codex cannot execute or round-trip these; replaying
/// them also ships xAI-generated ids that routinely exceed the 64-char limit
/// (e.g. `ws_<uuid>_call-<uuid>-N` at 83+ chars).
const HOSTED_TOOL_CALL_TYPES: &[&str] = &[
    "web_search_call",
    "code_interpreter_call",
    "custom_tool_call",
    "file_search_call",
    "image_generation_call",
    "computer_call",
    "computer_call_output",
];

/// Apply only the proven ChatGPT Codex transport deltas to an upstream
/// Responses request. Generic Responses defaults and schema construction stay
/// owned by the shared sampler; this shim removes fields rejected by the
/// subscription endpoint, lifts system/developer input into `instructions`,
/// filters unsupported hosted tools, and repairs oversized item IDs.
fn sanitize_body_for_codex_backend(mut body: Value, fast_mode: bool) -> Value {
    let Some(out) = body.as_object_mut() else {
        return body;
    };

    for key in [
        "temperature",
        "top_p",
        "max_output_tokens",
        "truncation",
        "background",
        "metadata",
        "stream_tool_calls",
    ] {
        out.remove(key);
    }

    let mut instruction_parts = out
        .get("instructions")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(|text| vec![text.to_string()])
        .unwrap_or_default();
    let mut filtered_input = Vec::new();
    if let Some(items) = out
        .remove("input")
        .and_then(|value| value.as_array().cloned())
    {
        for item in items {
            let role = item.get("role").and_then(Value::as_str).unwrap_or("");
            if role.eq_ignore_ascii_case("system") || role.eq_ignore_ascii_case("developer") {
                if let Some(text) = extract_input_item_text(&item)
                    && !text.trim().is_empty()
                {
                    instruction_parts.push(text);
                }
                continue;
            }

            let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
            if HOSTED_TOOL_CALL_TYPES.contains(&item_type) {
                filtered_input.push(hosted_tool_call_to_assistant_message(&item));
            } else {
                filtered_input.push(clamp_responses_item_ids(item));
            }
        }
    }
    out.insert("input".into(), Value::Array(filtered_input));

    if !instruction_parts.is_empty() {
        out.insert(
            "instructions".into(),
            Value::String(instruction_parts.join("\n\n")),
        );
    }

    // Upstream emits canonical function tools. Codex rejects hosted/xAI tool
    // definitions, so remove only non-function forms without reconstructing
    // otherwise ordinary tool JSON.
    if let Some(Value::Array(tools)) = out.get_mut("tools") {
        tools.retain(|tool| tool.get("type").and_then(Value::as_str) == Some("function"));
        if tools.is_empty() {
            out.remove("tools");
            out.remove("tool_choice");
            out.remove("parallel_tool_calls");
        }
    }

    // ChatGPT subscription inference must remain zero-data-retention even if a
    // generic Responses caller explicitly supplied `store = true`.
    out.insert("store".into(), Value::Bool(false));
    super::fast_mode::apply_codex_request_option(out, fast_mode);
    body
}

/// Clamp Responses item `id` / `call_id` fields to the API max length.
/// Uses a stable suffix truncation so paired call/result ids remain equal
/// when both sides are clamped the same way.
fn clamp_responses_id(id: &str) -> String {
    if id.len() <= RESPONSES_ITEM_ID_MAX_LEN {
        return id.to_string();
    }
    id[id.len() - RESPONSES_ITEM_ID_MAX_LEN..].to_string()
}

fn clamp_responses_item_ids(mut item: Value) -> Value {
    if let Value::Object(obj) = &mut item {
        for key in ["id", "call_id"] {
            if let Some(Value::String(s)) = obj.get(key)
                && s.len() > RESPONSES_ITEM_ID_MAX_LEN
            {
                let clamped = clamp_responses_id(s);
                obj.insert(key.to_string(), Value::String(clamped));
            }
        }
    }
    item
}

/// Flatten a Grok-hosted tool call into an assistant EasyMessage so Codex
/// still sees what happened without receiving illegal item types/ids.
fn hosted_tool_call_to_assistant_message(item: &Value) -> Value {
    let summary = summarize_hosted_tool_call(item);
    json!({
        "role": "assistant",
        "content": summary,
    })
}

fn summarize_hosted_tool_call(item: &Value) -> String {
    let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("tool");
    match item_type {
        "web_search_call" => {
            let action = item.get("action");
            let action_type = action
                .and_then(|a| a.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("search");
            let query = action
                .and_then(|a| a.get("query"))
                .and_then(|q| q.as_str())
                .unwrap_or("?");
            let url = action
                .and_then(|a| a.get("url"))
                .and_then(|u| u.as_str())
                .unwrap_or("?");
            match action_type {
                "open_page" | "open" => format!("[backend web_search] open: {url}"),
                "find" | "find_in_page" => {
                    let pattern = action
                        .and_then(|a| a.get("pattern"))
                        .and_then(|p| p.as_str())
                        .unwrap_or("?");
                    format!("[backend web_search] find \"{pattern}\" in {url}")
                }
                _ => format!("[backend web_search] search: {query}"),
            }
        }
        "code_interpreter_call" => {
            let code = item.get("code").and_then(|c| c.as_str()).unwrap_or("");
            let preview = if code.len() > 100 {
                format!("{}...", &code[..100])
            } else {
                code.to_string()
            };
            format!("[backend code_interpreter] {preview}")
        }
        "custom_tool_call" => {
            let name = item
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("custom");
            let input = item.get("input").and_then(|i| i.as_str()).unwrap_or("");
            format!("[backend {name}] {input}")
        }
        other => format!("[backend {other}]"),
    }
}

fn extract_input_item_text(item: &Value) -> Option<String> {
    // content may be string or array of {type, text}
    match item.get("content") {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Array(parts)) => {
            let mut texts = Vec::new();
            for p in parts {
                if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                    texts.push(t.to_string());
                }
            }
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n"))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_only_the_official_https_codex_endpoint() {
        assert_eq!(
            ResponsesBackend::detect("https://chatgpt.com/backend-api/codex"),
            ResponsesBackend::Codex
        );
        for url in [
            "https://proxy.example/backend-api/codex/v1",
            "https://chatgpt.com.attacker.example/backend-api/codex",
            "https://chatgpt.com@attacker.example/backend-api/codex",
            "http://chatgpt.com/backend-api/codex",
            "https://api.x.ai/v1",
        ] {
            assert_eq!(
                ResponsesBackend::detect(url),
                ResponsesBackend::Standard,
                "classified {url} as Codex"
            );
        }
    }

    #[test]
    fn fast_mode_adds_codex_priority_service_tier_only_when_enabled() {
        let body = json!({"model": "gpt-5", "input": []});
        let enabled = ResponsesBackend::Codex.prepare_request_body(body.clone(), true);
        assert_eq!(enabled["service_tier"], "priority");

        let disabled = ResponsesBackend::Codex.prepare_request_body(body.clone(), false);
        assert!(disabled.get("service_tier").is_none());

        let standard = ResponsesBackend::Standard.prepare_request_body(body, true);
        assert!(standard.get("service_tier").is_none());
    }

    #[test]
    fn lifts_system_and_strips_forbidden_params() {
        let body = json!({
            "model": "gpt-5.6-sol",
            "temperature": 1.0,
            "top_p": 0.98,
            "max_output_tokens": 4096,
            "truncation": "disabled",
            "background": false,
            "metadata": {"x": "y"},
            "stream_tool_calls": true,
            "store": true,
            "stream": false,
            "input": [
                {"role": "system", "content": [{"type": "input_text", "text": "You are Grok"}]},
                {"role": "user", "content": [{"type": "input_text", "text": "hi"}]},
            ],
            "tools": [{"type": "function", "name": "bash", "parameters": {"type": "object"}}],
            "reasoning": {"effort": "medium", "summary": "concise"},
        });
        let out = ResponsesBackend::Codex.prepare_request_body(body, false);
        assert_eq!(out["store"], serde_json::json!(false));
        assert_eq!(out["stream"], serde_json::json!(false));
        assert!(out.get("temperature").is_none());
        assert!(out.get("top_p").is_none());
        assert!(out.get("max_output_tokens").is_none());
        assert!(out.get("stream_tool_calls").is_none());
        assert!(out.get("metadata").is_none());
        assert!(
            out["instructions"]
                .as_str()
                .unwrap()
                .contains("You are Grok")
        );
        let input = out["input"].as_array().unwrap();
        assert_eq!(input.len(), 1);
        assert_eq!(input[0]["role"], "user");
        assert!(out.get("tools").is_some());
    }

    #[test]
    fn only_invents_the_codex_privacy_default() {
        let out =
            ResponsesBackend::Codex.prepare_request_body(serde_json::json!({"input": []}), false);
        assert!(out.get("instructions").is_none());
        assert_eq!(out.get("store"), Some(&serde_json::json!(false)));
        assert!(out.get("stream").is_none());
        assert!(out.get("include").is_none());
        assert!(out.get("reasoning").is_none());
        assert!(out.get("tool_choice").is_none());
        assert!(out.get("parallel_tool_calls").is_none());
    }

    #[test]
    fn flattens_long_id_web_search_calls_to_assistant_text() {
        // Real xAI web_search id shape from multi-model sessions (len 83).
        let long_id =
            "ws_fa360a91-a3af-9c3d-8e87-af63bef7cd19_call-a13bddef-5064-43a6-894f-30ae6ce39ff3-3";
        assert_eq!(long_id.len(), 83);
        assert!(long_id.len() > RESPONSES_ITEM_ID_MAX_LEN);

        let body = json!({
            "model": "gpt-5.6-sol",
            "input": [
                {"role": "user", "content": "find the olive food app"},
                {
                    "type": "web_search_call",
                    "id": long_id,
                    "status": "completed",
                    "action": {
                        "type": "search",
                        "query": "Olive food app App Store",
                        "sources": []
                    }
                },
                {"role": "assistant", "content": "It's a barcode scanner."},
            ],
        });

        let out = ResponsesBackend::Codex.prepare_request_body(body, false);
        let input = out["input"].as_array().expect("input array");
        assert_eq!(
            input.len(),
            3,
            "web_search should become a text item, not drop"
        );

        // No raw hosted tool types remain.
        for item in input {
            let t = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            assert_ne!(t, "web_search_call");
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                assert!(
                    id.len() <= RESPONSES_ITEM_ID_MAX_LEN,
                    "id still too long: {id} (len {})",
                    id.len()
                );
            }
            if let Some(id) = item.get("call_id").and_then(|v| v.as_str()) {
                assert!(
                    id.len() <= RESPONSES_ITEM_ID_MAX_LEN,
                    "call_id still too long: {id} (len {})",
                    id.len()
                );
            }
        }

        let flattened = &input[1];
        assert_eq!(flattened["role"], "assistant");
        let content = flattened["content"]
            .as_str()
            .expect("assistant text content");
        assert!(
            content.contains("[backend web_search]"),
            "expected summary, got {content}"
        );
        assert!(
            content.contains("Olive food app App Store"),
            "query should survive in summary: {content}"
        );
        // Critical: the illegal long id must not appear anywhere in input.
        let serialized = serde_json::to_string(&out["input"]).unwrap();
        assert!(
            !serialized.contains(long_id),
            "long web_search id leaked into Codex input"
        );
    }

    #[test]
    fn clamps_oversized_function_call_ids() {
        let long_call_id = "c".repeat(80);
        let body = json!({
            "input": [
                {
                    "type": "function_call",
                    "call_id": long_call_id,
                    "name": "bash",
                    "arguments": "{}"
                },
                {
                    "type": "function_call_output",
                    "call_id": long_call_id,
                    "output": "ok"
                }
            ]
        });
        let out = ResponsesBackend::Codex.prepare_request_body(body, false);
        let input = out["input"].as_array().unwrap();
        let call_id = input[0]["call_id"].as_str().unwrap();
        let out_id = input[1]["call_id"].as_str().unwrap();
        assert_eq!(call_id.len(), RESPONSES_ITEM_ID_MAX_LEN);
        assert_eq!(call_id, out_id, "paired call ids must clamp identically");
    }

    #[test]
    fn standard_backend_leaves_web_search_items_untouched() {
        let long_id =
            "ws_fa360a91-a3af-9c3d-8e87-af63bef7cd19_call-a13bddef-5064-43a6-894f-30ae6ce39ff3-3";
        let body = json!({
            "input": [{
                "type": "web_search_call",
                "id": long_id,
                "status": "completed",
                "action": {"type": "search", "query": "q", "sources": []}
            }]
        });
        let out = ResponsesBackend::Standard.prepare_request_body(body, false);
        assert_eq!(out["input"][0]["type"], "web_search_call");
        assert_eq!(out["input"][0]["id"], long_id);
    }
}
