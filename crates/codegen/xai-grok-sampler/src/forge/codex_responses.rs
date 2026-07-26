use reqwest::header::HeaderMap;
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponsesBackend {
    Standard,
    Codex,
}

impl ResponsesBackend {
    pub(crate) fn detect(base_url: &str) -> Self {
        if base_url.contains("chatgpt.com") || base_url.contains("backend-api/codex") {
            Self::Codex
        } else {
            Self::Standard
        }
    }

    pub(crate) fn accepts_xai_extensions(self) -> bool {
        matches!(self, Self::Standard)
    }

    pub(crate) fn uses_grok_headers(self) -> bool {
        matches!(self, Self::Standard)
    }

    /// Strip xAI/Grok transport metadata from requests sent to a third-party
    /// Responses backend. Authentication, content negotiation, tracing, and
    /// explicit provider headers remain intact; only the fork's private
    /// `x-grok-*` namespace and xAI proxy marker are removed.
    pub(crate) fn prepare_request_headers(self, mut headers: HeaderMap) -> HeaderMap {
        if matches!(self, Self::Codex) {
            let private_headers: Vec<_> = headers
                .keys()
                .filter(|name| name.as_str().starts_with("x-grok-"))
                .cloned()
                .collect();
            for name in private_headers {
                headers.remove(name);
            }
            headers.remove("x-xai-token-auth");
        }
        headers
    }

    pub(crate) fn supports_doom_loop_check(self) -> bool {
        matches!(self, Self::Standard)
    }

    pub(crate) fn prepare_request_body(self, body: Value, fast_mode: bool) -> Value {
        match self {
            Self::Standard => body,
            Self::Codex => sanitize_body_for_codex_backend(body, fast_mode),
        }
    }

