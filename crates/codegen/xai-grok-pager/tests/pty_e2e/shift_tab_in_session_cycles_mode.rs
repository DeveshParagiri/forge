// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use super::common::*;

// ── Interactive flow e2e tests ──────────────────────────────────────────

/// 15. **In-session Shift+Tab cycles reasoning effort.**
/// Routes BackTab through the composed prompt registry and verifies that the
/// runtime action advances the active model's advertised effort menu.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn shift_tab_in_session_cycles_reasoning_effort() {
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
    content.set_response(format!("{MOCK_RESPONSE_SENTINEL} turn done."));

    let binary = pager_binary().expect("resolve pager binary");
    let mut harness =
        PtyHarness::spawn_with_content(&binary, DEFAULT_ROWS, DEFAULT_COLS, &content, &[])
            .expect("spawn pager");

    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome text");

    harness
        .inject_keys(format!("{PROMPT}\r").as_bytes())
        .expect("submit prompt");
    harness
        .wait_for_text(MOCK_RESPONSE_SENTINEL, Duration::from_secs(30))
        .expect("turn rendered");

    harness.inject_keys(b"\x1b[Z").expect("inject BackTab");
    harness
        .wait_for_text("effort · medium", Duration::from_secs(10))
        .expect("first cycle: low -> medium");

    harness.inject_keys(b"\x1b[Z").expect("inject BackTab");
    harness
        .wait_for_text("effort · high", Duration::from_secs(10))
        .expect("second cycle: medium -> high");

    harness.inject_keys(b"\x1b[Z").expect("inject BackTab");
    harness
        .wait_for_text("effort · low", Duration::from_secs(10))
        .expect("third cycle: high -> low (full loop)");

    let screen = harness.screen_contents();
    assert!(
        !screen.contains("Switched to mode:"),
        "effort cycling must not change permission mode\nscreen:\n{screen}"
    );

    assert!(
        !harness.contains_text("panicked"),
        "pager panicked\nscreen:\n{}",
        harness.screen_contents()
    );

    harness.quit().expect("clean quit");
}
