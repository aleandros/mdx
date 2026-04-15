use std::process::Command;

#[test]
fn test_pipe_mode_renders_header() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.md");
    std::fs::write(&path, "# Hello World\n\nA paragraph.").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path).arg("--no-pager").env("NO_COLOR", "1")
        .output().expect("failed to run mdx");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello World"), "output was: {}", stdout);
    assert!(stdout.contains("A paragraph"), "output was: {}", stdout);
}

#[test]
fn test_pipe_mode_renders_mermaid() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mermaid.md");
    std::fs::write(&path, "# Chart\n\n```mermaid\ngraph TD\n    A[Start] --> B[End]\n```\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path).arg("--no-pager").env("NO_COLOR", "1")
        .output().expect("failed to run mdx");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Start"), "output was: {}", stdout);
    assert!(stdout.contains("End"), "output was: {}", stdout);
    assert!(stdout.contains('┌') || stdout.contains('│'), "output was: {}", stdout);
}

#[test]
fn test_sequence_diagram_renders() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("sequence.md");
    std::fs::write(
        &file,
        "# Test\n\n```mermaid\nsequenceDiagram\n    participant Alice\n    participant Bob\n    Alice->>Bob: Hello\n```\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(file.to_str().unwrap())
        .arg("--no-pager")
        .output()
        .expect("Failed to run mdx");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Alice"), "Should render participant Alice");
    assert!(stdout.contains("Bob"), "Should render participant Bob");
    assert!(stdout.contains("Hello"), "Should render message label");
}

#[test]
fn test_file_not_found() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("/tmp/nonexistent_mdx_test_file.md")
        .output().expect("failed to run mdx");
    assert!(!output.status.success());
}
