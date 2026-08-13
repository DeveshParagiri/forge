//! Forge Remote slash command.
//!
//! Browser/app remote access is private and tailnet-only. Forge never installs
//! Tailscale; starting remote control owns only this session's scoped Serve path.

use crate::app::actions::Action;
use crate::slash::command::{ArgItem, CommandExecCtx, CommandResult, SlashCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteControlCommand {
    Start,
    Status,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteControlStatus {
    pub state: String,
    pub message: String,
    /// Secret-bearing pairing URL. It stays in transient modal state and is
    /// never committed to scrollback or sent through ACP.
    pub remote_url: Option<String>,
    /// Monotonic expiry captured from the exact gateway arm.
    pub expires_at: Option<std::time::Instant>,
}

impl RemoteControlStatus {
    pub fn message(state: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            state: state.into(),
            message: message.into(),
            remote_url: None,
            expires_at: None,
        }
    }

    pub fn live(remote_url: String, expires_at: std::time::Instant) -> Self {
        Self {
            state: "live".into(),
            message: "Ready on your tailnet.".into(),
            remote_url: Some(remote_url),
            expires_at: Some(expires_at),
        }
    }
}

pub struct RemoteControlSlashCommand;

fn parse_remote_control_command(args: &str) -> Result<RemoteControlCommand, &'static str> {
    match args.trim() {
        "" | "enable" => Ok(RemoteControlCommand::Start),
        "status" => Ok(RemoteControlCommand::Status),
        "stop" => Ok(RemoteControlCommand::Stop),
        _ => Err("Usage: /rc [status|stop]"),
    }
}

impl SlashCommand for RemoteControlSlashCommand {
    fn name(&self) -> &str {
        "rc"
    }
    fn aliases(&self) -> &[&str] {
        &["remote-control"]
    }
    fn description(&self) -> &str {
        "Open this live session privately on your phone"
    }
    fn usage(&self) -> &str {
        "/rc [status|stop]"
    }
    fn session_scoped(&self) -> bool {
        true
    }
    fn suggest_args(
        &self,
        _ctx: &crate::slash::command::AppCtx<'_>,
        args_query: &str,
    ) -> Option<Vec<ArgItem>> {
        let items = [
            ("status", "Show private remote-control status"),
            ("stop", "Revoke phone access and stop remote control"),
        ]
        .into_iter()
        .filter(|(name, _)| name.starts_with(args_query.trim()))
        .map(|(name, description)| ArgItem {
            display: name.into(),
            match_text: name.into(),
            insert_text: name.into(),
            description: description.into(),
        })
        .collect::<Vec<_>>();
        (!items.is_empty()).then_some(items)
    }
    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        if ctx.session_id.is_none() {
            return CommandResult::Error("`/rc` needs an active Forge session.".into());
        }
        let command = match parse_remote_control_command(args) {
            Ok(command) => command,
            Err(usage) => return CommandResult::Error(usage.into()),
        };
        CommandResult::Action(Action::ForgeRemoteControl(command))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_are_stable() {
        let command = RemoteControlSlashCommand;
        assert_eq!(command.aliases(), &["remote-control"]);
    }

    #[test]
    fn bare_rc_activates_and_enable_remains_a_compatibility_alias() {
        assert_eq!(
            parse_remote_control_command(""),
            Ok(RemoteControlCommand::Start)
        );
        assert_eq!(
            parse_remote_control_command("enable"),
            Ok(RemoteControlCommand::Start)
        );
        assert_eq!(
            parse_remote_control_command("status"),
            Ok(RemoteControlCommand::Status)
        );
        assert_eq!(
            parse_remote_control_command("stop"),
            Ok(RemoteControlCommand::Stop)
        );
    }
}
