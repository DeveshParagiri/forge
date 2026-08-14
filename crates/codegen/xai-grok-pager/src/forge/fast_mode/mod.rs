//! Forge-owned `/fast` feature integration.
//!
//! The command, capability checks, UI state transitions, and active-model
//! decoration live here. Upstream pager modules should contain only narrow
//! registration and dispatch hooks.

use crate::acp::model_state::ModelState;
use crate::app::actions::{Action, Effect};
use crate::app::app_view::{ActiveView, AppView};
use crate::scrollback::RenderBlock;
use crate::slash::command::{AppCtx, CommandExecCtx, CommandResult, SlashCommand};

/// Whether the active model explicitly advertises fast inference support.
pub(crate) fn is_supported(models: &ModelState) -> bool {
    models
        .current
        .as_ref()
        .and_then(|id| models.available.get(id))
        .map(|info| xai_grok_shell::sampling::types::supports_fast_mode_meta(info.meta.as_ref()))
        .unwrap_or(false)
}

/// Reconcile session state after catalog refreshes or model switches.
pub(crate) fn reconcile(models: &mut ModelState) {
    if !is_supported(models) {
        models.fast_mode = false;
        models.cancel_fast_mode_change();
    }
}

/// Decorate the canonical primary model label when fast mode is active.
pub(crate) fn decorate_model_label(label: String, enabled: bool) -> String {
    if enabled {
        format!("⚡︎ {label}")
    } else {
        label
    }
}

/// Narrow action-dispatch hook used by the stock router.
pub(crate) fn dispatch_set_fast_mode(app: &mut AppView, enabled: bool) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    match dispatch_set_fast_mode_for_agent(app, id, enabled) {
        Ok(effects) => effects,
        Err(message) => {
            if let Some(agent) = app.agents.get_mut(&id) {
                agent.scrollback.push_block(RenderBlock::system(message));
            }
            vec![]
        }
    }
}

/// Start the canonical shell mutation for one exact pager-owned agent.
/// Remote callers use this instead of changing `active_view`, so a phone
/// command can never spill into the terminal tab that happens to be visible.
pub(crate) fn dispatch_set_fast_mode_for_agent(
    app: &mut AppView,
    id: crate::app::agent::AgentId,
    enabled: bool,
) -> Result<Vec<Effect>, String> {
    let Some(agent) = app.agents.get_mut(&id) else {
        return Err("The Forge session is no longer available.".into());
    };
    let Some(session_id) = agent.session.session_id.clone() else {
        return Err("Fast mode requires an active session".into());
    };
    let Some(request_id) = agent.session.models.begin_fast_mode_change() else {
        return Err("A Fast Mode change is already in progress.".into());
    };
    let session_binding_epoch = agent.session_binding_epoch;
    // Capability is deliberately not checked against the pager mirror here.
    // That mirror may lag a model switch; the shell validates the request
    // against the authoritative live sampling model under the session lock.
    Ok(vec![Effect::SetFastMode {
        agent_id: id,
        session_id,
        session_binding_epoch,
        request_id,
        enabled,
    }])
}

/// Apply the authoritative result of the session-only Fast Mode mutation.
pub(crate) fn handle_complete(
    app: &mut AppView,
    agent_id: crate::app::agent::AgentId,
    session_id: agent_client_protocol::SessionId,
    session_binding_epoch: u32,
    request_id: u64,
    enabled: bool,
    result: Result<(), String>,
) -> Vec<Effect> {
    let Some(agent) = app.agents.get_mut(&agent_id) else {
        return vec![];
    };
    if agent.session.session_id.as_ref() != Some(&session_id)
        || agent.session_binding_epoch != session_binding_epoch
        || !agent.session.models.finish_fast_mode_change(request_id)
    {
        return vec![];
    }
    match result {
        Ok(()) => {
            agent.session.models.fast_mode = enabled;
            agent.scrollback.push_block(RenderBlock::system(format!(
                "Fast mode {}",
                if enabled { "enabled" } else { "disabled" }
            )));
        }
        Err(message) => {
            agent.scrollback.push_block(RenderBlock::system(format!(
                "Couldn't set fast mode: {message}"
            )));
        }
    }
    vec![]
}

/// Toggle fast inference for the active model/session.
pub(crate) struct FastCommand;

impl SlashCommand for FastCommand {
    fn name(&self) -> &str {
        "fast"
    }

    fn description(&self) -> &str {
        "Toggle fast mode for the current model"
    }

    fn usage(&self) -> &str {
        "/fast"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn visible(&self, ctx: &AppCtx) -> bool {
        is_supported(ctx.models)
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        if !args.trim().is_empty() {
            return CommandResult::Error("Usage: /fast".into());
        }
        // Dispatch even if the mirrored model still looks unsupported. A model
        // switch can already be in flight; the shell checks the authoritative
        // live model before changing Fast Mode.
        CommandResult::Action(Action::SetFastMode(!ctx.models.fast_mode))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol as acp;
    use std::sync::Arc;

    static EMPTY_BUNDLE: crate::app::bundle::BundleState = crate::app::bundle::BundleState {
        has_cache: false,
        version: String::new(),
        personas: Vec::new(),
        roles: Vec::new(),
        agents: Vec::new(),
        skills: Vec::new(),
        persona_details: Vec::new(),
        role_details: Vec::new(),
    };

    fn state(supports_fast_mode: bool, enabled: bool) -> ModelState {
        let id = acp::ModelId::new(Arc::from("model"));
        let info = acp::ModelInfo::new(id.clone(), "Model".to_owned()).meta(
            supports_fast_mode
                .then(|| serde_json::json!({ "supportsFastMode": true }))
                .and_then(|value| value.as_object().cloned()),
        );
        let mut models = ModelState::default();
        models.available.insert(id.clone(), info);
        models.current = Some(id);
        models.fast_mode = enabled;
        models
    }

    fn exec_ctx(models: &ModelState) -> CommandExecCtx<'_> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: &EMPTY_BUNDLE,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot::default(),
        }
    }

    #[test]
    fn supported_model_toggles_fast_mode_on_and_off() {
        let command = FastCommand;
        let disabled = state(true, false);
        let mut ctx = exec_ctx(&disabled);
        assert!(matches!(
            command.run(&mut ctx, ""),
            CommandResult::Action(Action::SetFastMode(true))
        ));

        let enabled = state(true, true);
        let mut ctx = exec_ctx(&enabled);
        assert!(matches!(
            command.run(&mut ctx, ""),
            CommandResult::Action(Action::SetFastMode(false))
        ));
    }

    #[test]
    fn unsupported_mirror_is_hidden_but_dispatches_for_authoritative_validation() {
        let command = FastCommand;
        let mut models = state(false, true);
        reconcile(&mut models);
        assert!(!models.fast_mode);
        let app_ctx = AppCtx {
            models: &models,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            usage_command_visible: true,
            workflows_available: true,
            screen_mode: crate::app::ScreenMode::Inline,
        };
        assert!(!command.visible(&app_ctx));
        let mut exec_ctx = exec_ctx(&models);
        assert!(matches!(
            command.run(&mut exec_ctx, ""),
            CommandResult::Action(Action::SetFastMode(true))
        ));
    }

    #[test]
    fn lightning_follows_actual_state() {
        assert_eq!(decorate_model_label("Codex".into(), true), "⚡︎ Codex");
        assert_eq!(decorate_model_label("Codex".into(), false), "Codex");
    }
}
