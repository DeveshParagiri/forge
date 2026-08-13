//! Permission request selection, follow-up, cancellation, and queue draining.

use super::modes::{set_yolo_mode, set_yolo_mode_for_agent};
use crate::app::actions::Effect;
use crate::app::agent_view::AgentView;
use crate::app::app_view::{ActiveView, AppView};
use agent_client_protocol as acp;

// ---------------------------------------------------------------------------
// Permission dispatch
// ---------------------------------------------------------------------------

use crate::views::permission_view::{McpScope, PermissionFocus, PermissionViewState};
use xai_grok_workspace::permission::{BashCommandSelectedTerms, McpScopeSelection};

/// Free-form pattern taken from the editor on confirm.
pub(super) struct EditedPattern {
    pub pattern: String,
    /// True when the buffer was mutated — routes to `allowed_bash_globs`.
    pub is_glob: bool,
}

/// Take the free-form pattern-editor buffer, honoring it only when the resolved
/// request was in `PatternEdit` focus (and always clearing it, so an abandoned
/// edit can't leak into a later prompt).
pub(super) fn take_edited_pattern(
    agent: &mut AgentView,
    perm: &PermissionViewState,
) -> Option<EditedPattern> {
    agent
        .permission_pattern_edit
        .take()
        .filter(|_| perm.focus == PermissionFocus::PatternEdit)
        .and_then(|e| {
            e.trimmed().map(|s| EditedPattern {
                pattern: s.to_owned(),
                is_glob: e.is_dirty(),
            })
        })
}

/// Build the ACP response meta for a permission selection — the single source
/// of truth shared by the main and dashboard dispatch paths. MCP scope wins for
/// the `allow-always-mcp` id; otherwise a free-form edited pattern (glob when
/// dirty) wins over the arrow word-scope (literal prefix). `None` when there is
/// nothing to scope.
pub(super) fn build_selection_meta(
    perm: &PermissionViewState,
    option_id: &acp::PermissionOptionId,
    edited: Option<EditedPattern>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let obj = |v: serde_json::Value| v.as_object().cloned();

    if let Some(scope) = perm
        .mcp_scope
        .as_ref()
        .filter(|_| option_id.0.as_ref() == "allow-always-mcp")
    {
        let selection = match scope.selected {
            McpScope::Tool => McpScopeSelection::Tool {
                tool_name: scope.tool_name.clone(),
            },
            // Defensive: render disables Server when there is no prefix.
            McpScope::Server => match &scope.server_prefix {
                Some(prefix) => McpScopeSelection::Server {
                    server: prefix.clone(),
                },
                None => McpScopeSelection::Tool {
                    tool_name: scope.tool_name.clone(),
                },
            },
        };
        return serde_json::to_value(selection).ok().and_then(obj);
    }

    if let Some(edited) = edited.filter(|_| {
        // The editor authors an *allow* pattern, so apply it only to the bash
        // allow-always option. A different selection (reject-always, allow-once)
        // made while the editor is open must fall through to the arrow word-scope
        // — otherwise the allow text would land in the deny set.
        option_id.0.as_ref() == "allow-always-command" && perm.bash_highlights.is_some()
    }) {
        // Glob only when the editor is dirty. Unedited save is a literal grant
        // of the pre-filled command, so metacharacters that came from the
        // command itself (e.g. `find . -name *.rs`) stay literal.
        return serde_json::to_value(BashCommandSelectedTerms {
            command_parts: vec![edited.pattern],
            is_glob: edited.is_glob,
        })
        .ok()
        .and_then(obj);
    }

    if let Some(h) = perm
        .bash_highlights
        .as_ref()
        .filter(|_| perm.bash_selection_count > 0)
    {
        // Arrow word-scope: a literal command prefix, never a glob.
        return serde_json::to_value(BashCommandSelectedTerms {
            command_parts: h.highlighted_words[..perm.bash_selection_count].to_vec(),
            is_glob: false,
        })
        .ok()
        .and_then(obj);
    }

    None
}

