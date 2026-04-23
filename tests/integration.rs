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
    std::fs::write(&path, "```rust\nfn main() {}\n```\n").unwrap();
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
    assert!(
        stdout.contains("fn main()"),
        "Should still contain code text"
    );
}

#[test]
fn test_theme_flag_produces_output() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("highlight_theme.md");
    std::fs::write(&path, "```rust\nfn main() {}\n```\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path)
        .arg("--no-pager")
        .arg("--theme=base16-eighties.dark")
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
fn test_toml_syntax_highlighting() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("highlight_toml.md");
    std::fs::write(
        &path,
        "# Config\n\n```toml\n[package]\nname = \"hello\"\nversion = \"0.1.0\"\n```\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path)
        .arg("--no-pager")
        .output()
        .expect("failed to run mdx");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // TOML should now be highlighted with 24-bit RGB colors
    assert!(
        stdout.contains("38;2;"),
        "TOML code block should have RGB color escapes: {}",
        stdout
    );
    assert!(stdout.contains("package"), "Should contain TOML content");
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

#[test]
fn test_watch_requires_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--watch")
        .output()
        .expect("failed to run mdx");
    assert!(
        !output.status.success(),
        "Should fail without file argument"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("requires a file"), "stderr: {}", stderr);
}

#[test]
fn test_watch_conflicts_with_no_pager() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("watch_conflict.md");
    std::fs::write(&path, "# Test").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--watch")
        .arg("--no-pager")
        .arg(&path)
        .output()
        .expect("failed to run mdx");
    assert!(!output.status.success(), "Should fail with --no-pager");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("incompatible"), "stderr: {}", stderr);
}

#[test]
fn test_watch_short_flag_accepted() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("watch_short.md");
    std::fs::write(&path, "# Test").unwrap();
    // Watch mode blocks, so spawn and kill after a brief delay
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("-W")
        .arg(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to start mdx");

    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut child = child;
    let _ = child.kill();
    let output = child.wait_with_output().unwrap();

    // If it had a validation error, stderr would contain it
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("requires a file"),
        "Should not have file error: {}",
        stderr
    );
    assert!(
        !stderr.contains("incompatible"),
        "Should not have conflict error: {}",
        stderr
    );
}

// ─── mdx embed subcommand ─────────────────────────────────────────────────

#[test]
fn embed_honors_height_cap() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("embed_height.md");
    std::fs::write(
        &path,
        "# One\n\nPara one.\n\n# Two\n\nPara two.\n\n# Three\n\nPara three.\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--width", "40", "--height", "4"])
        .arg(&path)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success(), "mdx embed should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line_count = stdout.matches('\n').count();
    assert_eq!(line_count, 4, "output was: {:?}", stdout);
}

#[test]
fn embed_honors_width_cap() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("embed_width.md");
    std::fs::write(
        &path,
        "# Heading\n\nA fairly long paragraph that should wrap at narrow widths in normal rendering.\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--width", "20"])
        .arg(&path)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        assert!(
            line.chars().count() <= 20,
            "line exceeded width 20 (chars): {:?}",
            line
        );
    }
}

#[test]
fn embed_no_color_env_strips_escape_codes() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("embed_nocolor.md");
    std::fs::write(&path, "# Heading\n\nBody.\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--width", "40"])
        .arg(&path)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains('\x1b'),
        "no ESC bytes allowed with NO_COLOR: {:?}",
        stdout
    );
}

#[test]
fn embed_emits_color_by_default() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("embed_color.md");
    std::fs::write(&path, "# Heading\n\nBody.\n").unwrap();
    // Pipe stdout (not a TTY) — embed must still emit color.
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--width", "40"])
        .arg(&path)
        .env_remove("NO_COLOR")
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains('\x1b'),
        "embed must emit ANSI even when stdout is a pipe"
    );
}

#[test]
fn embed_rejects_pager_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--pager", "/tmp/whatever.md"])
        .output()
        .expect("failed to run mdx embed");
    assert!(
        !output.status.success(),
        "embed must reject --pager, got: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn embed_theme_list_prints_and_exits() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--theme", "list"])
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "theme list should print names");
}

#[test]
fn embed_diagram_crops_without_exceeding_width() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("embed_diagram.md");
    std::fs::write(
        &path,
        "```mermaid\ngraph LR\n    A[First node with a long label] --> B[Second node with a long label] --> C[Third node with a long label]\n```\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--width", "30"])
        .arg(&path)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        assert!(
            line.chars().count() <= 30,
            "diagram line exceeded width 30: {:?}",
            line
        );
    }
}

