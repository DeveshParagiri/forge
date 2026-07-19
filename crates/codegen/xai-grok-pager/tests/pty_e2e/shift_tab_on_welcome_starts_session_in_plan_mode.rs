// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use super::common::*;

/// 2b. **Shift+Tab on the welcome screen starts a session and cycles effort.**
/// Pressing Shift+Tab (BackTab, `ESC [ Z`) before typing anything promotes the
/// welcome prompt to an agent session, then forwards the key through the same
/// composed effort binding used by an established prompt.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn shift_tab_on_welcome_starts_session_and_cycles_effort() {
    let content = ContentController::start_with_models(vec![
        MockModel::new("test-model")
            .with_supports_reasoning_effort(true)
            .with_reasoning_effort("low")
            .with_reasoning_efforts(vec![
                json!({ "id": "low", "value": "low", "label": "Low" }),
                json!({ "id": "medium", "value": "medium", "label": "Medium" }),
                json!({ "id": "high", "value": "high", "label": "High" }),
            ]),
    ])
    .await
    .expect("start content");

    let binary = pager_binary().expect("resolve pager binary");
    let mut harness =
        PtyHarness::spawn_with_content(&binary, DEFAULT_ROWS, DEFAULT_COLS, &content, &[])
            .expect("spawn pager with content");

    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome text");

    // Shift+Tab → BackTab (CSI Z).
    harness.inject_keys(b"\x1b[Z").expect("inject BackTab");

    harness
        .wait_for_text("effort · medium", Duration::from_secs(10))
        .expect("welcome promotion followed by low -> medium effort cycle");

    harness.inject_keys(b"\x1b[Z").expect("inject BackTab");
    harness
        .wait_for_text("effort · high", Duration::from_secs(10))
        .expect("second Shift+Tab advances medium -> high");

    assert!(
        !harness.contains_text("Switched to mode:"),
        "welcome Shift+Tab must not change permission mode\nscreen:\n{}",
        harness.screen_contents()
    );

    harness.quit().expect("clean quit");
}