/// Handle permission option selection (AllowOnce, AllowAlways, RejectAlways).
///
/// Pops the front request, sends the response, and handles queue transitions
/// (prompt restore on empty, prompt clear on next-front).
///
/// Special case for [`xai_grok_workspace::permission::ENABLE_ALWAYS_APPROVE_OPTION_ID`]:
/// when the user picks the prepended "Yes, and don't ask again for anything"
/// option, this dispatcher (a) sends the standard `Selected` response so the
/// in-flight request is allowed once (the shell's `map_selected_outcome`
/// resolves the id to `PromptOutcome::AllowOnce`), then (b) reuses the
/// existing `set_yolo_mode(true)` flow to flip the local YOLO state, drain
/// any remaining queued permissions, persist `[ui] permission_mode =
/// "always-approve"` to `~/.grok/config.toml`, and fire the
/// `x.ai/yolo_mode_changed` ACP notification. See the option-id constant
/// doc-comment for the full client/shell split. Under a managed-policy
/// pin step (b) is refused with a toast — the request is still allowed once.
pub(super) fn dispatch_permission_select(
    app: &mut AppView,
    option_id: acp::PermissionOptionId,
) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    dispatch_permission_select_for_agent(app, id, None, option_id, true).unwrap_or_default()
}

/// Resolve the front permission card for a session-pinned remote client.
/// The expected tool-call id is checked before consuming the response sender,
/// which makes a terminal-first/phone-first race deterministic and harmless.
pub(crate) fn dispatch_remote_permission_select(
    app: &mut AppView,
    id: crate::app::agent::AgentId,
    expected_tool_call_id: &str,
    option_id: acp::PermissionOptionId,
) -> Result<Vec<Effect>, String> {
    dispatch_permission_select_for_agent(app, id, Some(expected_tool_call_id), option_id, false)
}

fn dispatch_permission_select_for_agent(
    app: &mut AppView,
    id: crate::app::agent::AgentId,
    expected_tool_call_id: Option<&str>,
    option_id: acp::PermissionOptionId,
    use_terminal_pattern: bool,
) -> Result<Vec<Effect>, String> {
    let Some(agent) = app.agents.get_mut(&id) else {
        return Err("The armed Forge session is no longer available.".into());
    };
    let Some(front) = agent.permission_queue.front() else {
        return Err("This interaction was already resolved.".into());
    };
    if let Some(expected) = expected_tool_call_id
        && front.request.request.tool_call.tool_call_id.0.as_ref() != expected
    {
        return Err("This interaction was already resolved.".into());
    }
    if !front
        .options
        .iter()
        .any(|option| option.option_id == option_id)
    {
        return Err("That permission option is not available.".into());
    }
    let Some(perm) = agent.permission_queue.pop_front() else {
        return Err("This interaction was already resolved.".into());
    };

    let edited_pattern = if use_terminal_pattern {
        take_edited_pattern(agent, &perm)
    } else {
        // A phone choice must not accidentally consume a half-edited pattern
        // from the laptop composer. The remote UI exposes only the advertised
        // immutable options in its first implementation.
        agent.permission_pattern_edit = None;
        None
    };

    // Detect the "enable always-approve mode" id BEFORE moving option_id
    // into the response. Cheap str compare on the `Arc<str>` interior.
    let enable_always_approve =
        option_id.0.as_ref() == xai_grok_workspace::permission::ENABLE_ALWAYS_APPROVE_OPTION_ID;

    // Remember the user's choice (by option kind) so the next prompt's cursor
    // sticks to it. Allow-flavored choices only — a rejection must not steer a
    // later prompt's cursor onto a reject row. Also skip the two options that
    // aren't per-prompt choices:
    //  - the global always-approve (YOLO) option flips global auto-approve, so
    //    there will be no subsequent prompt to land on;
    //  - "allow all edits during this session" is edit-scoped (kind
    //    `AllowAlways`) — letting it stick would steer an unrelated later
    //    prompt onto its "always allow this command" row, escalating scope.
    let steers_next_cursor = !enable_always_approve
        && option_id.0.as_ref() != xai_grok_workspace::permission::ALLOW_EDITS_SESSION_OPTION_ID;
    if steers_next_cursor
        && let Some(kind) = perm
            .options
            .iter()
            .find(|o| o.option_id == option_id)
            .map(|o| o.kind)
        && matches!(
            kind,
            acp::PermissionOptionKind::AllowOnce | acp::PermissionOptionKind::AllowAlways
        )
    {
        crate::appearance::permission_cursor::set_last_used_permission(
            crate::appearance::permission_cursor::DefaultSelectedPermission::from_kind(&kind),
        );
    }

    let meta = build_selection_meta(&perm, &option_id, edited_pattern);

    let response_sent = perm
        .request
        .response_tx
        .send(Ok(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(option_id)),
        )
        .meta(meta)))
        .is_ok();

    // Queue transition: restore prompt if queue is now empty, clear if next-front.
    resolve_permission_queue_transition(agent);
    if !response_sent {
        return if expected_tool_call_id.is_some() {
            Err("This interaction was already resolved.".into())
        } else {
            Ok(vec![])
        };
    }

    // "Enable always-approve" side effect: flip YOLO + persist + notify.
    // Reuses the existing `set_yolo_mode` pipeline so telemetry, queue
    // drain, toast, modal refresh, config persistence, and ACP
    // notification all flow through one well-tested code path.
    //
    // Idempotency: if YOLO is already on, the pager auto-approves in
    // `handle_permission_request` before the panel is shown, so the
    // user couldn't have selected this option. The `is_yolo()` guard
    // is defensive — a redundant call would re-emit the toast and a
    // duplicate `PersistPermissionMode` effect, but is otherwise safe.
    if enable_always_approve {
        let already_on = app
            .agents
            .get(&id)
            .map(|a| a.session.is_yolo())
            .unwrap_or(false);
        if !already_on {
            return Ok(if expected_tool_call_id.is_some() {
                set_yolo_mode_for_agent(app, id, true)
            } else {
                set_yolo_mode(app, true)
            });
        }
    }

    Ok(vec![])
}

