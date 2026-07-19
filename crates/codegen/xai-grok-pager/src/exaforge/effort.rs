//! Shift+Tab reasoning-effort cycle and quiet model-switch feedback.

use crate::app::actions::Effect;
use crate::app::app_view::{ActiveView, AppView};

/// Shift+Tab cycles reasoning effort on the active model.
///
/// Steps through the model's offered effort menu (low → medium → high → …)
/// via the same `SwitchModel` path as `/effort`. Models without reasoning
/// effort support get a short toast and no-op.
pub(crate) fn dispatch_cycle_effort(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    let Some(model_id) = agent.session.models.current.clone() else {
        agent.show_toast("No active model — pick one with /model first.");
        return vec![];
    };
    let options = agent.session.models.reasoning_effort_options_for(&model_id);
    if options.is_empty() {
        agent.show_toast("This model does not support reasoning effort.");
        return vec![];
    }
    let current = agent.session.models.reasoning_effort;
    let idx = current
        .and_then(|c| options.iter().position(|o| o.value == c))
        .unwrap_or(0);
    let next = options[(idx + 1) % options.len()].clone();
    let effort = next.value;
    // Quiet: only a short toast — no scrollback spam, no mode banner.
    agent.show_toast(&format!("effort · {}", next.id));
    // Mirror Action::SwitchModel: defer until session exists.
    let Some(session_id) = agent.session.session_id.clone() else {
        agent.session.deferred_model_switch = Some((model_id, Some(effort)));
        return vec![];
    };
    agent.session.model_switch_pending = true;
    vec![Effect::SwitchModel {
        agent_id: id,
        session_id,
        model_id,
        effort: Some(effort),
        prev_model_id: None,
    }]
}

/// Whether a completed model switch should push a scrollback system line.
///
/// Effort-only changes (Shift+Tab / `/effort`) stay quiet; full model switches
/// still get a system line. Unchanged switches never log.
pub(crate) fn should_log_model_switch_line(same_model: bool, unchanged: bool) -> bool {
    !unchanged && !same_model
}
