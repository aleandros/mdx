use std::process::Command;

#[test]
fn test_pipe_mode_renders_header() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.md");
    std::fs::write(&path, "# Hello World\n\nA paragraph.").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path)
        .arg("--no-pager")
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello World"), "output was: {}", stdout);
    assert!(stdout.contains("A paragraph"), "output was: {}", stdout);
}

#[test]
fn test_pipe_mode_renders_mermaid() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mermaid.md");
    std::fs::write(
        &path,
        "# Chart\n\n```mermaid\ngraph TD\n    A[Start] --> B[End]\n```\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path)
        .arg("--no-pager")
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Start"), "output was: {}", stdout);
    assert!(stdout.contains("End"), "output was: {}", stdout);
    assert!(
        stdout.contains('┌') || stdout.contains('│'),
        "output was: {}",
        stdout
    );
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
        .output()
        .expect("failed to run mdx");
    assert!(!output.status.success());
}

#[test]
fn test_syntax_highlighting_produces_colors() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("highlight.md");
    std::fs::write(
        &path,
        "# Code\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path)
        .arg("--no-pager")
        .output()
        .expect("failed to run mdx");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain ANSI escape codes for syntax highlighting
    assert!(
        stdout.contains("\x1b["),
        "Highlighted output should contain ANSI escapes: {}",
        stdout
    );
    assert!(stdout.contains("fn"), "Should contain the code text");
    assert!(stdout.contains("main"), "Should contain the code text");
}

#[test]
fn test_no_color_strips_highlighting() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("highlight_nocolor.md");
    std::fs::write(
        &path,
        "```rust\nfn main() {}\n```\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path)
        .arg("--no-pager")
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("\x1b["),
        "NO_COLOR output should have no ANSI escapes: {}",
        stdout
    );
    assert!(stdout.contains("fn main()"), "Should still contain code text");
}

#[test]
fn test_theme_flag_produces_output() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("highlight_theme.md");
    std::fs::write(
        &path,
        "```rust\nfn main() {}\n```\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path)
        .arg("--no-pager")
        .arg("--theme=base16-ocean.dark")
        .output()
        .expect("failed to run mdx");
    assert!(output.status.success(), "Should succeed with valid theme");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Named theme uses 24-bit color escapes (38;2;r;g;b)
    assert!(
        stdout.contains("38;2;"),
        "Named theme should use 24-bit RGB colors: {}",
        stdout
    );
}

#[test]
fn test_theme_list() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--theme=list")
        .output()
        .expect("failed to run mdx");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("base16-ocean.dark"),
        "Theme list should include base16-ocean.dark: {}",
        stdout
    );
}