/// Handle permission followup message (RejectOnce with user-typed text).
pub(super) fn dispatch_permission_followup(app: &mut AppView, text: String) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    dispatch_permission_followup_for_agent(app, id, None, text).unwrap_or_default()
}

pub(crate) fn dispatch_remote_permission_followup(
    app: &mut AppView,
    id: crate::app::agent::AgentId,
    expected_tool_call_id: &str,
    text: String,
) -> Result<Vec<Effect>, String> {
    dispatch_permission_followup_for_agent(app, id, Some(expected_tool_call_id), text)
}

fn dispatch_permission_followup_for_agent(
    app: &mut AppView,
    id: crate::app::agent::AgentId,
    expected_tool_call_id: Option<&str>,
    text: String,
) -> Result<Vec<Effect>, String> {
    let Some(agent) = app.agents.get_mut(&id) else {
        return Err("The armed Forge session is no longer available.".into());
    };
    let Some(front) = agent.permission_queue.front() else {
        return Err("This interaction was already resolved.".into());
    };
    if let Some(expected) = expected_tool_call_id
        && front.request.request.tool_call.tool_call_id.0.as_ref() != expected
    {
        return Err("This interaction was already resolved.".into());
    }

    // Find the RejectOnce option.
    let option_id = front
        .options
        .iter()
        .find(|o| o.kind == acp::PermissionOptionKind::RejectOnce)
        .map(|o| o.option_id.clone());

    let Some(option_id) = option_id else {
        return Err("This permission does not accept a follow-up message.".into());
    };
    let Some(perm) = agent.permission_queue.pop_front() else {
        return Err("This interaction was already resolved.".into());
    };

    // Include followup message in meta.
    let meta = if !text.trim().is_empty() {
        serde_json::json!({
            "followup_message": text,
        })
        .as_object()
        .cloned()
    } else {
        None
    };

    let response_sent = perm
        .request
        .response_tx
        .send(Ok(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(option_id)),
        )
        .meta(meta)))
        .is_ok();

    resolve_permission_queue_transition(agent);
    if response_sent || expected_tool_call_id.is_none() {
        Ok(vec![])
    } else {
        Err("This interaction was already resolved.".into())
    }
}

