//! Provider-neutral usage snapshots and provider-specific usage fetchers.
//!
//! Keep provider wire formats private to this module. UI consumers receive a
//! small normalized snapshot, so adding another provider does not leak its API
//! response shape into the pager.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::identity::ProviderId;

const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderUsageSnapshot {
    pub provider: ProviderId,
    pub plan: Option<String>,
    pub allowed: Option<bool>,
    pub windows: Vec<UsageWindow>,
    pub credits: Option<UsageCredits>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageWindow {
    pub used_percent: f64,
    pub window_seconds: Option<u64>,
    pub reset_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageCredits {
    pub balance: String,
    pub unlimited: bool,
}

/// Fetch usage for a provider with a direct account-usage API.
///
/// SpaceXAI billing still travels through its ACP extension and is intentionally
/// handled by the pager. Providers without an account-usage API return a clear
/// unsupported error instead of being mistaken for authentication failures.
pub async fn fetch_provider_usage(provider: ProviderId) -> Result<ProviderUsageSnapshot> {
    match provider {
        ProviderId::OpenaiCodex => fetch_codex_usage(CODEX_USAGE_URL).await,
        ProviderId::Spacexai => bail!("SpaceXAI usage is provided by the billing extension"),
        ProviderId::Openrouter => bail!("OpenRouter account usage is not available yet"),
    }
}

async fn fetch_codex_usage(url: &str) -> Result<ProviderUsageSnapshot> {
    let token = super::credentials::read_codex_access_token()
        .context("ChatGPT Codex OAuth is not configured; run /login codex")?;
    let account_id = super::credentials::read_codex_account_id()
        .context("ChatGPT Codex account ID is missing; run /login codex")?;

    let response = crate::http::shared_client()
        .get(url)
        .bearer_auth(token)
        .header("ChatGPT-Account-Id", account_id)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, "grok-build-forge")
        .send()
        .await
        .context("could not reach ChatGPT usage service")?;

    if !response.status().is_success() {
        bail!("ChatGPT usage service returned HTTP {}", response.status());
    }

    let wire: CodexUsageResponse = response
        .json()
        .await
        .context("could not parse ChatGPT usage response")?;
    Ok(wire.into_snapshot())
}

#[derive(Debug, Deserialize)]
struct CodexUsageResponse {
    plan_type: Option<String>,
    rate_limit: Option<CodexRateLimit>,
    credits: Option<CodexCredits>,
}

#[derive(Debug, Deserialize)]
struct CodexRateLimit {
    allowed: Option<bool>,
    primary_window: Option<CodexUsageWindow>,
    secondary_window: Option<CodexUsageWindow>,
}

#[derive(Debug, Deserialize)]
struct CodexUsageWindow {
    used_percent: f64,
    limit_window_seconds: Option<u64>,
    reset_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CodexCredits {
    balance: String,
    #[serde(default)]
    unlimited: bool,
}

impl CodexUsageResponse {
    fn into_snapshot(self) -> ProviderUsageSnapshot {
        let mut windows = Vec::new();
        let allowed = self.rate_limit.as_ref().and_then(|limit| limit.allowed);
        if let Some(limit) = self.rate_limit {
            windows.extend(
                [limit.primary_window, limit.secondary_window]
                    .into_iter()
                    .flatten()
                    .map(|window| UsageWindow {
                        used_percent: window.used_percent.clamp(0.0, 100.0),
                        window_seconds: window.limit_window_seconds,
                        reset_at: window.reset_at,
                    }),
            );
        }
        ProviderUsageSnapshot {
            provider: ProviderId::OpenaiCodex,
            plan: self.plan_type,
            allowed,
            windows,
            credits: self.credits.map(|credits| UsageCredits {
                balance: credits.balance,
                unlimited: credits.unlimited,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_wire_normalizes_without_personal_fields() {
        let wire: CodexUsageResponse = serde_json::from_value(serde_json::json!({
            "account_id": "private",
            "email": "private@example.com",
            "plan_type": "plus",
            "rate_limit": {
                "allowed": true,
                "primary_window": {
                    "used_percent": 60,
                    "limit_window_seconds": 604800,
                    "reset_at": 1785039403
                },
                "secondary_window": null
            },
            "credits": { "balance": "0", "unlimited": false }
        }))
        .unwrap();

        let snapshot = wire.into_snapshot();
        assert_eq!(snapshot.provider, ProviderId::OpenaiCodex);
        assert_eq!(snapshot.plan.as_deref(), Some("plus"));
        assert_eq!(snapshot.allowed, Some(true));
        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.windows[0].used_percent, 60.0);
        assert_eq!(snapshot.credits.unwrap().balance, "0");
    }

    #[test]
    fn percentages_are_clamped() {
        let wire: CodexUsageResponse = serde_json::from_value(serde_json::json!({
            "rate_limit": {
                "primary_window": { "used_percent": 140 },
                "secondary_window": { "used_percent": -4 }
            }
        }))
        .unwrap();
        let snapshot = wire.into_snapshot();
        assert_eq!(snapshot.windows[0].used_percent, 100.0);
        assert_eq!(snapshot.windows[1].used_percent, 0.0);
    }
}
