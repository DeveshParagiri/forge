// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use crate::common::*;

/// Minimal mode uses the same prompt-level Shift+Tab effort action as the full
/// TUI. The default mock model has no effort capability, so the action must
/// explain that limitation without changing the session to Plan mode.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn minimal_shift_tab_preserves_permission_mode() {
    let content = ContentController::start().await.expect("start content");
    let mut harness = spawn_minimal(&content);
    wait_minimal_ready(&mut harness);

    harness.inject_keys(b"\x1b[Z").expect("inject BackTab");
    harness
        .wait_for_text("does not support reasoning effort", Duration::from_secs(10))
        .expect("unsupported-effort toast after Shift+Tab");

    assert!(
        !harness.contains_text("Switched to mode: Plan"),
        "minimal Shift+Tab must not change permission mode\nscreen:\n{}",
        harness.screen_contents()
    );

    assert!(
        !harness.contains_text("panicked"),
        "pager panicked\nscreen:\n{}",
        harness.screen_contents()
    );

    quit_minimal(&mut harness);
}