/// Handle permission cancel (Ctrl-C / Esc — cancels front request only).
pub(super) fn dispatch_permission_cancel(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    dispatch_permission_cancel_for_agent(app, id, None).unwrap_or_default()
}

pub(crate) fn dispatch_remote_permission_cancel(
    app: &mut AppView,
    id: crate::app::agent::AgentId,
    expected_tool_call_id: &str,
) -> Result<Vec<Effect>, String> {
    dispatch_permission_cancel_for_agent(app, id, Some(expected_tool_call_id))
}

fn dispatch_permission_cancel_for_agent(
    app: &mut AppView,
    id: crate::app::agent::AgentId,
    expected_tool_call_id: Option<&str>,
) -> Result<Vec<Effect>, String> {
    let Some(agent) = app.agents.get_mut(&id) else {
        return Err("The armed Forge session is no longer available.".into());
    };
    let Some(front) = agent.permission_queue.front() else {
        return Err("This interaction was already resolved.".into());
    };
    if let Some(expected) = expected_tool_call_id
        && front.request.request.tool_call.tool_call_id.0.as_ref() != expected
    {
        return Err("This interaction was already resolved.".into());
    }
    let Some(perm) = agent.permission_queue.pop_front() else {
        return Err("This interaction was already resolved.".into());
    };

    let response_sent = perm
        .request
        .response_tx
        .send(Ok(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Cancelled,
        )))
        .is_ok();

    resolve_permission_queue_transition(agent);
    if response_sent || expected_tool_call_id.is_none() {
        Ok(vec![])
    } else {
        Err("This interaction was already resolved.".into())
    }
}

/// Drain all queued permission requests, sending `Cancelled` to each.
///
/// Called on turn-end and turn-cancel. After draining, restores stashed
/// prompt/pane. Distinct from `dispatch_permission_cancel` (front only).
pub(super) fn drain_permission_queue(agent: &mut AgentView) {
    agent.last_permission_click = None;
    if agent.permission_queue.is_empty() {
        return;
    }
    for perm in agent.permission_queue.drain(..) {
        perm.request
            .response_tx
            .send(Ok(acp::RequestPermissionResponse::new(
                acp::RequestPermissionOutcome::Cancelled,
            )))
            .ok();
    }
    restore_permission_stashes(agent);
}

/// Handle queue transition after resolving (select/followup/cancel) the front
/// permission request.
///
/// - Queue now empty → restore stashed prompt/pane.
/// - Queue still has items → clear prompt text and reset next front to Options.
pub(crate) fn resolve_permission_queue_transition(agent: &mut AgentView) {
    agent.last_permission_click = None;
    // The pattern editor is front-request scoped: drop any buffer when the
    // front request is resolved (covers cancel/followup/select paths).
    agent.permission_pattern_edit = None;
    if agent.permission_queue.is_empty() {
        restore_permission_stashes(agent);
    } else {
        // Clear any followup text from the just-resolved permission so it
        // doesn't leak into the next permission's UI.
        agent.prompt.set_text("");
        // Reset next front's focus to Options.
        if let Some(next) = agent.permission_queue.front_mut() {
            next.focus = crate::views::permission_view::PermissionFocus::Options;
        }
    }
}

/// Restore composer + pane stashes when the permission queue empties.
pub(super) fn restore_permission_stashes(agent: &mut AgentView) {
    agent.permission_pattern_edit = None;
    if let Some(stashed) = agent.permission_stashed_prompt.take() {
        agent.prompt.restore(stashed);
    }
    if let Some(pane) = agent.permission_stashed_pane.take() {
        agent.set_active_pane(pane, true);
    }
}