    pub(crate) fn augment_error_message(self, message: String, bytes: &[u8]) -> String {
        match self {
            Self::Standard => message,
            Self::Codex => {
                let detail = String::from_utf8_lossy(bytes);
                if detail.contains("detail") || detail.contains("Unsupported") {
                    format!("{message} — {detail}")
                } else {
                    message
                }
            }
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

/// Reshape a Responses API JSON body for ChatGPT Codex
/// (`chatgpt.com/backend-api/codex/responses`), matching Pi's
/// `openai-codex-responses` contract.
///
/// Codex rejects (400) parameters that api.openai.com / api.x.ai accept:
/// `temperature`, `top_p`, `max_output_tokens`, `truncation`, `background`,
/// `metadata`, `stream_tool_calls`, and **system messages in `input`**
/// (use `instructions` instead). Also requires `store: false` and `stream: true`.
///
/// Additionally, when a session previously used xAI-hosted tools (web search
/// etc.), those `web_search_call` input items carry ids longer than 64 chars
/// and/or tool types Codex does not accept. We flatten them to synthetic
/// assistant text — the same strategy chat-completions uses for
/// `BackendToolCall` — and clamp any remaining `id` / `call_id` fields.
fn sanitize_body_for_codex_backend(mut body: Value, fast_mode: bool) -> Value {
    // Lift system / developer text out of input → instructions (Pi style).
    let mut instruction_parts: Vec<String> = Vec::new();
    if let Some(existing) = body.get("instructions").and_then(|v| v.as_str())
        && !existing.trim().is_empty()
    {
        instruction_parts.push(existing.to_string());
    }

    let mut filtered_input: Vec<Value> = Vec::new();
    if let Some(items) = body.get("input").and_then(|v| v.as_array()) {
        for item in items {
            let role = item.get("role").and_then(|r| r.as_str()).unwrap_or("");
            // EasyMessage form: { type?, role, content }
            // Item form: { type: "message", role, content }
            let is_systemish =
                role.eq_ignore_ascii_case("system") || role.eq_ignore_ascii_case("developer");
            if is_systemish {
                if let Some(text) = extract_input_item_text(item)
                    && !text.trim().is_empty()
                {
                    instruction_parts.push(text);
                }
                continue;
            }

            let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if HOSTED_TOOL_CALL_TYPES.contains(&item_type) {
                // Preserve search/context continuity without shipping illegal
                // ids or tool types Codex cannot round-trip.
                filtered_input.push(hosted_tool_call_to_assistant_message(item));
                continue;
            }

            filtered_input.push(clamp_responses_item_ids(item.clone()));
        }
    }

    // Whitelist of Codex-accepted top-level keys (Pi buildRequestBody + tools).
    // Anything else → 400 Unsupported parameter.
    const ALLOW: &[&str] = &[
        "model",
        "input",
        "instructions",
        "tools",
        "tool_choice",
        "parallel_tool_calls",
        "reasoning",
        "include",
        "text",
        "prompt_cache_key",
        "service_tier",
        "store",
        "stream",
    ];

    let mut out = Map::new();
    if let Some(obj) = body.as_object_mut() {
        for key in ALLOW {
            if let Some(v) = obj.remove(*key)
                && !v.is_null()
            {
                out.insert((*key).to_string(), v);
            }
        }
    }

    out.insert("store".into(), json!(false));
    out.insert("stream".into(), json!(true));
    // Forge: concrete fast-mode wire mapping stays in its feature module.
    super::fast_mode::apply_codex_request_option(&mut out, fast_mode);
    out.insert("input".into(), Value::Array(filtered_input));

    if !instruction_parts.is_empty() {
        out.insert(
            "instructions".into(),
            Value::String(instruction_parts.join("\n\n")),
        );
    } else if !out.contains_key("instructions") {
        // Pi always sends instructions; Codex is fine with a default.
        out.insert(
            "instructions".into(),
            Value::String("You are a helpful assistant.".into()),
        );
    }

    // Reasoning: keep effort; prefer summary "auto" (Pi) for visible streams.
    if let Some(Value::Object(reasoning)) = out.get_mut("reasoning") {
        if reasoning.get("effort").map(|e| e.is_null()).unwrap_or(true) {
            reasoning.remove("effort");
        }
        // Drop null-only reasoning
        if reasoning.is_empty() {
            out.remove("reasoning");
        } else if !reasoning.contains_key("summary")
            && let Some(Value::Object(reasoning)) = out.get_mut("reasoning")
        {
            reasoning.insert("summary".into(), json!("auto"));
        }
    }

    // include: only encrypted reasoning is useful for multi-turn store:false
    if let Some(Value::Array(inc)) = out.get_mut("include") {
        inc.retain(|v| {
            v.as_str()
                .is_some_and(|s| s == "reasoning.encrypted_content")
        });
        let empty = out
            .get("include")
            .and_then(|v| v.as_array())
            .is_some_and(|a| a.is_empty());
        if empty {
            out.insert("include".into(), json!(["reasoning.encrypted_content"]));
        }
    } else {
        out.insert("include".into(), json!(["reasoning.encrypted_content"]));
    }

    // parallel_tool_calls default when tools present
    if out.contains_key("tools") && !out.contains_key("parallel_tool_calls") {
        out.insert("parallel_tool_calls".into(), json!(true));
    }
    if out.contains_key("tools") && !out.contains_key("tool_choice") {
        out.insert("tool_choice".into(), json!("auto"));
    }

    // Keep only function tools (Codex rejects hosted/xAI tool types).
    if let Some(Value::Array(tools)) = out.get_mut("tools") {
        tools.retain(|tool| {
            tool.get("type")
                .and_then(|t| t.as_str())
                .is_some_and(|t| t == "function")
                || tool.get("name").is_some() // EasyFunction form
        });
        for tool in tools.iter_mut() {
            if let Value::Object(t) = tool {
                t.retain(|_, v| !v.is_null());
                // Ensure type is present for bare function shapes
                if !t.contains_key("type") && t.contains_key("name") {
                    t.insert("type".into(), json!("function"));
                }
            }
        }
        if tools.is_empty() {
            out.remove("tools");
            out.remove("tool_choice");
            out.remove("parallel_tool_calls");
        }
    }

    Value::Object(out)
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
    fn detects_codex_with_existing_substring_rules() {
        assert_eq!(
            ResponsesBackend::detect("https://chatgpt.com/backend-api/codex"),
            ResponsesBackend::Codex
        );
        assert_eq!(
            ResponsesBackend::detect("https://proxy.example/backend-api/codex/v1"),
            ResponsesBackend::Codex
        );
        assert_eq!(
            ResponsesBackend::detect("https://api.x.ai/v1"),
            ResponsesBackend::Standard
        );
    }

    #[test]
    fn codex_header_boundary_strips_only_xai_private_metadata() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer provider-token".parse().unwrap());
        headers.insert("user-agent", "forge-test".parse().unwrap());
        headers.insert("traceparent", "00-test-trace".parse().unwrap());
        headers.insert("x-provider-feature", "enabled".parse().unwrap());
        headers.insert("x-grok-client-identifier", "forge".parse().unwrap());
        headers.insert("x-grok-conv-id", "conversation".parse().unwrap());
        headers.insert("x-xai-token-auth", "xai-grok-cli".parse().unwrap());

        let codex = ResponsesBackend::Codex.prepare_request_headers(headers.clone());
        assert!(codex.contains_key("authorization"));
        assert!(codex.contains_key("user-agent"));
        assert!(codex.contains_key("traceparent"));
        assert!(codex.contains_key("x-provider-feature"));
        assert!(!codex.contains_key("x-grok-client-identifier"));
        assert!(!codex.contains_key("x-grok-conv-id"));
        assert!(!codex.contains_key("x-xai-token-auth"));

        assert_eq!(
            ResponsesBackend::Standard.prepare_request_headers(headers.clone()),
            headers,
        );
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
        assert_eq!(out["store"], json!(false));
        assert_eq!(out["stream"], json!(true));
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
    fn retains_default_instructions_punctuation() {
        let out = ResponsesBackend::Codex.prepare_request_body(json!({"input": []}), false);
        assert_eq!(out["instructions"], "You are a helpful assistant.");
    }

    #[test]
    fn augments_only_codex_errors_with_detail_markers() {
        let message = "Bad Request (400)".to_string();
        assert_eq!(
            ResponsesBackend::Codex
                .augment_error_message(message.clone(), br#"{"detail":"Unsupported parameter"}"#),
            "Bad Request (400) — {\"detail\":\"Unsupported parameter\"}"
        );
        assert_eq!(
            ResponsesBackend::Codex.augment_error_message(message.clone(), b"plain failure"),
            message
        );
        assert_eq!(
            ResponsesBackend::Standard
                .augment_error_message("Bad Request (400)".to_string(), b"Unsupported parameter"),
            "Bad Request (400)"
        );
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
