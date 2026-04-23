# Config File Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add TOML-based user-level and project-level configuration files so users can set default themes and flags without repeating CLI arguments.

**Architecture:** New `src/config.rs` module handles discovery, parsing, merging, and validation. Config is loaded before CLI resolution in `main()` and `run_embed()`. A new `init` subcommand generates the user config. A `--config` flag allows explicit config file paths.

**Tech Stack:** `serde` (derive), `toml` crate, existing `clap` + `anyhow`

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `Cargo.toml` | Modify | Add `serde` and `toml` dependencies |
| `src/config.rs` | Create | Config struct, file discovery, parsing, merging, validation, default generation |
| `src/main.rs` | Modify | Add `mod config`, `Init` subcommand, `--config` flag to `Args`/`EmbedArgs`, wire config loading into main flow |
| `tests/integration.rs` | Modify | Integration tests for config discovery, precedence, `--config`, `init` |

---

### Task 1: Add dependencies

**Files:**
- Modify: `Cargo.toml:26-34`

- [ ] **Step 1: Add serde and toml to Cargo.toml**

In the `[dependencies]` section, add:

```toml
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: success, no errors

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat(config): add serde and toml dependencies"
```

---

### Task 2: Config struct and merge logic

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs:1-10` (add `mod config;`)

- [ ] **Step 1: Write the failing test for merge**

Create `src/config.rs` with the struct and test:

```rust
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub theme: Option<String>,
    pub ui_theme: Option<String>,
    pub pager: Option<bool>,
    pub width: Option<u16>,
    pub no_mermaid_rendering: Option<bool>,
    pub split_mermaid_rendering: Option<bool>,
}

