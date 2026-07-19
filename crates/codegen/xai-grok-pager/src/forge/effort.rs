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
    let next_idx = current
        .and_then(|c| options.iter().position(|o| o.value == c))
        .map_or(0, |idx| (idx + 1) % options.len());
    let next = options[next_idx].clone();
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
        fast_mode: None,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agent_client_protocol as acp;
    use xai_grok_shell::sampling::types::ReasoningEffort;

    use super::*;
    use crate::app::app_view::tests::test_app_with_agent;

    fn app_with_effort_menu(
        current: Option<ReasoningEffort>,
        has_session: bool,
    ) -> (AppView, acp::ModelId) {
        let mut app = test_app_with_agent();
        let ActiveView::Agent(agent_id) = app.active_view else {
            panic!("test app must have an active agent");
        };
        let model_id = acp::ModelId::new(Arc::from("effort-model"));
        let info = acp::ModelInfo::new(model_id.clone(), "Effort model".to_owned()).meta(
            serde_json::json!({
                "supportsReasoningEffort": true,
                "reasoningEfforts": [
                    { "id": "low", "value": "low", "label": "Low" },
                    { "id": "medium", "value": "medium", "label": "Medium" },
                    { "id": "high", "value": "high", "label": "High" }
                ]
            })
            .as_object()
            .cloned(),
        );
        let agent = app.agents.get_mut(&agent_id).expect("active agent");
        agent
            .session
            .models
            .available
            .insert(model_id.clone(), info);
        agent.session.models.current = Some(model_id.clone());
        agent.session.models.reasoning_effort = current;
        if !has_session {
            agent.session.session_id = None;
        }
        (app, model_id)
    }

    fn switched_effort(effects: &[Effect]) -> Option<ReasoningEffort> {
        match effects {
            [Effect::SwitchModel { effort, .. }] => *effort,
            _ => None,
        }
    }

    #[test]
    fn absent_effort_selects_first_option_without_mutating_permission_mode() {
        let (mut app, _) = app_with_effort_menu(None, true);
        let ActiveView::Agent(agent_id) = app.active_view else {
            unreachable!();
        };
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            agent.session.yolo_mode = true;
            agent.session.auto_mode = false;
            agent.plan_mode_pending = Some(false);
        }

        let effects = dispatch_cycle_effort(&mut app);

        assert_eq!(switched_effort(&effects), Some(ReasoningEffort::Low));
        let agent = &app.agents[&agent_id];
        assert!(agent.session.model_switch_pending);
        assert!(agent.session.yolo_mode);
        assert!(!agent.session.auto_mode);
        assert_eq!(agent.plan_mode_pending, Some(false));
        assert_eq!(
            agent.toast.as_ref().map(|toast| toast.0.as_str()),
            Some("effort · low")
        );
    }

    #[test]
    fn known_effort_advances_and_last_option_wraps() {
        for (current, expected) in [
            (ReasoningEffort::Low, ReasoningEffort::Medium),
            (ReasoningEffort::Medium, ReasoningEffort::High),
            (ReasoningEffort::High, ReasoningEffort::Low),
        ] {
            let (mut app, _) = app_with_effort_menu(Some(current), true);
            assert_eq!(
                switched_effort(&dispatch_cycle_effort(&mut app)),
                Some(expected),
                "current effort {current:?}",
            );
        }
    }

    #[test]
    fn sessionless_cycle_defers_the_first_option() {
        let (mut app, model_id) = app_with_effort_menu(None, false);
        let ActiveView::Agent(agent_id) = app.active_view else {
            unreachable!();
        };

        assert!(dispatch_cycle_effort(&mut app).is_empty());
        assert_eq!(
            app.agents[&agent_id].session.deferred_model_switch,
            Some((model_id, Some(ReasoningEffort::Low)))
        );
        assert!(!app.agents[&agent_id].session.model_switch_pending);
    }

    #[test]
    fn missing_or_unsupported_model_is_a_safe_noop() {
        let mut no_model = test_app_with_agent();
        let ActiveView::Agent(no_model_id) = no_model.active_view else {
            unreachable!();
        };
        assert!(dispatch_cycle_effort(&mut no_model).is_empty());
        assert_eq!(
            no_model.agents[&no_model_id]
                .toast
                .as_ref()
                .map(|toast| toast.0.as_str()),
            Some("No active model — pick one with /model first.")
        );

        let (mut unsupported, _) = app_with_effort_menu(None, true);
        let ActiveView::Agent(unsupported_id) = unsupported.active_view else {
            unreachable!();
        };
        let model = unsupported.agents[&unsupported_id]
            .session
            .models
            .current
            .clone()
            .unwrap();
        unsupported
            .agents
            .get_mut(&unsupported_id)
            .unwrap()
            .session
            .models
            .available[&model]
            .meta = None;
        assert!(dispatch_cycle_effort(&mut unsupported).is_empty());
        assert_eq!(
            unsupported.agents[&unsupported_id]
                .toast
                .as_ref()
                .map(|toast| toast.0.as_str()),
            Some("This model does not support reasoning effort.")
        );
    }
}
