//! `/login` -- configure provider authentication (Pi-style multi-provider).
//!
//! Forge: opens a provider picker (SpaceXAI / OpenAI Codex / OpenRouter).
//! Welcome-screen "Log in" still uses [`Action::Login`] (SpaceXAI only) so
//! upstream cold-start auth is unchanged.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

pub struct LoginCommand;

impl SlashCommand for LoginCommand {
    fn name(&self) -> &str {
        "login"
    }

    fn description(&self) -> &str {
        "Configure provider auth (SpaceXAI, OpenAI Codex, OpenRouter)"
    }

    fn usage(&self) -> &str {
        "/login [spacexai|codex|openrouter]"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let arg = args.trim();
        if arg.is_empty() {
            // Forge: Pi-style picker.
            return CommandResult::Action(Action::ChooseProviderLogin);
        }
        // Direct provider shortcuts keep scripts happy without the picker.
        let id = match arg.to_ascii_lowercase().as_str() {
            "spacexai" | "xai" | "grok" => "spacexai",
            "codex" | "openai-codex" | "chatgpt" => "openai-codex",
            "openrouter" | "or" => "openrouter",
            _ => {
                return CommandResult::Message(format!(
                    "Unknown provider `{arg}`. Use /login, or /login spacexai|codex|openrouter"
                ));
            }
        };
        CommandResult::Action(Action::ProviderLoginSelected {
            provider_id: id.to_string(),
        })
    }
}