impl Config {
    pub fn merge(self, other: Config) -> Config {
        Config {
            theme: other.theme.or(self.theme),
            ui_theme: other.ui_theme.or(self.ui_theme),
            pager: other.pager.or(self.pager),
            width: other.width.or(self.width),
            no_mermaid_rendering: other.no_mermaid_rendering.or(self.no_mermaid_rendering),
            split_mermaid_rendering: other.split_mermaid_rendering.or(self.split_mermaid_rendering),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_other_some_overrides_self_none() {
        let base = Config::default();
        let overlay = Config {
            theme: Some("nord".to_string()),
            ..Config::default()
        };
        let merged = base.merge(overlay);
        assert_eq!(merged.theme.as_deref(), Some("nord"));
    }

    #[test]
    fn merge_other_some_overrides_self_some() {
        let base = Config {
            ui_theme: Some("clay".to_string()),
            ..Config::default()
        };
        let overlay = Config {
            ui_theme: Some("frost".to_string()),
            ..Config::default()
        };
        let merged = base.merge(overlay);
        assert_eq!(merged.ui_theme.as_deref(), Some("frost"));
    }

    #[test]
    fn merge_other_none_preserves_self_some() {
        let base = Config {
            pager: Some(true),
            width: Some(120),
            ..Config::default()
        };
        let overlay = Config::default();
        let merged = base.merge(overlay);
        assert_eq!(merged.pager, Some(true));
        assert_eq!(merged.width, Some(120));
    }

    #[test]
    fn merge_both_none_stays_none() {
        let base = Config::default();
        let overlay = Config::default();
        let merged = base.merge(overlay);
        assert!(merged.theme.is_none());
        assert!(merged.pager.is_none());
    }
}
```

Add `mod config;` to `src/main.rs` after line 9 (`mod watch;`):

```rust
mod config;
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test config::tests`
Expected: all 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat(config): add Config struct with merge logic and tests"
```

---

### Task 3: TOML parsing with from_file

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing tests for from_file**

Add to the `impl Config` block in `src/config.rs`:

```rust
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Config> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file {}", path.display()))?;
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("failed to parse config file {}", path.display()))?;
        Ok(config)
    }
```

Add the import at the top of `src/config.rs`:

```rust
use anyhow::Context;
```

Add tests to the `mod tests` block:

```rust
    #[test]
    fn from_file_parses_valid_toml() {
        let dir = std::env::temp_dir().join("mdx_config_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("valid.toml");
        std::fs::write(&path, "theme = \"nord\"\nui_theme = \"frost\"\npager = true\n").unwrap();
        let config = Config::from_file(&path).unwrap();
        assert_eq!(config.theme.as_deref(), Some("nord"));
        assert_eq!(config.ui_theme.as_deref(), Some("frost"));
        assert_eq!(config.pager, Some(true));
        assert!(config.width.is_none());
    }

    #[test]
    fn from_file_rejects_unknown_keys() {
        let dir = std::env::temp_dir().join("mdx_config_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("unknown_key.toml");
        std::fs::write(&path, "colour = \"red\"\n").unwrap();
        let result = Config::from_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("failed to parse config file"), "err: {}", err);
    }

    #[test]
    fn from_file_rejects_invalid_toml() {
        let dir = std::env::temp_dir().join("mdx_config_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad_syntax.toml");
        std::fs::write(&path, "theme = \n").unwrap();
        let result = Config::from_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn from_file_errors_on_missing_file() {
        let path = std::path::Path::new("/tmp/mdx_config_nonexistent_file.toml");
        let result = Config::from_file(path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("failed to read config file"), "err: {}", err);
    }

    #[test]
    fn from_file_handles_empty_file() {
        let dir = std::env::temp_dir().join("mdx_config_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.toml");
        std::fs::write(&path, "").unwrap();
        let config = Config::from_file(&path).unwrap();
        assert!(config.theme.is_none());
        assert!(config.pager.is_none());
    }

    #[test]
    fn from_file_parses_all_fields() {
        let dir = std::env::temp_dir().join("mdx_config_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("all_fields.toml");
        std::fs::write(
            &path,
            "theme = \"base16-eighties.dark\"\nui_theme = \"nord\"\npager = false\nwidth = 100\nno_mermaid_rendering = true\nsplit_mermaid_rendering = false\n",
        ).unwrap();
        let config = Config::from_file(&path).unwrap();
        assert_eq!(config.theme.as_deref(), Some("base16-eighties.dark"));
        assert_eq!(config.ui_theme.as_deref(), Some("nord"));
        assert_eq!(config.pager, Some(false));
        assert_eq!(config.width, Some(100));
        assert_eq!(config.no_mermaid_rendering, Some(true));
        assert_eq!(config.split_mermaid_rendering, Some(false));
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test config::tests`
Expected: all 10 tests pass

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add TOML file parsing with from_file"
```

---

### Task 4: Config file discovery and load

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add discovery functions and load orchestrator**

Add to the `impl Config` block in `src/config.rs`:

```rust
    /// Discover and load config, merging layers.
    /// If `cli_config_path` is Some, only that file is loaded (error if missing).
    /// Otherwise: user config (optional) merged with project config (optional).
    pub fn load(cli_config_path: Option<&std::path::Path>) -> anyhow::Result<Config> {
        if let Some(path) = cli_config_path {
            if !path.exists() {
                anyhow::bail!("config file not found: {}", path.display());
            }
            return Config::from_file(path);
        }

        let mut config = Config::default();

        if let Some(user_path) = Config::user_config_path() {
            if user_path.exists() {
                config = config.merge(Config::from_file(&user_path)?);
            }
        }

        if let Some(project_path) = Config::project_config_path() {
            if project_path.exists() {
                config = config.merge(Config::from_file(&project_path)?);
            }
        }

        Ok(config)
    }

    /// Returns path to user config: $XDG_CONFIG_HOME/mdx/config.toml
    /// Falls back to ~/.config/mdx/config.toml
    pub fn user_config_path() -> Option<std::path::PathBuf> {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| std::path::PathBuf::from(h).join(".config"))
            })?;
        Some(config_dir.join("mdx").join("config.toml"))
    }

    /// Walk from CWD upward to find a directory containing .git,
    /// then check for .mdx.toml there.
    pub fn project_config_path() -> Option<std::path::PathBuf> {
        let mut dir = std::env::current_dir().ok()?;
        loop {
            if dir.join(".git").exists() {
                let config_path = dir.join(".mdx.toml");
                return Some(config_path);
            }
            if !dir.pop() {
                return None;
            }
        }
    }
