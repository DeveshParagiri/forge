use std::collections::BTreeMap;

use xai_grok_sampling_types::{ConversationItem, ToolCall, rs};

#[derive(Debug, Clone)]
struct StreamedFunctionCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Debug, Default)]
pub(crate) struct ResponsesTerminalRecovery {
    text: String,
    function_calls: BTreeMap<u32, StreamedFunctionCall>,
}

impl ResponsesTerminalRecovery {
    pub(crate) fn observe_text_delta(&mut self, delta: &str) {
        self.text.push_str(delta);
    }

    pub(crate) fn observe_function_call_added(
        &mut self,
        output_index: u32,
        call: &rs::FunctionToolCall,
    ) {
        self.function_calls.insert(
            output_index,
            StreamedFunctionCall {
                id: call.call_id.clone(),
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            },
        );
    }

    pub(crate) fn observe_function_call_arguments_delta(&mut self, output_index: u32, delta: &str) {
        if let Some(call) = self.function_calls.get_mut(&output_index) {
            call.arguments.push_str(delta);
        }
    }

    pub(crate) fn observe_function_call_arguments_done(
        &mut self,
        output_index: u32,
        name: Option<String>,
        arguments: String,
    ) {
        if let Some(call) = self.function_calls.get_mut(&output_index) {
            call.arguments = arguments;
            if let Some(name) = name {
                call.name = name;
            }
        }
    }

    pub(crate) fn observe_function_call_done(
        &mut self,
        output_index: u32,
        call: &rs::FunctionToolCall,
    ) {
        self.function_calls.insert(
            output_index,
            StreamedFunctionCall {
                id: call.call_id.clone(),
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            },
        );
    }

    /// Recover terminal content from streaming events without depending on the
    /// backend that emitted them. Existing terminal text wins, and streamed
    /// calls are de-duplicated by call id.
    pub(crate) fn apply(self, items: &mut Vec<ConversationItem>) {
        if !self.function_calls.is_empty() {
            let fallback_calls = self.function_calls.into_values().map(|call| ToolCall {
                id: std::sync::Arc::<str>::from(call.id),
                name: call.name,
                arguments: std::sync::Arc::<str>::from(call.arguments),
            });
            if let Some(ConversationItem::Assistant(assistant)) = items
                .iter_mut()
                .rev()
                .find(|item| matches!(item, ConversationItem::Assistant(_)))
            {
                for call in fallback_calls {
                    if !assistant
                        .tool_calls
                        .iter()
                        .any(|existing| existing.id == call.id)
                    {
                        assistant.tool_calls.push(call);
                    }
                }
            } else {
                items.push(ConversationItem::assistant_tool_calls(
                    fallback_calls.collect(),
                ));
            }
        }

        if !self.text.is_empty() {
            if let Some(ConversationItem::Assistant(assistant)) = items
                .iter_mut()
                .rev()
                .find(|item| matches!(item, ConversationItem::Assistant(_)))
            {
                if assistant.content.is_empty() {
                    assistant.content = std::sync::Arc::<str>::from(self.text);
                }
            } else {
                items.push(ConversationItem::assistant(self.text));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function_call(call_id: &str, name: &str, arguments: &str) -> rs::FunctionToolCall {
        rs::FunctionToolCall {
            arguments: arguments.into(),
            call_id: call_id.into(),
            name: name.into(),
            id: None,
            status: None,
        }
    }

    #[test]
    fn recovers_streamed_text_when_terminal_items_are_empty() {
        let mut recovery = ResponsesTerminalRecovery::default();
        recovery.observe_text_delta("hello");
        recovery.observe_text_delta(" world");
        let mut items = Vec::new();

        recovery.apply(&mut items);

        let ConversationItem::Assistant(assistant) = &items[0] else {
            panic!("expected assistant item");
        };
        assert_eq!(assistant.content.as_ref(), "hello world");
    }

    #[test]
    fn preserves_existing_terminal_text() {
        let mut recovery = ResponsesTerminalRecovery::default();
        recovery.observe_text_delta("streamed");
        let mut items = vec![ConversationItem::assistant("terminal")];

        recovery.apply(&mut items);

        let ConversationItem::Assistant(assistant) = &items[0] else {
            panic!("expected assistant item");
        };
        assert_eq!(assistant.content.as_ref(), "terminal");
    }

    #[test]
    fn recovers_and_deduplicates_streamed_function_calls() {
        let mut recovery = ResponsesTerminalRecovery::default();
        recovery.observe_function_call_added(0, &function_call("call_1", "bash", ""));
        recovery.observe_function_call_arguments_delta(0, "{\"command\":");
        recovery.observe_function_call_arguments_delta(0, "\"pwd\"}");
        let existing = ToolCall {
            id: std::sync::Arc::<str>::from("call_1"),
            name: "bash".into(),
            arguments: std::sync::Arc::<str>::from("{\"command\":\"pwd\"}"),
        };
        let mut items = vec![ConversationItem::assistant_tool_calls(vec![existing])];

        recovery.apply(&mut items);

        let ConversationItem::Assistant(assistant) = &items[0] else {
            panic!("expected assistant item");
        };
        assert_eq!(assistant.tool_calls.len(), 1);
    }

    #[test]
    fn completed_call_recovers_without_added_event() {
        let mut recovery = ResponsesTerminalRecovery::default();
        recovery.observe_function_call_done(
            1,
            &function_call("call_done", "run_terminal_command", "{\"command\":\"pwd\"}"),
        );
        let mut items = Vec::new();

        recovery.apply(&mut items);

        let ConversationItem::Assistant(assistant) = &items[0] else {
            panic!("expected assistant item");
        };
        assert_eq!(assistant.tool_calls[0].id.as_ref(), "call_done");
        assert_eq!(assistant.tool_calls[0].name, "run_terminal_command");
        assert_eq!(
            assistant.tool_calls[0].arguments.as_ref(),
            "{\"command\":\"pwd\"}"
        );
    }
}
