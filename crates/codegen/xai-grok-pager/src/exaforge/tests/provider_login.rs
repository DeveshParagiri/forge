//! Provider-login interaction/render tests (moved out of agent_view/interactions).

use crate::actions::ActionRegistry;
use crate::app::agent_view::AgentView;
use crate::app::agent_view::test_fixtures::make_agent;
use crate::app::app_view::InputOutcome;
use crate::exaforge::provider_login::{
    FooterLeftPolicy, enter_label, esc_cancels_direct_input, esc_cancels_provider_dialog,
    footer_left_policy, is_direct_input_kind, translate_provider_login_submit,
};
use crate::views::prompt_widget::StashedPrompt;
use crate::views::question_view::{
    LocalQuestionKind, QuestionFocus, QuestionSelection, QuestionViewState,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use xai_grok_tools::implementations::grok_build::ask_user_question::{Question, QuestionOption};

fn draw_frame(agent: &mut AgentView) -> Buffer {
    let area = Rect::new(0, 0, 80, 30);
    let reg = ActionRegistry::defaults();
    let bundle = crate::app::bundle::BundleState::default();
    let mut buf = Buffer::empty(area);
    let mut scratch = crate::scrollback::render::ScratchBuffer::new();
    agent.last_terminal_size = (80, 30);
    agent.draw(
        area,
        &mut buf,
        &reg,
        &mut scratch,
        None,
        false,
        0,
        &[],
        &std::collections::BTreeSet::new(),
        None,
        &bundle,
        false,
        &mut Vec::new(),
        false,
        false,
        None,
    );
    buf
}

#[test]
fn predicates_cover_provider_surfaces() {
    assert!(is_direct_input_kind(Some(
        &LocalQuestionKind::OpenRouterApiKey
    )));
    assert!(!is_direct_input_kind(Some(
        &LocalQuestionKind::ProviderLogin
    )));
    assert!(esc_cancels_direct_input(Some(
        &LocalQuestionKind::OpenRouterApiKey
    )));
    assert!(esc_cancels_provider_dialog(Some(
        &LocalQuestionKind::ProviderLogin
    )));
    assert!(esc_cancels_provider_dialog(Some(
        &LocalQuestionKind::OpenRouterApiKey
    )));
    assert!(!esc_cancels_provider_dialog(Some(
        &LocalQuestionKind::NewSession
    )));
    assert_eq!(
        footer_left_policy(Some(&LocalQuestionKind::OpenRouterApiKey)),
        FooterLeftPolicy::DirectInput
    );
    assert_eq!(
        footer_left_policy(Some(&LocalQuestionKind::ProviderLogin)),
        FooterLeftPolicy::ProviderPicker
    );
    assert_eq!(footer_left_policy(None), FooterLeftPolicy::Standard);
    assert_eq!(enter_label(true, false, true), "submit");
    assert_eq!(enter_label(false, true, false), "edit");
    assert_eq!(enter_label(false, false, true), "submit");
    assert_eq!(enter_label(false, false, false), "select");
}

#[test]
fn translate_openrouter_and_picker_submit() {
    let mut or_state = QuestionViewState::new(
        "openrouter-key".into(),
        vec![Question {
            question: "Enter OpenRouter API key".into(),
            options: vec![],
            multi_select: Some(false),
            id: None,
        }],
        StashedPrompt::default(),
    )
    .with_local_kind(LocalQuestionKind::OpenRouterApiKey);
    or_state.per_question_freeform[0] = "sk-or-test".into();
    let outcome =
        translate_provider_login_submit(&or_state, &LocalQuestionKind::OpenRouterApiKey).unwrap();
    assert!(matches!(
        outcome,
        InputOutcome::Action(crate::app::actions::Action::OpenRouterKeySubmitted { api_key })
            if api_key == "sk-or-test"
    ));

    let mut picker = QuestionViewState::new(
        "provider-login".into(),
        vec![Question {
            question: "Configure provider".into(),
            options: vec![QuestionOption {
                label: "OpenRouter".into(),
                description: "API key · configured".into(),
                preview: None,
                id: Some("openrouter".into()),
            }],
            multi_select: Some(false),
            id: None,
        }],
        StashedPrompt::default(),
    )
    .with_local_kind(LocalQuestionKind::ProviderLogin)
    .with_no_freeform();
    picker.selections[0] = QuestionSelection::Single(Some(0));
    let outcome =
        translate_provider_login_submit(&picker, &LocalQuestionKind::ProviderLogin).unwrap();
    assert!(matches!(
        outcome,
        InputOutcome::Action(crate::app::actions::Action::ProviderLoginSelected { provider_id })
            if provider_id == "openrouter"
    ));
}

#[test]
fn provider_picker_escape_cancels_in_one_press_and_restores_draft() {
    let mut agent = make_agent();
    agent.prompt.set_text("keep this draft");
    let stashed = agent.prompt.stash();
    let state = QuestionViewState::new(
        "provider-login".into(),
        vec![Question {
            question: "Configure provider".into(),
            options: vec![QuestionOption {
                label: "OpenAI Codex".into(),
                description: "ChatGPT Plus/Pro · configured".into(),
                preview: None,
                id: Some("openai-codex".into()),
            }],
            multi_select: Some(false),
            id: None,
        }],
        stashed,
    )
    .with_local_kind(LocalQuestionKind::ProviderLogin)
    .with_no_freeform();
    agent.question_view = Some(state);

    let outcome = agent.handle_question_key(&KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(outcome, InputOutcome::Changed));
    assert!(agent.question_view.is_none());
    assert_eq!(agent.prompt.text(), "keep this draft");
}

#[test]
fn provider_picker_renders_only_compact_configuration_status() {
    let mut agent = make_agent();
    let option = |label: &str, description: &str, id: &str| QuestionOption {
        label: label.into(),
        description: description.into(),
        preview: None,
        id: Some(id.into()),
    };
    agent.question_view = Some(
        QuestionViewState::new(
            "provider-login".into(),
            vec![Question {
                question: "Configure provider".into(),
                options: vec![
                    option("SpaceXAI", "Grok subscription · configured", "spacexai"),
                    option(
                        "OpenAI Codex",
                        "ChatGPT Plus/Pro · configured",
                        "openai-codex",
                    ),
                    option("OpenRouter", "API key · configured", "openrouter"),
                ],
                multi_select: Some(false),
                id: None,
            }],
            StashedPrompt::default(),
        )
        .with_local_kind(LocalQuestionKind::ProviderLogin)
        .with_no_freeform(),
    );

    let buf = draw_frame(&mut agent);
    let rendered = buf
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Configure provider"));
    assert!(rendered.contains("Grok subscription · configured"));
    assert!(rendered.contains("ChatGPT Plus/Pro · configured"));
    assert!(rendered.contains("API key · configured"));
    assert!(rendered.contains("Esc cancel"));
    assert!(!rendered.contains("~/.codex"));
    assert!(!rendered.contains("stored ("));
}

#[test]
fn direct_openrouter_input_escape_cancels_in_one_press() {
    let mut agent = make_agent();
    agent.prompt.set_text("main draft");
    let stashed = agent.prompt.stash();
    let mut state = QuestionViewState::new(
        "openrouter-key".into(),
        vec![Question {
            question: "Enter OpenRouter API key".into(),
            options: vec![],
            multi_select: Some(false),
            id: None,
        }],
        stashed,
    )
    .with_local_kind(LocalQuestionKind::OpenRouterApiKey);
    state.focus = QuestionFocus::InputMode;
    state.per_question_freeform_selected[0] = true;
    agent.question_view = Some(state);
    agent.prompt.set_text("sk-or-test-value");

    let outcome = agent.handle_question_key(&KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(outcome, InputOutcome::Changed));
    assert!(agent.question_view.is_none());
    assert_eq!(agent.prompt.text(), "main draft");
}

#[test]
fn direct_openrouter_input_enter_submits_key_without_option_step() {
    let mut agent = make_agent();
    let mut state = QuestionViewState::new(
        "openrouter-key".into(),
        vec![Question {
            question: "Enter OpenRouter API key".into(),
            options: vec![],
            multi_select: Some(false),
            id: None,
        }],
        StashedPrompt::default(),
    )
    .with_local_kind(LocalQuestionKind::OpenRouterApiKey);
    state.focus = QuestionFocus::InputMode;
    state.per_question_freeform_selected[0] = true;
    agent.question_view = Some(state);
    agent.prompt.set_text("sk-or-test-value");

    let outcome = agent.handle_question_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        outcome,
        InputOutcome::Action(crate::app::actions::Action::OpenRouterKeySubmitted {
            api_key
        }) if api_key == "sk-or-test-value"
    ));
    assert!(agent.question_view.is_none());
}

