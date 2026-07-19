use base64::Engine;

use super::identity::ProviderId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderAuthStatus {
    Stored { detail: String },
    ApiKeyConfigured { detail: String },
    Unconfigured,
}

impl ProviderAuthStatus {
    pub fn label(&self) -> String {
        match self {
            Self::Stored { detail } => format!("✓ stored ({detail})"),
            Self::ApiKeyConfigured { detail } => format!("✓ api key ({detail})"),
            Self::Unconfigured => "unconfigured".into(),
        }
    }

    pub fn is_ready(&self) -> bool {
        !matches!(self, Self::Unconfigured)
    }

    /// Deliberately hides credential source, account, plan, and key details.
    pub fn configured_label(&self) -> &'static str {
        if self.is_ready() {
            "configured"
        } else {
            "not configured"
        }
    }
}

pub fn codex_status() -> ProviderAuthStatus {
    let Ok(raw) = std::fs::read_to_string(super::credentials::codex_auth_path()) else {
        return ProviderAuthStatus::Unconfigured;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return ProviderAuthStatus::Unconfigured;
    };
    let mode = value
        .get("auth_mode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let access_token = value
        .get("tokens")
        .and_then(|tokens| tokens.get("access_token"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if access_token.trim().is_empty() {
        return ProviderAuthStatus::Unconfigured;
    }
    let detail = jwt_chatgpt_plan(access_token)
        .map(|plan| format!("{mode}, {plan}"))
        .unwrap_or_else(|| mode.to_string());
    ProviderAuthStatus::Stored { detail }
}

fn jwt_chatgpt_plan(token: &str) -> Option<String> {
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let payload = parts[1];
    let padded = match payload.len() % 4 {
        2 => format!("{payload}=="),
        3 => format!("{payload}="),
        _ => payload.to_string(),
    };
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&padded)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(&padded))
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(&padded))
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    value
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_plan_type"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

pub fn openrouter_status() -> ProviderAuthStatus {
    if let Some(key) = super::credentials::read_openrouter_api_key() {
        let prefix = key.chars().take(8).collect::<String>();
        ProviderAuthStatus::ApiKeyConfigured {
            detail: format!("{prefix}…"),
        }
    } else {
        ProviderAuthStatus::Unconfigured
    }
}

pub fn spacexai_status() -> ProviderAuthStatus {
    let Ok(raw) = std::fs::read_to_string(super::credentials::grok_home().join("auth.json")) else {
        return ProviderAuthStatus::Unconfigured;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return ProviderAuthStatus::Unconfigured;
    };
    let has_any = value.as_object().is_some_and(|object| {
        object.values().any(|entry| {
            ["key", "token", "access_token", "refresh_token"]
                .into_iter()
                .any(|field| {
                    entry
                        .get(field)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|token| !token.trim().is_empty())
                })
                || entry.as_str().is_some_and(|token| !token.trim().is_empty())
        })
    });
    if has_any {
        ProviderAuthStatus::Stored {
            detail: "session".into(),
        }
    } else {
        ProviderAuthStatus::Unconfigured
    }
}

pub fn status_for(id: ProviderId) -> ProviderAuthStatus {
    match id {
        ProviderId::Spacexai => spacexai_status(),
        ProviderId::OpenaiCodex => codex_status(),
        ProviderId::Openrouter => openrouter_status(),
    }
}

pub fn picker_auth_status(id: ProviderId) -> &'static str {
    if status_for(id).is_ready() {
        "ready"
    } else {
        match id {
            ProviderId::Spacexai | ProviderId::OpenaiCodex => "login required",
            ProviderId::Openrouter => "key missing",
        }
    }
}

pub fn login_picker_providers() -> &'static [ProviderId] {
    &[
        ProviderId::Spacexai,
        ProviderId::OpenaiCodex,
        ProviderId::Openrouter,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_labels_do_not_expose_credential_details() {
        assert_eq!(
            ProviderAuthStatus::Unconfigured.configured_label(),
            "not configured"
        );
        assert_eq!(
            ProviderAuthStatus::ApiKeyConfigured {
                detail: "secret-prefix".into()
            }
            .configured_label(),
            "configured"
        );
    }

    #[test]
    fn codex_token_readable_from_home() {
        if super::super::credentials::read_codex_access_token().is_none() {
            eprintln!("skip: no ~/.codex/auth.json token");
            return;
        }
        assert!(
            super::super::credentials::read_codex_access_token()
                .unwrap()
                .len()
                > 20
        );
        assert!(super::super::credentials::read_codex_account_id().is_some());
        assert!(codex_status().is_ready());
    }

    #[test]
    fn spacex_subscription_readable_from_home() {
        if !super::super::credentials::grok_home()
            .join("auth.json")
            .is_file()
        {
            eprintln!("skip: no ~/.grok/auth.json");
            return;
        }
        assert!(
            spacexai_status().is_ready(),
            "existing Grok subscription credentials must render as configured"
        );
    }
}
