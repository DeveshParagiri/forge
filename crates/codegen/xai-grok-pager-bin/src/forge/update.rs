//! Forge source-checkout update command.
//!
//! The release installer owns the update mechanics. This module gives
//! `forge update` a Forge-aware entry point while preserving the stock updater
//! for unsupported flags and non-Forge installations.

use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::process::Command;

const DEFAULT_UPDATER: &str = "~/bin/forge-update-from-source";

/// Returns whether Forge should handle this invocation instead of the stock
/// release-channel updater.
pub(crate) fn should_handle(
    check: bool,
    json: bool,
    force_reinstall: bool,
    version: Option<&str>,
    alpha: bool,
    stable: bool,
    enterprise: bool,
) -> bool {
    !check
        && !json
        && !force_reinstall
        && version.is_none()
        && !alpha
        && !stable
        && !enterprise
        && updater_path().is_file()
}

/// Run the same safe source updater installed by `scripts/install`.
pub(crate) fn run() -> Result<()> {
    let updater = updater_path();
    if !updater.is_file() {
        bail!(
            "Forge updater not found at {}. Re-run the Forge installer or use {} directly after installation.",
            updater.display(),
            DEFAULT_UPDATER
        );
    }

    let mut command = updater_command(&updater);
    let status = command
        .status()
        .with_context(|| format!("failed to start Forge updater at {}", updater.display()))?;
    if !status.success() {
        bail!("Forge updater exited with {status}");
    }
    Ok(())
}

fn updater_command(updater: &std::path::Path) -> Command {
    if cfg!(windows) {
        let mut command = Command::new("powershell");
        command.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]);
        command.arg(updater);
        command
    } else {
        Command::new(updater)
    }
}

fn updater_path() -> PathBuf {
    if let Some(path) = std::env::var_os("FORGE_UPDATER") {
        return PathBuf::from(path);
    }
    if let Some(path) = std::env::var_os("GROK_UPDATER") {
        return PathBuf::from(path);
    }
    let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
    let forge_path = home.join(default_updater_relpath());
    if forge_path.is_file() {
        forge_path
    } else {
        home.join(legacy_updater_relpath())
    }
}

fn default_updater_relpath() -> &'static str {
    if cfg!(windows) {
        "bin/forge-update-from-source.ps1"
    } else {
        "bin/forge-update-from-source"
    }
}

fn legacy_updater_relpath() -> &'static str {
    if cfg!(windows) {
        "bin/grok-update-from-source.ps1"
    } else {
        "bin/grok-update-from-source"
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|home| !home.is_empty()))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn explicit_stock_update_options_are_not_intercepted() {
        assert!(!should_handle(
            true, false, false, None, false, false, false
        ));
        assert!(!should_handle(
            false,
            false,
            false,
            Some("0.2.105"),
            false,
            false,
            false
        ));
        assert!(!should_handle(
            false, false, false, None, true, false, false
        ));
    }

    #[test]
    fn plain_update_uses_an_installed_forge_updater() {
        let _guard = ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!("forge-update-test-{}", std::process::id()));
        let updater = root.join("forge-update-from-source");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&updater, "#!/bin/sh\nexit 0\n").unwrap();
        unsafe { std::env::set_var("GROK_UPDATER", &updater) };

        assert!(should_handle(
            false, false, false, None, false, false, false
        ));

        unsafe { std::env::remove_var("GROK_UPDATER") };
        std::fs::remove_dir_all(root).unwrap();
    }
}
