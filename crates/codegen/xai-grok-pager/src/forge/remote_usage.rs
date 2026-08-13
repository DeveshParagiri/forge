//! Structured, session-scoped usage state for Forge Remote.
//!
//! This deliberately does not reuse the terminal `/usage` presentation path:
//! remote refreshes fetch the same low-level data, then cache a typed snapshot
//! on the exact [`AgentView`](crate::app::agent_view::AgentView) binding.

use serde::Serialize;
use xai_grok_shell::agent::provider_auth::{ProviderId, ProviderUsageSnapshot};
use xai_grok_shell::extensions::notification::{PromptUsage, PromptUsageModel};
use xai_grok_shell::session::ContextInfo;

use crate::app::agent::AgentId;
use crate::views::credit_bar::CreditBalance;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteUsageSnapshot {
    pub status: RemoteUsageStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refreshed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<RemoteContextUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<RemoteSessionUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<RemoteAccountUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<RemoteUsageErrors>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RemoteUsageStatus {
    Idle,
    Loading,
    Ready,
    Partial,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteContextUsage {
    pub used_tokens: u64,
    pub total_tokens: u64,
    pub free_tokens: u64,
    pub used_percent: f64,
    pub auto_compact_percent: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteSessionUsage {
    pub input_tokens: u64,
    pub cached_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub model_calls: u64,
    pub api_duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd_ticks: Option<String>,
    pub cost_state: RemoteCostState,
    pub incomplete: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<RemoteSessionModelUsage>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteSessionModelUsage {
    pub model_id: String,
    pub input_tokens: u64,
    pub cached_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub model_calls: u64,
    pub api_duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd_ticks: Option<String>,
    pub cost_state: RemoteCostState,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RemoteCostState {
    Exact,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteAccountUsage {
    pub provider: String,
    pub status: RemoteAccountStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed: Option<bool>,
    pub windows: Vec<RemoteUsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credits: Option<RemoteUsageCredits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RemoteAccountStatus {
    Ready,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteUsageWindow {
    pub label: String,
    pub used_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteUsageCredits {
    pub balance: String,
    pub unlimited: bool,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteUsageErrors {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

impl RemoteUsageErrors {
    fn is_empty(&self) -> bool {
        self.context.is_none() && self.session.is_none() && self.account.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUsageRequestIdentity {
    pub agent_id: AgentId,
    pub session_id: agent_client_protocol::SessionId,
    pub session_binding_epoch: u32,
    pub bridge_generation: u64,
    pub gateway_generation: u64,
    pub request_id: String,
    pub refresh_generation: u64,
    pub provider: Option<ProviderId>,
}

#[derive(Debug)]
pub struct RemoteUsageFetchOutcome {
    pub(crate) context: Result<RemoteContextUsage, String>,
    pub(crate) session: Result<RemoteSessionUsage, String>,
    pub(crate) account: Result<RemoteAccountUsage, String>,
    pub(crate) refreshed_at: String,
}

#[derive(Debug, Default)]
pub(crate) struct RemoteUsageState {
    snapshot: Option<RemoteUsageSnapshot>,
    pending: Option<RemoteUsageRequestIdentity>,
    next_generation: u64,
}

impl RemoteUsageState {
    pub(crate) fn snapshot(&self) -> Option<&RemoteUsageSnapshot> {
        self.snapshot.as_ref()
    }

    pub(crate) fn next_refresh_generation(&mut self) -> u64 {
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        self.next_generation
    }

    pub(crate) fn begin_refresh(
        &mut self,
        identity: RemoteUsageRequestIdentity,
        immediate_context: Option<&ContextInfo>,
    ) {
        let mut snapshot = self.snapshot.take().unwrap_or(RemoteUsageSnapshot {
            status: RemoteUsageStatus::Idle,
            refreshed_at: None,
            context: immediate_context.map(context_from_info),
            session: None,
            account: None,
            errors: None,
        });
        snapshot.status = RemoteUsageStatus::Loading;
        snapshot.errors = None;
        if snapshot.context.is_none() {
            snapshot.context = immediate_context.map(context_from_info);
        }
        self.snapshot = Some(snapshot);
        self.pending = Some(identity);
    }

    pub(crate) fn apply_result(
        &mut self,
        identity: &RemoteUsageRequestIdentity,
        outcome: RemoteUsageFetchOutcome,
    ) -> bool {
        if self.pending.as_ref() != Some(identity) {
            return false;
        }
        self.pending = None;

        let (context, context_error) = split_result(outcome.context);
        let (session, session_error) = split_result(outcome.session);
        let (mut account, account_error) = split_result(outcome.account);
        if let Some(error) = account_error.as_ref() {
            account = Some(RemoteAccountUsage {
                provider: identity
                    .provider
                    .map(ProviderId::as_str)
                    .unwrap_or("unknown")
                    .to_owned(),
                status: RemoteAccountStatus::Error,
                plan: None,
                allowed: None,
                windows: Vec::new(),
                credits: None,
                message: Some(error.clone()),
            });
        }
        let errors = RemoteUsageErrors {
            context: context_error,
            session: session_error,
            account: account_error,
        };
        let successful_scopes = usize::from(context.is_some())
            + usize::from(session.is_some())
            + usize::from(
                account
                    .as_ref()
                    .is_some_and(|value| value.status != RemoteAccountStatus::Error),
            );
        let session_incomplete = session
            .as_ref()
            .is_some_and(|value| value.incomplete || value.cost_state == RemoteCostState::Partial);
        let status = if successful_scopes == 0 {
            RemoteUsageStatus::Error
        } else if !errors.is_empty() || session_incomplete {
            RemoteUsageStatus::Partial
        } else {
            RemoteUsageStatus::Ready
        };
        self.snapshot = Some(RemoteUsageSnapshot {
            status,
            refreshed_at: Some(outcome.refreshed_at),
            context,
            session,
            account,
            errors: (!errors.is_empty()).then_some(errors),
        });
        true
    }

    pub(crate) fn clear(&mut self) {
        // Invalidate cached/pending data without rewinding the request
        // generation. A client command ID is not a server-owned nonce and may
        // be reused, so a late pre-clear result must never become identical to
        // a later refresh on the same bridge/session/provider binding.
        self.snapshot = None;
        self.pending = None;
    }
}

fn split_result<T>(result: Result<T, String>) -> (Option<T>, Option<String>) {
    match result {
        Ok(value) => (Some(value), None),
        Err(error) => (None, Some(error)),
    }
}

pub(crate) fn context_from_info(info: &ContextInfo) -> RemoteContextUsage {
    RemoteContextUsage {
        used_tokens: info.used,
        total_tokens: info.total,
        free_tokens: info.free_tokens,
        used_percent: f64::from(info.usage_pct),
        auto_compact_percent: f64::from(info.auto_compact_threshold_percent),
    }
}

pub(crate) fn session_from_usage(usage: &PromptUsage) -> RemoteSessionUsage {
    let force_partial = usage.usage_is_incomplete || usage.totals.cost_is_partial;
    RemoteSessionUsage {
        input_tokens: usage.totals.input_tokens,
        cached_read_tokens: usage.totals.cached_read_tokens,
        cache_creation_tokens: usage.totals.cache_creation_tokens,
        output_tokens: usage.totals.output_tokens,
        reasoning_tokens: usage.totals.reasoning_tokens,
        total_tokens: usage.totals.total_tokens,
        model_calls: usage.totals.model_calls,
        api_duration_ms: usage.totals.api_duration_ms,
        cost_usd_ticks: trusted_cost_ticks(&usage.totals, force_partial),
        cost_state: cost_state(&usage.totals, force_partial),
        incomplete: usage.usage_is_incomplete,
        models: usage
            .model_usage
            .iter()
            .map(|(model_id, model)| RemoteSessionModelUsage {
                model_id: model_id.clone(),
                input_tokens: model.input_tokens,
                cached_read_tokens: model.cached_read_tokens,
                cache_creation_tokens: model.cache_creation_tokens,
                output_tokens: model.output_tokens,
                reasoning_tokens: model.reasoning_tokens,
                total_tokens: model.total_tokens,
                model_calls: model.model_calls,
                api_duration_ms: model.api_duration_ms,
                cost_usd_ticks: trusted_cost_ticks(model, force_partial),
                cost_state: cost_state(model, force_partial),
            })
            .collect(),
    }
}

fn trusted_cost_ticks(model: &PromptUsageModel, force_partial: bool) -> Option<String> {
    (!force_partial && !model.cost_is_partial)
        .then_some(model.cost_usd_ticks)
        .flatten()
        .map(|ticks| ticks.to_string())
}

fn cost_state(model: &PromptUsageModel, force_partial: bool) -> RemoteCostState {
    if force_partial || model.cost_is_partial {
        RemoteCostState::Partial
    } else if model.cost_usd_ticks.is_some() {
        RemoteCostState::Exact
    } else {
        RemoteCostState::Unavailable
    }
}

pub(crate) fn account_from_provider(snapshot: &ProviderUsageSnapshot) -> RemoteAccountUsage {
    RemoteAccountUsage {
        provider: snapshot.provider.as_str().to_owned(),
        status: RemoteAccountStatus::Ready,
        plan: snapshot.plan.clone(),
        allowed: snapshot.allowed,
        windows: snapshot
            .windows
            .iter()
            .enumerate()
            .map(|(index, window)| RemoteUsageWindow {
                label: window_label(window.window_seconds, index),
                used_percent: window.used_percent.clamp(0.0, 100.0),
                window_seconds: window.window_seconds,
                reset_at: window.reset_at,
                reset_label: None,
            })
            .collect(),
        credits: snapshot.credits.as_ref().map(|credits| RemoteUsageCredits {
            balance: credits.balance.clone(),
            unlimited: credits.unlimited,
        }),
        message: None,
    }
}

pub(crate) fn account_from_xai(
    balance: Option<&CreditBalance>,
    subscription_tier: Option<String>,
) -> RemoteAccountUsage {
    let Some(balance) = balance else {
        return unavailable_account(
            Some(ProviderId::Spacexai),
            "Account limits are unavailable for this session.",
        );
    };
    RemoteAccountUsage {
        provider: ProviderId::Spacexai.as_str().to_owned(),
        status: RemoteAccountStatus::Ready,
        plan: subscription_tier,
        allowed: Some(balance.effective_usage_pct < 100.0),
        windows: vec![RemoteUsageWindow {
            label: balance.usage_label().to_owned(),
            used_percent: balance.usage_pct.clamp(0.0, 100.0),
            window_seconds: None,
            reset_at: None,
            reset_label: balance.period_end_display.clone(),
        }],
        credits: balance
            .prepaid_balance_cents
            .map(|cents| RemoteUsageCredits {
                balance: format_cents(cents),
                unlimited: false,
            }),
        message: None,
    }
}

pub(crate) fn unavailable_account(
    provider: Option<ProviderId>,
    message: impl Into<String>,
) -> RemoteAccountUsage {
    RemoteAccountUsage {
        provider: provider
            .map(ProviderId::as_str)
            .unwrap_or("unknown")
            .to_owned(),
        status: RemoteAccountStatus::Unavailable,
        plan: None,
        allowed: None,
        windows: Vec::new(),
        credits: None,
        message: Some(message.into()),
    }
}

pub(crate) fn refreshed_at_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn window_label(seconds: Option<u64>, index: usize) -> String {
    match seconds {
        Some(604_800) => "Weekly limit".to_owned(),
        Some(seconds) if seconds % 86_400 == 0 => format!("{}-day limit", seconds / 86_400),
        Some(seconds) if seconds % 3_600 == 0 => format!("{}-hour limit", seconds / 3_600),
        _ if index == 0 => "Primary limit".to_owned(),
        _ => "Secondary limit".to_owned(),
    }
}

fn format_cents(cents: i64) -> String {
    let absolute = cents.unsigned_abs();
    format!("${}.{:02}", absolute / 100, absolute % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(agent_id: usize, session_id: &str, request_id: &str) -> RemoteUsageRequestIdentity {
        RemoteUsageRequestIdentity {
            agent_id: AgentId(agent_id),
            session_id: agent_client_protocol::SessionId::new(session_id),
            session_binding_epoch: 7,
            bridge_generation: 11,
            gateway_generation: 13,
            request_id: request_id.to_owned(),
            refresh_generation: 17,
            provider: Some(ProviderId::OpenaiCodex),
        }
    }

    fn context() -> RemoteContextUsage {
        RemoteContextUsage {
            used_tokens: 25,
            total_tokens: 100,
            free_tokens: 75,
            used_percent: 25.0,
            auto_compact_percent: 85.0,
        }
    }

    fn session(incomplete: bool) -> RemoteSessionUsage {
        RemoteSessionUsage {
            input_tokens: 10,
            cached_read_tokens: 2,
            cache_creation_tokens: 1,
            output_tokens: 5,
            reasoning_tokens: 3,
            total_tokens: 15,
            model_calls: 1,
            api_duration_ms: 250,
            cost_usd_ticks: (!incomplete).then(|| "123456789012345678".to_owned()),
            cost_state: if incomplete {
                RemoteCostState::Partial
            } else {
                RemoteCostState::Exact
            },
            incomplete,
            models: Vec::new(),
        }
    }

    fn unavailable() -> RemoteAccountUsage {
        unavailable_account(Some(ProviderId::Openrouter), "Not supported")
    }

    fn outcome(
        context: Result<RemoteContextUsage, String>,
        session: Result<RemoteSessionUsage, String>,
        account: Result<RemoteAccountUsage, String>,
    ) -> RemoteUsageFetchOutcome {
        RemoteUsageFetchOutcome {
            context,
            session,
            account,
            refreshed_at: "2026-08-12T20:00:00.000Z".to_owned(),
        }
    }

    #[test]
    fn session_cost_is_a_decimal_string_and_partial_costs_fail_closed() {
        let mut usage = PromptUsage::default();
        usage.totals = PromptUsageModel {
            input_tokens: 10,
            output_tokens: 5,
            total_tokens: 15,
            model_calls: 1,
            cost_usd_ticks: Some(9_007_199_254_740_993),
            ..Default::default()
        };
        usage.model_usage.insert(
            "model-a".to_owned(),
            PromptUsageModel {
                model_calls: 1,
                cost_usd_ticks: Some(42),
                ..Default::default()
            },
        );
        let projected = session_from_usage(&usage);
        assert_eq!(
            projected.cost_usd_ticks.as_deref(),
            Some("9007199254740993")
        );
        assert_eq!(projected.models[0].cost_usd_ticks.as_deref(), Some("42"));

        usage.usage_is_incomplete = true;
        let projected = session_from_usage(&usage);
        assert_eq!(projected.cost_usd_ticks, None);
        assert_eq!(projected.cost_state, RemoteCostState::Partial);
        assert_eq!(projected.models[0].cost_usd_ticks, None);
        assert_eq!(projected.models[0].cost_state, RemoteCostState::Partial);

        usage.usage_is_incomplete = false;
        usage.totals.cost_is_partial = true;
        let partial_cost_only = session_from_usage(&usage);
        assert!(!partial_cost_only.incomplete);
        assert_eq!(partial_cost_only.cost_state, RemoteCostState::Partial);
        assert_eq!(partial_cost_only.cost_usd_ticks, None);
    }

    #[test]
    fn state_projects_ready_partial_and_error_without_mixing_scopes() {
        let request = identity(1, "session-a", "request-a");
        let mut ready = RemoteUsageState::default();
        ready.begin_refresh(request.clone(), None);
        assert!(ready.apply_result(
            &request,
            outcome(Ok(context()), Ok(session(false)), Ok(unavailable()))
        ));
        assert_eq!(ready.snapshot().unwrap().status, RemoteUsageStatus::Ready);

        let mut partial = RemoteUsageState::default();
        partial.begin_refresh(request.clone(), None);
        assert!(partial.apply_result(
            &request,
            outcome(
                Ok(context()),
                Ok(session(true)),
                Err("quota fetch failed".to_owned())
            )
        ));
        let snapshot = partial.snapshot().unwrap();
        assert_eq!(snapshot.status, RemoteUsageStatus::Partial);
        assert_eq!(
            snapshot.account.as_ref().unwrap().status,
            RemoteAccountStatus::Error
        );
        assert_eq!(
            snapshot.errors.as_ref().unwrap().account.as_deref(),
            Some("quota fetch failed")
        );

        let mut partial_cost = RemoteUsageState::default();
        partial_cost.begin_refresh(request.clone(), None);
        let mut partial_cost_session = session(false);
        partial_cost_session.cost_state = RemoteCostState::Partial;
        partial_cost_session.cost_usd_ticks = None;
        assert!(partial_cost.apply_result(
            &request,
            outcome(Ok(context()), Ok(partial_cost_session), Ok(unavailable()))
        ));
        assert_eq!(
            partial_cost.snapshot().unwrap().status,
            RemoteUsageStatus::Partial
        );

        let mut error = RemoteUsageState::default();
        error.begin_refresh(request.clone(), None);
        assert!(error.apply_result(
            &request,
            outcome(
                Err("context failed".to_owned()),
                Err("session failed".to_owned()),
                Err("account failed".to_owned())
            )
        ));
        assert_eq!(error.snapshot().unwrap().status, RemoteUsageStatus::Error);
    }

    #[test]
    fn stale_and_duplicate_session_results_are_rejected_by_full_identity() {
        let request = identity(1, "duplicate-session", "request-a");
        let mut state = RemoteUsageState::default();
        state.begin_refresh(request.clone(), None);

        let different_agent = identity(2, "duplicate-session", "request-a");
        assert!(!state.apply_result(
            &different_agent,
            outcome(Ok(context()), Ok(session(false)), Ok(unavailable()))
        ));
        let mut stale_request = request.clone();
        stale_request.request_id = "request-b".to_owned();
        assert!(!state.apply_result(
            &stale_request,
            outcome(Ok(context()), Ok(session(false)), Ok(unavailable()))
        ));
        let mut stale_gateway = request.clone();
        stale_gateway.gateway_generation += 1;
        assert!(!state.apply_result(
            &stale_gateway,
            outcome(Ok(context()), Ok(session(false)), Ok(unavailable()))
        ));
        assert_eq!(state.snapshot().unwrap().status, RemoteUsageStatus::Loading);
    }

    #[test]
    fn clear_invalidates_pending_without_rewinding_server_generation() {
        let mut state = RemoteUsageState::default();
        let first_generation = state.next_refresh_generation();
        let mut first = identity(1, "session-a", "reused-command-id");
        first.refresh_generation = first_generation;
        state.begin_refresh(first.clone(), None);

        state.clear();

        let second_generation = state.next_refresh_generation();
        assert!(second_generation > first_generation);
        let mut second = first.clone();
        second.refresh_generation = second_generation;
        state.begin_refresh(second.clone(), None);

        assert!(!state.apply_result(
            &first,
            outcome(Ok(context()), Ok(session(false)), Ok(unavailable()))
        ));
        assert!(state.apply_result(
            &second,
            outcome(Ok(context()), Ok(session(false)), Ok(unavailable()))
        ));
    }

    #[test]
    fn xai_negative_accounting_credit_serializes_as_positive_balance() {
        let balance = CreditBalance {
            usage_pct: 20.0,
            effective_usage_pct: 20.0,
            period_end_display: Some("Aug 18, 9:00 PM".to_owned()),
            pay_as_you_go: false,
            on_demand_cap_cents: None,
            on_demand_used_cents: None,
            prepaid_balance_cents: Some(-1_234),
            period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".to_owned()),
            is_unified_billing_user: Some(true),
        };
        let account = account_from_xai(Some(&balance), Some("super".to_owned()));
        assert_eq!(account.windows[0].label, "Weekly limit");
        assert_eq!(account.credits.unwrap().balance, "$12.34");
    }

    #[test]
    fn unavailable_account_and_snapshot_use_exact_camel_case_schema() {
        let request = identity(1, "session-a", "request-a");
        let mut state = RemoteUsageState::default();
        state.begin_refresh(request.clone(), None);
        assert!(state.apply_result(
            &request,
            outcome(Ok(context()), Ok(session(false)), Ok(unavailable()))
        ));
        let value = serde_json::to_value(state.snapshot().unwrap()).unwrap();
        assert_eq!(value["status"], "ready");
        assert_eq!(value["refreshedAt"], "2026-08-12T20:00:00.000Z");
        assert_eq!(value["context"]["usedTokens"], 25);
        assert_eq!(value["session"]["costUsdTicks"], "123456789012345678");
        assert_eq!(value["account"]["status"], "unavailable");
        assert!(value["account"]["windows"].is_array());
        assert!(value.get("errors").is_none());
    }
}