#[test]
fn direct_openrouter_input_renders_as_one_plain_field() {
    let mut agent = make_agent();
    let mut state = QuestionViewState::new(
        "openrouter-key".into(),
        vec![Question {
            question: "Enter OpenRouter API key".into(),
            options: vec![],
            multi_select: Some(false),
            id: None,
        }],
        StashedPrompt::default(),
    )
    .with_local_kind(LocalQuestionKind::OpenRouterApiKey);
    state.focus = QuestionFocus::InputMode;
    state.per_question_freeform_selected[0] = true;
    agent.question_view = Some(state);

    let buf = draw_frame(&mut agent);
    let rendered = buf
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Enter OpenRouter API key"));
    assert!(rendered.contains("Esc cancel"));
    assert!(rendered.contains("Enter:submit"));
    assert!(!rendered.contains("Type your answer here"));
    assert!(!rendered.contains("Paste key in freeform below"));
}

#[test]
fn effort_suppresses_scrollback_for_same_model() {
    use crate::exaforge::effort::should_log_model_switch_line;
    assert!(!should_log_model_switch_line(true, false)); // effort-only
    assert!(!should_log_model_switch_line(true, true)); // unchanged
    assert!(should_log_model_switch_line(false, false)); // full model switch
    assert!(!should_log_model_switch_line(false, true)); // impossible but quiet
}