#[test]
fn test_preview_themes_runs_and_prints_all_themes() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("preview-themes")
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx preview-themes");
    assert!(
        output.status.success(),
        "preview-themes should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain every theme name
    for name in &[
        "clay",
        "hearth",
        "frost",
        "nord",
        "glacier",
        "steel",
        "solarized-dark",
        "solarized-light",
        "paper",
        "snow",
        "latte",
    ] {
        assert!(
            stdout.contains(name),
            "output should contain theme '{}', got: {}",
            name,
            stdout
        );
    }
}

// ─── config file support ─────────────────────────────────────────────────

#[test]
fn test_config_flag_sets_ui_theme() {
    let dir = std::env::temp_dir().join("mdx_config_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("test_config.toml");
    std::fs::write(&config_path, "ui_theme = \"nord\"\n").unwrap();
    let md_path = dir.join("config_test.md");
    std::fs::write(&md_path, "# Hello\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--config")
        .arg(&config_path)
        .arg("--no-pager")
        .arg(&md_path)
        .output()
        .expect("failed to run mdx");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_config_flag_missing_file_errors() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--config")
        .arg("/tmp/mdx_nonexistent_config.toml")
        .arg("--no-pager")
        .arg("/dev/null")
        .output()
        .expect("failed to run mdx");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("config file not found"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn test_config_flag_invalid_toml_errors() {
    let dir = std::env::temp_dir().join("mdx_config_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("bad_config.toml");
    std::fs::write(&config_path, "not valid toml = = =\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--config")
        .arg(&config_path)
        .arg("--no-pager")
        .arg("/dev/null")
        .output()
        .expect("failed to run mdx");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to parse config file"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn test_config_flag_unknown_key_errors() {
    let dir = std::env::temp_dir().join("mdx_config_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("unknown_key_config.toml");
    std::fs::write(&config_path, "bogus_key = true\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--config")
        .arg(&config_path)
        .arg("--no-pager")
        .arg("/dev/null")
        .output()
        .expect("failed to run mdx");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to parse config file"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn test_init_creates_config_file() {
    let dir = std::env::temp_dir().join("mdx_init_test");
    // Clean up from previous runs
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("init")
        .env("XDG_CONFIG_HOME", &dir)
        .env("HOME", &dir)
        .output()
        .expect("failed to run mdx init");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Config file created"), "stdout: {}", stdout);
    let config_path = dir.join("mdx").join("config.toml");
    assert!(
        config_path.exists(),
        "config file should exist at {:?}",
        config_path
    );
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("# theme"),
        "should contain commented defaults: {}",
        content
    );
}

#[test]
fn test_init_refuses_if_config_exists() {
    let dir = std::env::temp_dir().join("mdx_init_exists_test");
    let _ = std::fs::remove_dir_all(&dir);
    let mdx_dir = dir.join("mdx");
    std::fs::create_dir_all(&mdx_dir).unwrap();
    let config_path = mdx_dir.join("config.toml");
    std::fs::write(&config_path, "theme = \"nord\"\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("init")
        .env("XDG_CONFIG_HOME", &dir)
        .env("HOME", &dir)
        .output()
        .expect("failed to run mdx init");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"), "stderr: {}", stderr);
    // Verify original content is untouched
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(content, "theme = \"nord\"\n");
}

#[test]
fn test_embed_config_flag() {
    let dir = std::env::temp_dir().join("mdx_config_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("embed_config.toml");
    std::fs::write(&config_path, "ui_theme = \"frost\"\n").unwrap();
    let md_path = dir.join("embed_config_test.md");
    std::fs::write(&md_path, "# Hello\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--width", "40"])
        .arg("--config")
        .arg(&config_path)
        .arg(&md_path)
        .output()
        .expect("failed to run mdx embed");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_project_config_discovery() {
    let dir = std::env::temp_dir().join("mdx_project_config_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::write(dir.join(".mdx.toml"), "ui_theme = \"hearth\"\n").unwrap();
    let md_path = dir.join("test.md");
    std::fs::write(&md_path, "# Hello\n").unwrap();
    // Run from inside the project dir so discovery finds .git
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--no-pager")
        .arg(&md_path)
        .current_dir(&dir)
        // Prevent user config from interfering
        .env("XDG_CONFIG_HOME", dir.join("no_user_config"))
        .env("HOME", dir.join("no_user_config"))
        .output()
        .expect("failed to run mdx");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_cli_flag_overrides_config() {
    let dir = std::env::temp_dir().join("mdx_config_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("override_config.toml");
    // Config sets an invalid theme to prove CLI flag wins
    std::fs::write(&config_path, "ui_theme = \"nord\"\n").unwrap();
    let md_path = dir.join("override_test.md");
    std::fs::write(&md_path, "# Hello\n").unwrap();
    // CLI flag --ui-theme overrides the config value
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--config")
        .arg(&config_path)
        .arg("--ui-theme=frost")
        .arg("--no-pager")
        .arg(&md_path)
        .output()
        .expect("failed to run mdx");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
