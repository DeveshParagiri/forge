//! Forge provider-aware usage policy and presentation.

use chrono::{DateTime, Local, Utc};
use xai_grok_shell::agent::provider_auth::{ProviderId, ProviderUsageSnapshot};

use crate::app::actions::Effect;
use crate::app::app_view::AppView;
use crate::scrollback::block::RenderBlock;

/// Plan the provider-account follow-up after upstream session token/cost usage.
///
/// Keeping this branch in Forge lets every provider share upstream's session
/// usage block while still using its own account/quota surface afterward.
pub(crate) fn account_follow_up(
    app: &mut AppView,
    id: crate::app::agent::AgentId,
) -> Vec<Effect> {
    let Some(provider) = app
        .agents
        .get(&id)
        .and_then(|agent| agent.session.models.current_provider_id())
    else {
        push_system(
            app,
            id,
            "Account usage is unavailable because the active model's provider is unknown.",
        );
        return vec![];
    };

    match provider {
        ProviderId::Spacexai => {
            if !app.usage_visible {
                return vec![];
            }
            if let Some(url) = app.usage_billing_redirect_url.clone() {
                push_system(app, id, &format!("Please check your usage on {url}"));
                return vec![];
            }
            vec![Effect::FetchBilling {
                agent_id: id,
                silent: false,
            }]
        }
        ProviderId::OpenaiCodex => vec![Effect::FetchProviderUsage {
            agent_id: id,
            provider,
        }],
        ProviderId::Openrouter => {
            push_system(
                app,
                id,
                "OpenRouter account usage is not available in Forge yet.",
            );
            vec![]
        }
    }
}

fn push_system(app: &mut AppView, id: crate::app::agent::AgentId, message: &str) {
    if let Some(agent) = app.agents.get_mut(&id) {
        agent
            .scrollback
            .push_block(RenderBlock::system(message.to_owned()));
    }
}

/// Only SpaceXAI sessions use the native credit warning model.
pub(crate) fn warning(
    provider: Option<ProviderId>,
    balance: &crate::views::credit_bar::CreditBalance,
    autotopup: Option<&crate::views::credit_bar::AutoTopupInfo>,
    usage_visible: bool,
    gateway_chat: bool,
) -> Option<(String, bool)> {
    if provider != Some(ProviderId::Spacexai) {
        return None;
    }
    crate::views::credit_bar::usage_warning_for_session(
        balance,
        autotopup,
        usage_visible,
        gateway_chat,
    )
}

pub(crate) fn format(snapshot: &ProviderUsageSnapshot) -> String {
    let mut lines = vec![format!("{} usage", snapshot.provider.display_name())];

    if let Some(plan) = snapshot
        .plan
        .as_deref()
        .map(sanitize_provider_text)
        .filter(|plan| !plan.is_empty())
    {
        lines.push(format!("Plan: {}", title_case(&plan)));
    }

    for (index, window) in snapshot.windows.iter().enumerate() {
        let label = window_label(window.window_seconds, index);
        lines.push(format!("{label}: {:.0}% used", window.used_percent.floor()));
        if let Some(reset) = window.reset_at.and_then(format_reset) {
            lines.push(format!("{label} reset: {reset}"));
        }
    }

    if snapshot.windows.is_empty() {
        lines.push("Quota windows: unavailable".to_string());
    }
    if snapshot.allowed == Some(false) {
        lines.push("Status: limit reached".to_string());
    }
    if let Some(credits) = &snapshot.credits {
        if credits.unlimited {
            lines.push("Credits: unlimited".to_string());
        } else {
            lines.push(format!(
                "Credits: {}",
                sanitize_provider_text(&credits.balance)
            ));
        }
    }

    lines.join("\n")
}

fn sanitize_provider_text(value: &str) -> String {
    value
        .chars()
        .filter(|c| !crate::render::line_utils::is_unsafe_display_char(*c))
        .take(128)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn window_label(seconds: Option<u64>, index: usize) -> String {
    match seconds {
        Some(604_800) => "Weekly limit".to_string(),
        Some(seconds) if seconds % 86_400 == 0 => format!("{}-day limit", seconds / 86_400),
        Some(seconds) if seconds % 3_600 == 0 => format!("{}-hour limit", seconds / 3_600),
        _ if index == 0 => "Primary limit".to_string(),
        _ => "Secondary limit".to_string(),
    }
}

fn format_reset(timestamp: i64) -> Option<String> {
    let utc = DateTime::<Utc>::from_timestamp(timestamp, 0)?;
    Some(
        utc.with_timezone(&Local)
            .format("%b %-d, %-I:%M %p %Z")
            .to_string(),
    )
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_shell::agent::provider_auth::{UsageCredits, UsageWindow};

    #[test]
    fn strips_unsafe_provider_text() {
        assert_eq!(
            sanitize_provider_text("plus\n\u{1b}[31m\u{202e}spoof"),
            "plus[31mspoof"
        );
    }

    #[test]
    fn formats_codex_weekly_usage() {
        let snapshot = ProviderUsageSnapshot {
            provider: ProviderId::OpenaiCodex,
            plan: Some("plus".to_string()),
            allowed: Some(true),
            windows: vec![UsageWindow {
                used_percent: 60.0,
                window_seconds: Some(604_800),
                reset_at: None,
            }],
            credits: Some(UsageCredits {
                balance: "0".to_string(),
                unlimited: false,
            }),
        };
        assert_eq!(
            format(&snapshot),
            "OpenAI Codex usage\nPlan: Plus\nWeekly limit: 60% used\nCredits: 0"
        );
    }
}
