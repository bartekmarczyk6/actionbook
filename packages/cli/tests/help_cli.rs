use assert_cmd::Command;

#[test]
fn top_level_help_lists_setup_command() {
    let output = Command::cargo_bin("actionbook")
        .expect("binary exists")
        .arg("--help")
        .output()
        .expect("run --help");

    assert!(
        output.status.success(),
        "expected --help success\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("setup      Configure actionbook"),
        "top-level help should list setup as an available command\nstdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("setup      Coming soon"),
        "top-level help should no longer mark setup as coming soon\nstdout:\n{stdout}"
    );
}

#[test]
fn top_level_no_args_shows_content_first_home() {
    let output = Command::cargo_bin("actionbook")
        .expect("binary exists")
        .output()
        .expect("run without args");

    assert!(
        output.status.success(),
        "expected no-arg success\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("command: home"));
    assert!(
        stdout.contains("description: Manage browser automation actions in the current workspace")
    );
}
