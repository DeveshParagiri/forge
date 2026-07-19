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

/// Reshape a Responses API JSON body for ChatGPT Codex
/// (`chatgpt.com/backend-api/codex/responses`), matching Pi's
/// `openai-codex-responses` contract.
///
/// Codex rejects (400) parameters that api.openai.com / api.x.ai accept:
/// `temperature`, `top_p`, `max_output_tokens`, `truncation`, `background`,
/// `metadata`, `stream_tool_calls`, and **system messages in `input`**
/// (use `instructions` instead). Also requires `store: false` and `stream: true`.
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
            // Drop empty items
            filtered_input.push(item.clone());
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
}
