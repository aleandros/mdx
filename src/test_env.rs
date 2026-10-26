//! Run environment-dependent tests in a separate process, without unsafe env writes.

pub(crate) fn in_child(test_name: &str, config_home: &std::path::Path) -> bool {
    if std::env::var("MDX_ENV_TEST_CHILD").as_deref() == Ok(test_name) {
        return true;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test_name, "--nocapture"])
        .env("MDX_ENV_TEST_CHILD", test_name)
        .env("XDG_CONFIG_HOME", config_home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{test_name} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}