```

- [ ] **Step 2: Write tests for load and discovery**

Add to the `mod tests` block:

```rust
    #[test]
    fn load_explicit_config_path() {
        let dir = std::env::temp_dir().join("mdx_config_test_load");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("explicit.toml");
        std::fs::write(&path, "ui_theme = \"nord\"\n").unwrap();
        let config = Config::load(Some(&path)).unwrap();
        assert_eq!(config.ui_theme.as_deref(), Some("nord"));
    }

    #[test]
    fn load_explicit_config_path_missing_errors() {
        let path = std::path::Path::new("/tmp/mdx_config_does_not_exist.toml");
        let result = Config::load(Some(path));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("config file not found"), "err: {}", err);
    }

    #[test]
    fn load_no_config_files_returns_default() {
        // With a bogus XDG and no .git ancestor, should return defaults
        let config = std::panic::catch_unwind(|| {
            // This is a best-effort test — in CI the real user config
            // or project config might exist, so we just check it doesn't crash.
            Config::load(None)
        });
        assert!(config.is_ok());
    }

    #[test]
    fn user_config_path_respects_xdg() {
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/mdx_xdg_test");
        let path = Config::user_config_path().unwrap();
        assert_eq!(path, std::path::PathBuf::from("/tmp/mdx_xdg_test/mdx/config.toml"));
        std::env::remove_var("XDG_CONFIG_HOME");
    }
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test config::tests`
Expected: all 14 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add config file discovery and layered loading"
```

---

### Task 5: Generate default config (for init subcommand)

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add generate_default function**

Add to the `impl Config` block in `src/config.rs`:

```rust
    /// Generate the default config file content with all keys commented out.
    pub fn generate_default() -> &'static str {
        "\
# Syntax highlighting theme for code blocks
# theme = \"base16-ocean.dark\"

# UI theme for headers, text, and chrome
# ui_theme = \"clay\"

# Force pager mode
# pager = false

# Terminal width override (omit to use terminal width)
# width = 100

# Mermaid diagram rendering
# no_mermaid_rendering = false
# split_mermaid_rendering = false
"
    }
```

- [ ] **Step 2: Write test that default content is valid TOML when uncommented**

Add to the `mod tests` block:

