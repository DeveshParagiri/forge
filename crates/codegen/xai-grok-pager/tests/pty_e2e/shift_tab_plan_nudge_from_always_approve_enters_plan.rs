// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use super::common::*;

/// The plan nudge advertises `/plan`; Shift+Tab remains an effort binding even
/// when Always-Approve is active and planning language triggered the nudge.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn plan_nudge_does_not_rebind_shift_tab_from_effort() {
    let content = ContentController::start_with_models(vec![
        MockModel::new("test-model")
            .with_supports_reasoning_effort(true)
            .with_reasoning_effort("low")
            .with_reasoning_efforts(vec![
                json!({ "id": "low", "value": "low", "label": "Low" }),
                json!({ "id": "medium", "value": "medium", "label": "Medium" }),
            ]),
    ])
    .await
    .expect("start content");
    content.set_response(format!("{MOCK_RESPONSE_SENTINEL} turn done."));

    let binary = pager_binary().expect("resolve pager binary");
    // --yolo/--trust seed Always-Approve; hints env opts the tip in; CWD is
    // the sandboxed content home so trust resolves against the same tree.
    let env = contextual_hints_env(&content);
    let env_refs: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let mut harness = PtyHarness::new_in_dir(
        &binary,
        DEFAULT_ROWS,
        DEFAULT_COLS,
        &["--yolo", "--trust"],
        &env_refs,
        Some(content.home()),
    )
    .expect("spawn pager in always-approve");

    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome");

    // Promote to idle (nudge is gated on session.state.is_idle()).
    harness
        .inject_keys(format!("{PROMPT}\r").as_bytes())
        .expect("submit prompt");
    harness
        .wait_for_text(MOCK_RESPONSE_SENTINEL, Duration::from_secs(30))
        .expect("turn rendered");
    harness
        .wait_for_turn_idle(Duration::from_secs(15))
        .expect("turn idle before plan nudge");

    harness
        .inject_keys(b"plan the refactor")
        .expect("type planning keyword");
    harness
        .wait_for_text("Enter plan mode with", Duration::from_secs(10))
        .unwrap_or_else(|e| {
            panic!(
                "plan nudge must show; {e}\nscreen:\n{}",
                harness.screen_contents()
            )
        });

    harness.inject_keys(b"\x1b[Z").expect("inject Shift+Tab");
    harness
        .wait_for_text("effort · medium", Duration::from_secs(10))
        .unwrap_or_else(|e| {
            panic!(
                "nudge must leave Shift+Tab bound to effort; {e}\nscreen:\n{}",
                harness.screen_contents()
            )
        });

    assert!(
        !harness.contains_text("Switched to mode:"),
        "effort binding must preserve Always-Approve; screen:\n{}",
        harness.screen_contents()
    );
    assert!(
        !harness.contains_text("panicked"),
        "pager panicked\nscreen:\n{}",
        harness.screen_contents()
    );

    harness.quit().expect("clean quit");
}
