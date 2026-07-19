//! Provider-neutral account usage formatting for `/usage`.

use chrono::{DateTime, Local, Utc};
use xai_grok_shell::agent::provider_auth::ProviderUsageSnapshot;

pub fn format_provider_usage(snapshot: &ProviderUsageSnapshot) -> String {
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
    use xai_grok_shell::agent::provider_auth::{ProviderId, UsageCredits, UsageWindow};

    #[test]
    fn strips_unsafe_provider_text() {
        assert_eq!(
            sanitize_provider_text("plus\n\u{1b}[31m\u{202e}spoof"),
            "plus[31mspoof"
        );
    }

    #[test]
    fn formats_codex_weekly_usage_without_personal_fields() {
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
            format_provider_usage(&snapshot),
            "OpenAI Codex usage\nPlan: Plus\nWeekly limit: 60% used\nCredits: 0"
        );
    }
}