```rust
    #[test]
    fn generate_default_is_valid_toml_when_uncommented() {
        let default = Config::generate_default();
        // Uncomment all lines
        let uncommented: String = default
            .lines()
            .map(|line| {
                if let Some(stripped) = line.strip_prefix("# ") {
                    stripped
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let result: Result<Config, _> = toml::from_str(&uncommented);
        assert!(result.is_ok(), "uncommented default should parse: {:?}", result.err());
    }
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test config::tests::generate_default`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add generate_default for init subcommand"
```

---

### Task 6: Add --config flag and Init subcommand to CLI

**Files:**
- Modify: `src/main.rs:47-55` (Commands enum)
- Modify: `src/main.rs:57-96` (Args struct)
- Modify: `src/main.rs:98-126` (EmbedArgs struct)

- [ ] **Step 1: Add Init to Commands enum**

In `src/main.rs`, add `Init` to the `Commands` enum:

```rust
#[derive(clap::Subcommand)]
enum Commands {
    /// Update mdx to the latest version
    Update,
    /// Render markdown into a bounded ANSI stream for embedding in other TUIs
    Embed(EmbedArgs),
    /// Preview all available UI themes with sample markdown
    PreviewThemes,
    /// Generate a default user config file at ~/.config/mdx/config.toml
    Init,
}
```

- [ ] **Step 2: Add --config flag to Args**

Add to the `Args` struct, after the `ui_theme` field:

```rust
    /// Path to a config file (overrides user and project config)
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
```

- [ ] **Step 3: Add --config flag to EmbedArgs**

Add to the `EmbedArgs` struct, after the `ui_theme` field:

```rust
    /// Path to a config file (overrides user and project config)
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
```

- [ ] **Step 4: Write unit tests for new CLI args**

Add to `mod tests` in `src/main.rs`:

```rust
    #[test]
    fn test_init_subcommand() {
        let cli = Cli::parse_from(["mdx", "init"]);
        assert!(matches!(cli.command, Some(Commands::Init)));
    }

    #[test]
    fn test_config_flag() {
        let cli = Cli::parse_from(["mdx", "--config", "/tmp/my.toml", "test.md"]);
        assert_eq!(cli.args.config, Some(PathBuf::from("/tmp/my.toml")));
    }

    #[test]
    fn test_embed_config_flag() {
        let cli = Cli::parse_from(["mdx", "embed", "--config", "/tmp/my.toml", "file.md"]);
        match cli.command {
            Some(Commands::Embed(args)) => {
                assert_eq!(args.config, Some(PathBuf::from("/tmp/my.toml")));
            }
            _ => panic!("expected Embed subcommand"),
        }
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -- test_init_subcommand test_config_flag test_embed_config_flag`
Expected: all 3 pass

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat(config): add --config flag and init subcommand to CLI"
```

---

### Task 7: Wire config loading into main flow

**Files:**
- Modify: `src/main.rs:233-290` (main function)
- Modify: `src/main.rs:292-331` (run_embed function)

- [ ] **Step 1: Implement the Init subcommand handler**

In `src/main.rs`, add a `run_init` function before `main()`:

```rust
fn run_init() -> Result<()> {
    let path = config::Config::user_config_path()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    if path.exists() {
        anyhow::bail!("Config file already exists: {}", path.display());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, config::Config::generate_default())?;
    println!("Config file created: {}", path.display());
    Ok(())
}
```

- [ ] **Step 2: Add Init to the subcommand match**

In the `main()` function, add `Init` to the match on `cli.command`:

```rust
    match cli.command {
        Some(Commands::Update) => return self_update::run(),
        Some(Commands::Embed(eargs)) => return run_embed(eargs),
        Some(Commands::PreviewThemes) => return preview::run(),
        Some(Commands::Init) => return run_init(),
        None => {}
    }
```

- [ ] **Step 3: Wire config loading into main path**

In `main()`, after `let args = cli.args;` and before the `--theme=list` check, add config loading. Then update the resolution lines to use config values:

```rust
    let args = cli.args;

    // Load config (user + project, or explicit --config)
    let config = config::Config::load(args.config.as_deref())?;

    // Handle --theme=list before reading input
    if args.theme.as_deref() == Some("list") {
        let h = highlight::Highlighter::new(None).map_err(|e| anyhow::anyhow!(e))?;
        for name in h.available_themes() {
            println!("{}", name);
        }
        return Ok(());
    }

    // Handle --ui-theme=list before reading input
    if args.ui_theme.as_deref() == Some("list") {
        for name in theme::Theme::available_names() {
            println!("{}", name);
        }
        return Ok(());
    }

    // Validate watch args early
    validate_watch_args(&args)?;

    let width = resolve_width(args.width.or(config.width));
    let no_color = std::env::var("NO_COLOR").is_ok();
    let theme_name = args.theme.or(config.theme);
    let highlighter =
        highlight::Highlighter::new(theme_name).map_err(|e| anyhow::anyhow!(e))?;
    let ui_theme_name = args.ui_theme.or(config.ui_theme);
    let ui_theme = resolve_ui_theme(ui_theme_name.as_deref())?;
    let mermaid_mode = resolve_mermaid_mode(
        args.no_mermaid_rendering || config.no_mermaid_rendering.unwrap_or(false),
        args.split_mermaid_rendering || config.split_mermaid_rendering.unwrap_or(false),
    );
    let use_pager = if args.no_pager {
        false
    } else if args.pager || config.pager.unwrap_or(false) {
        true
    } else {
        std::io::stdout().is_terminal()
    };
```

Then update the pager decision at the bottom of `main()` to use the local `use_pager` variable instead of calling the function:

```rust
    if use_pager {
        setup_panic_hook();
        pager::run_pager(rendered, ui_theme)?;
    } else {
        pipe_output(&rendered, no_color)?;
    }
```

- [ ] **Step 4: Wire config loading into run_embed**

Update `run_embed` to load config before resolving options:

```rust
fn run_embed(eargs: EmbedArgs) -> Result<()> {
    // Load config
    let config = config::Config::load(eargs.config.as_deref())?;

    // theme=list / ui-theme=list short-circuits
    if eargs.theme.as_deref() == Some("list") {
        let h = highlight::Highlighter::new(None).map_err(|e| anyhow::anyhow!(e))?;
        for name in h.available_themes() {
            println!("{}", name);
        }
        return Ok(());
    }
    if eargs.ui_theme.as_deref() == Some("list") {
        for name in theme::Theme::available_names() {
            println!("{}", name);
        }
        return Ok(());
    }

    let width = resolve_width(eargs.width.or(config.width));
    let no_color = std::env::var("NO_COLOR").is_ok();
    let theme_name = eargs.theme.or(config.theme);
    let highlighter =
        highlight::Highlighter::new(theme_name).map_err(|e| anyhow::anyhow!(e))?;
    let ui_theme_name = eargs.ui_theme.or(config.ui_theme);
    let ui_theme = resolve_ui_theme(ui_theme_name.as_deref())?;
    let mermaid_mode = resolve_mermaid_mode(
        eargs.no_mermaid_rendering || config.no_mermaid_rendering.unwrap_or(false),
        eargs.split_mermaid_rendering || config.split_mermaid_rendering.unwrap_or(false),
    );

    let input = read_input_from(eargs.file.as_deref(), std::io::stdin().is_terminal())?;
    let opts = embed::EmbedOptions {
        width,
        height: eargs.height,
        no_color,
    };
    let mut stdout = std::io::stdout().lock();
    embed::run(
        &input,
        opts,
        &highlighter,
        ui_theme,
        mermaid_mode,
        &mut stdout,
    )
}
```

- [ ] **Step 5: Update validate_watch_args and test structs**

The `Args` struct now has a `config` field. Update `validate_watch_args` tests in `src/main.rs` that construct `Args` manually — add `config: None` to each:

```rust
        let args = Args {
            file: None,
            pager: false,
            no_pager: false,
            watch: true,
            width: None,
            theme: None,
            ui_theme: None,
            config: None,
            no_mermaid_rendering: false,
            split_mermaid_rendering: false,
        };
```

Apply the same to all three test functions: `test_watch_requires_file`, `test_watch_conflicts_with_no_pager`, `test_watch_valid_args`.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: all tests pass (unit + integration)

- [ ] **Step 7: Commit**

```bash
git add src/main.rs
git commit -m "feat(config): wire config loading into main and embed flows"
```

---

### Task 8: Integration tests

**Files:**
- Modify: `tests/integration.rs`

- [ ] **Step 1: Write integration test for --config flag**

Add to `tests/integration.rs`:

```rust
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
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}
```

- [ ] **Step 2: Write integration test for --config with missing file**

```rust
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
    assert!(stderr.contains("config file not found"), "stderr: {}", stderr);
}
```

- [ ] **Step 3: Write integration test for --config with invalid TOML**

```rust
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
    assert!(stderr.contains("failed to parse config file"), "stderr: {}", stderr);
}
```

- [ ] **Step 4: Write integration test for --config with unknown keys**

```rust
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
    assert!(stderr.contains("failed to parse config file"), "stderr: {}", stderr);
}
```

- [ ] **Step 5: Write integration test for init subcommand**

```rust
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
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Config file created"), "stdout: {}", stdout);
    let config_path = dir.join("mdx").join("config.toml");
    assert!(config_path.exists(), "config file should exist at {:?}", config_path);
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("# theme"), "should contain commented defaults: {}", content);
}
```

- [ ] **Step 6: Write integration test for init refusing to overwrite**

```rust
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
```

- [ ] **Step 7: Write integration test for embed --config**

```rust
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
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}
```

- [ ] **Step 8: Write integration test for project config discovery**

```rust
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
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}
```

- [ ] **Step 9: Write integration test for CLI flag overriding config**

```rust
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
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}
```

- [ ] **Step 10: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 11: Commit**

```bash
git add tests/integration.rs
git commit -m "test(config): add integration tests for config loading, init, and precedence"
```

---

### Task 9: Final validation

- [ ] **Step 1: Run full test suite one more time**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: no warnings

- [ ] **Step 3: Manual smoke test**

Run: `cargo run -- init`
Expected: prints path, creates config file

Run: `cat ~/.config/mdx/config.toml`
Expected: shows commented-out defaults

Run: `cargo run -- init`
Expected: errors with "already exists"

Run: `cargo run -- --config ~/.config/mdx/config.toml --no-pager README.md`
Expected: renders README normally (all values commented out = defaults)

- [ ] **Step 4: Commit any fixes if needed**
