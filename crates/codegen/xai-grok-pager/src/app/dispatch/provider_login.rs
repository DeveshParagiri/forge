//! Personal: Pi-style multi-provider `/login` dispatch.
//!
//! Kept in its own file so upstream rebases of `auth.rs` stay clean.
//! Hooks: `Action::ChooseProviderLogin` / `ProviderLoginSelected` /
//! `OpenRouterKeySubmitted` in `router.rs`.

use super::auth::dispatch_login;
use crate::app::actions::{Action, Effect};
use crate::app::app_view::{ActiveView, AppView};
use crate::views::question_view::{LocalQuestionKind, QuestionFocus, QuestionViewState};
use xai_grok_shell::agent::provider_auth::{self, ProviderId, set_openrouter_api_key, status_for};
use xai_grok_tools::implementations::grok_build::ask_user_question::{Question, QuestionOption};

/// Open the provider picker (Pi `/login` equivalent).
pub(super) fn dispatch_choose_provider_login(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        // On welcome / no agent: fall back to SpaceXAI login (upstream path).
        return dispatch_login(app);
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return dispatch_login(app);
    };
    if agent.question_view.is_some() {
        app.show_toast("Finish answering the current question first");
        return vec![];
    }

    let mut options = Vec::new();
    for pid in provider_auth::login_picker_providers() {
        let status = status_for(*pid);
        let label = pid.display_name();
        let description = match pid {
            ProviderId::Spacexai => {
                format!("Grok subscription · {}", status.configured_label())
            }
            ProviderId::OpenaiCodex => {
                format!("ChatGPT Plus/Pro · {}", status.configured_label())
            }
            ProviderId::Openrouter => {
                format!("API key · {}", status.configured_label())
            }
        };
        options.push(QuestionOption {
            label: label.into(),
            description,
            preview: None,
            id: Some(pid.as_str().into()),
        });
    }

    let question = Question {
        question: "Configure provider".into(),
        id: Some("provider-login".into()),
        options,
        multi_select: Some(false),
    };
    let stashed = agent.prompt.stash();
    let state = QuestionViewState::new(
        format!("provider-login-{}", uuid::Uuid::new_v4()),
        vec![question],
        stashed,
    )
    .with_local_kind(LocalQuestionKind::ProviderLogin)
    .with_no_freeform();
    agent.question_view = Some(state);
    agent.prompt.set_text("");
    vec![]
}

/// Handle a provider selection from the picker (or `/login <provider>`).
pub(super) fn dispatch_provider_login_selected(
    app: &mut AppView,
    provider_id: String,
) -> Vec<Effect> {
    let Some(pid) = ProviderId::from_str_id(&provider_id) else {
        let msg = format!("Unknown provider `{provider_id}`");
        app.show_toast(&msg);
        return vec![];
    };
    match pid {
        ProviderId::Spacexai => {
            if status_for(ProviderId::Spacexai).is_ready() {
                app.show_toast("Grok subscription is configured");
                vec![]
            } else {
                // Reuse upstream SpaceXAI interactive login.
                dispatch_login(app)
            }
        }
        ProviderId::OpenaiCodex => dispatch_codex_login(app),
        ProviderId::Openrouter => open_openrouter_key_question(app),
    }
}

fn dispatch_codex_login(app: &mut AppView) -> Vec<Effect> {
    let status = status_for(ProviderId::OpenaiCodex);
    if status.is_ready() {
        app.show_toast("OpenAI Codex is configured");
        return vec![];
    }

    // Not configured: try launching Codex CLI login if available.
    let codex_on_path = which_codex();
    if let Some(codex) = codex_on_path {
        app.show_toast("Opening OpenAI Codex login…");
        // Fire-and-forget; user completes browser flow. We do not block the TUI.
        let _ = std::process::Command::new(codex)
            .arg("login")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        return vec![];
    }

    app.show_toast("Codex CLI not found; run `codex login` after installing it");
    vec![]
}

fn which_codex() -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("codex");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // Homebrew common path
    let brew = std::path::PathBuf::from("/opt/homebrew/bin/codex");
    if brew.is_file() {
        return Some(brew);
    }
    None
}

fn open_openrouter_key_question(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        app.show_toast("Open a session first, then /login openrouter");
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    if agent.question_view.is_some() {
        app.show_toast("Finish answering the current question first");
        return vec![];
    }

    // Pi-style direct API-key input: no synthetic option before the field.
    let question = Question {
        question: "Enter OpenRouter API key".into(),
        id: Some("openrouter-key".into()),
        options: vec![],
        multi_select: Some(false),
    };
    let stashed = agent.prompt.stash();
    let mut state = QuestionViewState::new(
        format!("openrouter-key-{}", uuid::Uuid::new_v4()),
        vec![question],
        stashed,
    )
    .with_local_kind(LocalQuestionKind::OpenRouterApiKey);
    state.focus = QuestionFocus::InputMode;
    state.per_question_freeform_selected[0] = true;
    agent.question_view = Some(state);
    agent.prompt.set_text("");
    vec![]
}

pub(super) fn dispatch_openrouter_key_submitted(app: &mut AppView, api_key: String) -> Vec<Effect> {
    let key = api_key.trim();
    if key.is_empty() {
        app.show_toast("Empty API key — cancelled");
        return vec![];
    }
    match set_openrouter_api_key(key) {
        Ok(()) => {
            app.show_toast("OpenRouter is configured");
        }
        Err(e) => {
            let msg = format!("Failed to save OpenRouter key: {e}");
            app.show_toast(&msg);
        }
    }
    vec![]
}

/// Optional no-op keep-alive for unused Action import in tests.
#[allow(dead_code)]
fn _action_touch() -> Action {
    Action::ChooseProviderLogin
}
