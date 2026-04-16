# Bundled Grammars Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bundle additional `.sublime-syntax` grammar files into the mdx binary at compile time, replacing syntect's limited defaults with a richer set covering TOML, TypeScript, Dockerfile, Bash (improved), and more.

**Architecture:** A `build.rs` script loads syntect's default grammars, merges in custom `.sublime-syntax` files from a `syntaxes/` directory, serializes the combined set to a `.packdump` file, which is then embedded into the binary via `include_bytes!`. At runtime, `highlight.rs` loads from the embedded packdump instead of syntect's defaults.

**Tech Stack:** syntect 5 (yaml-load, plist-load, dump-create, dump-load features), build.rs

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `syntaxes/*.sublime-syntax` | Create | Vendored grammar files from bat/Sublime ecosystem |
| `syntaxes/*.tmLanguage` | Create | Vendored grammar files (plist format, for Vue) |
| `syntaxes/NOTICE.md` | Create | Provenance, license, and update instructions for vendored files |
| `syntaxes/LICENSE-APACHE` | Create | Apache 2.0 license text (required for TypeScript, Kotlin) |
| `build.rs` | Create | Compile-time syntax set builder + packdump serializer |
| `Cargo.toml` | Modify | Add syntect features + build-dependencies |
| `src/highlight.rs` | Modify | Load syntax set from embedded packdump instead of defaults |
| `tests/integration.rs` | Modify | Add TOML highlighting integration test |

---

### Task 1: Vendor syntax files and create compliance docs

**Files:**
- Create: `syntaxes/` directory and all `.sublime-syntax` / `.tmLanguage` files
- Create: `syntaxes/NOTICE.md`
- Create: `syntaxes/LICENSE-APACHE`

This task downloads all grammar files and creates the compliance documentation. No Rust code changes yet.

- [ ] **Step 1: Create the syntaxes directory**

```bash
mkdir -p /home/edgar/mdx/syntaxes
```

- [ ] **Step 2: Download all grammar files**

Download each syntax file from its source. Use the raw file URLs from the repos that bat references as submodules, plus bat's own direct files and Sublime's official Packages for Bash.

```bash
cd /home/edgar/mdx/syntaxes

# TOML (MIT) — from jasonwilliams/sublime_toml_highlighting
curl -fLO "https://raw.githubusercontent.com/jasonwilliams/sublime_toml_highlighting/master/TOML.sublime-syntax"

# TypeScript + TSX (Apache-2.0) — direct files from bat's 02_Extra
curl -fLO "https://raw.githubusercontent.com/sharkdp/bat/master/assets/syntaxes/02_Extra/TypeScript.sublime-syntax"
curl -fL "https://raw.githubusercontent.com/sharkdp/bat/master/assets/syntaxes/02_Extra/TypsecriptReact.sublime-syntax" -o "TypeScriptReact.sublime-syntax"

# Dockerfile (MIT) — from asbjornenge/Docker.tmbundle
curl -fL "https://raw.githubusercontent.com/asbjornenge/Docker.tmbundle/master/Syntaxes/Dockerfile.sublime-syntax" -o "Dockerfile.sublime-syntax"

# Kotlin (Apache-2.0) — direct file from bat's 02_Extra
curl -fLO "https://raw.githubusercontent.com/sharkdp/bat/master/assets/syntaxes/02_Extra/Kotlin.sublime-syntax"

# Swift (MIT) — direct file from bat's 02_Extra
curl -fLO "https://raw.githubusercontent.com/sharkdp/bat/master/assets/syntaxes/02_Extra/Swift.sublime-syntax"

# Zig (MIT) — from ziglang/sublime-zig-language on Codeberg
curl -fL "https://codeberg.org/ziglang/sublime-zig-language/raw/branch/master/Zig.sublime-syntax" -o "Zig.sublime-syntax"

# Terraform + HCL (MIT) — from alexlouden/Terraform.tmLanguage
curl -fLO "https://raw.githubusercontent.com/alexlouden/Terraform.tmLanguage/master/Terraform.sublime-syntax"
curl -fLO "https://raw.githubusercontent.com/alexlouden/Terraform.tmLanguage/master/HCL.sublime-syntax"

# Vue (MIT) — from vuejs/vue-syntax-highlight (plist format)
curl -fL "https://raw.githubusercontent.com/vuejs/vue-syntax-highlight/master/vue.tmLanguage" -o "Vue.tmLanguage"

# Svelte (MIT) — from corneliusio/svelte-sublime
curl -fLO "https://raw.githubusercontent.com/corneliusio/svelte-sublime/master/Svelte.sublime-syntax"

# SCSS (MIT) — from braver/SublimeSass
curl -fL "https://raw.githubusercontent.com/braver/SublimeSass/master/Syntaxes/SCSS.sublime-syntax" -o "SCSS.sublime-syntax"

# Bash replacement (Permissive) — from sublimehq/Packages
# Bash.sublime-syntax depends on Shell-Unix-Generic.sublime-syntax
curl -fLO "https://raw.githubusercontent.com/sublimehq/Packages/master/ShellScript/Bash.sublime-syntax"
curl -fLO "https://raw.githubusercontent.com/sublimehq/Packages/master/ShellScript/Shell-Unix-Generic.sublime-syntax"
```

- [ ] **Step 3: Verify all files downloaded**

```bash
ls -la /home/edgar/mdx/syntaxes/
```

Expected: 15 files (TOML, TypeScript, TypeScriptReact, Dockerfile, Kotlin, Swift, Zig, Terraform, HCL, Vue.tmLanguage, Svelte, SCSS, Bash, Shell-Unix-Generic). If any download failed (curl shows error), investigate the URL and retry. The Zig URL on Codeberg may need adjustment — check `https://codeberg.org/ziglang/sublime-zig-language` for the actual file name if it fails.

- [ ] **Step 4: Create NOTICE.md**

Write `/home/edgar/mdx/syntaxes/NOTICE.md`:

```markdown
# Vendored Syntax Grammars

This directory contains syntax grammar files vendored from third-party sources
for compile-time embedding into the mdx binary. Each file's provenance and
license is documented below.

Vendored: 2026-04-16

## Update Instructions

1. Check each source repo for updates
2. Download updated `.sublime-syntax` / `.tmLanguage` files
3. Update the commit hash and date below
4. Run `cargo build` to recompile the syntax packdump
5. Run `cargo test` to verify all language tokens still resolve

---

## TOML.sublime-syntax
- Source: https://github.com/jasonwilliams/sublime_toml_highlighting
- License: MIT

## TypeScript.sublime-syntax
- Source: https://github.com/sharkdp/bat (assets/syntaxes/02_Extra)
- Upstream: https://github.com/Microsoft/TypeScript-Sublime-Plugin
- License: Apache-2.0 (see LICENSE-APACHE)

## TypeScriptReact.sublime-syntax
- Source: https://github.com/sharkdp/bat (assets/syntaxes/02_Extra)
- Upstream: https://github.com/Microsoft/TypeScript-Sublime-Plugin
- License: Apache-2.0 (see LICENSE-APACHE)

## Dockerfile.sublime-syntax
- Source: https://github.com/asbjornenge/Docker.tmbundle
- License: MIT

## Kotlin.sublime-syntax
- Source: https://github.com/sharkdp/bat (assets/syntaxes/02_Extra)
- Upstream: https://github.com/vkostyukov/kotlin-sublime-package
- License: Apache-2.0 (see LICENSE-APACHE)

## Swift.sublime-syntax
- Source: https://github.com/quiqueg/Swift-Sublime-Package
- License: MIT

## Zig.sublime-syntax
- Source: https://codeberg.org/ziglang/sublime-zig-language
- License: MIT

## Terraform.sublime-syntax
- Source: https://github.com/alexlouden/Terraform.tmLanguage
- License: MIT

## HCL.sublime-syntax
- Source: https://github.com/alexlouden/Terraform.tmLanguage
- License: MIT

## Vue.tmLanguage
- Source: https://github.com/vuejs/vue-syntax-highlight
- License: MIT

## Svelte.sublime-syntax
- Source: https://github.com/corneliusio/svelte-sublime
- License: MIT

## SCSS.sublime-syntax
- Source: https://github.com/braver/SublimeSass
- License: MIT

## Bash.sublime-syntax
- Source: https://github.com/sublimehq/Packages (ShellScript/)
- License: Permissive ("Permission to copy, use, modify, sell and distribute is granted")
- Note: Replaces syntect's weak built-in Bash grammar

## Shell-Unix-Generic.sublime-syntax
- Source: https://github.com/sublimehq/Packages (ShellScript/)
- License: Permissive ("Permission to copy, use, modify, sell and distribute is granted")
- Note: Required dependency of Bash.sublime-syntax
```

- [ ] **Step 5: Create LICENSE-APACHE**

Download the Apache 2.0 license text:

```bash
curl -fL "https://www.apache.org/licenses/LICENSE-2.0.txt" -o /home/edgar/mdx/syntaxes/LICENSE-APACHE
```

- [ ] **Step 6: Commit**

```bash
cd /home/edgar/mdx
git add syntaxes/
git commit -m "chore: vendor syntax grammars from bat/Sublime ecosystem"
```

---

### Task 2: Create build.rs and update Cargo.toml

**Files:**
- Create: `build.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Update Cargo.toml**

Replace the existing syntect dependency line and add build-dependencies. The main dependency no longer needs `default-syntaxes` (the packdump replaces it) but still needs `default-themes`, `dump-load`, and `regex-fancy`. The build dependency needs `default-syntaxes`, `yaml-load`, `plist-load`, `dump-create`, and `regex-fancy`.

In `/home/edgar/mdx/Cargo.toml`, replace:

```toml
syntect = { version = "5", default-features = false, features = ["default-syntaxes", "default-themes", "regex-fancy"] }
```

with:

```toml
syntect = { version = "5", default-features = false, features = ["default-themes", "dump-load", "regex-fancy"] }

[build-dependencies]
syntect = { version = "5", default-features = false, features = ["default-syntaxes", "yaml-load", "plist-load", "dump-create", "regex-fancy"] }
```

- [ ] **Step 2: Create build.rs**

Write `/home/edgar/mdx/build.rs`:

```rust
use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let packdump_path = Path::new(&out_dir).join("syntaxes.packdump");

    // Start with syntect's bundled defaults
    let mut builder = syntect::parsing::SyntaxSet::load_defaults_newlines().into_builder();

    // Add our custom grammars on top (later additions override defaults for same-name syntaxes)
    builder
        .add_from_folder("syntaxes", true)
        .expect("Failed to load custom syntaxes from syntaxes/ directory");

    let syntax_set = builder.build();

    syntect::dumps::dump_to_uncompressed_file(&syntax_set, &packdump_path)
        .expect("Failed to write syntax packdump");

    // Rebuild if any syntax file changes
    println!("cargo:rerun-if-changed=syntaxes");
}
```

- [ ] **Step 3: Verify it builds**

Run: `cargo build 2>&1`

Expected: successful build. The packdump is generated in `target/debug/build/mdx-*/out/syntaxes.packdump`.

If the build fails with errors about missing syntax references (e.g., a syntax file referencing another syntax that doesn't exist), download the missing dependency file into `syntaxes/` and retry. Common issues:
- `Bash.sublime-syntax` referencing `Shell-Unix-Generic` (we already included it)
- `Dockerfile.sublime-syntax` referencing Bash syntax (available from defaults + our replacement)
- Vue/Svelte referencing HTML/CSS/JS (available from defaults)

- [ ] **Step 4: Commit**

```bash
cd /home/edgar/mdx
git add build.rs Cargo.toml Cargo.lock
git commit -m "feat: add build.rs to compile bundled syntax grammars"
```

---

### Task 3: Update highlight.rs to use packdump

**Files:**
- Modify: `src/highlight.rs:1-31`

- [ ] **Step 1: Write failing test for new language support**

Add to the test module in `src/highlight.rs`:

```rust
#[test]
fn test_bundled_syntax_toml() {
    let h = Highlighter::new(None).unwrap();
    let code = "[package]\nname = \"test\"\n";
    let result = h.highlight_code(code, Some("toml"));
    assert!(result.is_some(), "TOML should be a recognized language");
    let lines = result.unwrap();
    let has_color = lines
        .iter()
        .any(|line| line.iter().any(|span| span.style.fg.is_some()));
    assert!(has_color, "TOML code should have colored spans");
}

#[test]
fn test_bundled_syntax_tokens_resolve() {
    let h = Highlighter::new(None).unwrap();
    let tokens = [
        "toml", "ts", "tsx", "dockerfile", "kt", "swift",
        "zig", "hcl", "tf", "scss", "vue", "svelte",
    ];
    for token in &tokens {
        let result = h.highlight_code("x", Some(token));
        assert!(
            result.is_some(),
            "Token '{}' should resolve to a syntax",
            token
        );
    }
}

#[test]
fn test_bash_highlighting_has_multiple_colors() {
    let h = Highlighter::new(None).unwrap();
    let code = "#!/bin/bash\necho \"hello $USER\"\nif [[ -f foo ]]; then\n  cat foo\nfi\n";
    let result = h.highlight_code(code, Some("bash"));
    assert!(result.is_some(), "Bash should be recognized");
    let lines = result.unwrap();
    // Collect all distinct (r,g,b) colors across all spans
    let mut colors = std::collections::HashSet::new();
    for line in &lines {
        for span in line {
            if let Some(Color::Rgb(r, g, b)) = span.style.fg {
                colors.insert((r, g, b));
            }
        }
    }
    assert!(
        colors.len() >= 3,
        "Bash should produce at least 3 distinct colors (got {}), proving the grammar tokenizes properly",
        colors.len()
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_bundled_syntax 2>&1 && cargo test test_bash_highlighting 2>&1`
Expected: FAIL — `find_syntax_by_token("toml")` returns None because highlight.rs still loads from `SyntaxSet::load_defaults_newlines()` which doesn't include TOML.

- [ ] **Step 3: Update highlight.rs to load from packdump**

In `/home/edgar/mdx/src/highlight.rs`, replace:

```rust
use crate::render::{Color, SpanStyle, StyledSpan};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
```

with:

```rust
use crate::render::{Color, SpanStyle, StyledSpan};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

/// Combined syntax set compiled at build time by build.rs.
/// Includes syntect defaults + custom grammars from syntaxes/ directory.
static SYNTAX_SET_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/syntaxes.packdump"));
```

Replace the `Highlighter::new` function body. Change:

```rust
        Ok(Highlighter {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set,
            theme_name,
        })
```

to:

```rust
        let syntax_set: SyntaxSet = syntect::dumps::from_uncompressed_data(SYNTAX_SET_DATA)
            .expect("Failed to load embedded syntax packdump");
        Ok(Highlighter {
            syntax_set,
            theme_set,
            theme_name,
        })
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test test_bundled_syntax 2>&1 && cargo test test_bash_highlighting 2>&1`
Expected: all 3 new tests PASS.

If `test_bundled_syntax_tokens_resolve` fails for specific tokens, it means that syntax file uses different extension tokens than expected. Check the `.sublime-syntax` file's `file_extensions` field to find the correct token, and either update the test or add a note.

- [ ] **Step 5: Run full test suite**

Run: `cargo test 2>&1`
Expected: all tests pass. Existing tests (Rust highlighting, unknown language fallback, etc.) should be unaffected since the packdump includes all defaults plus extras.

- [ ] **Step 6: Commit**

```bash
cd /home/edgar/mdx
git add src/highlight.rs
git commit -m "feat: load syntax grammars from embedded packdump"
```

---

### Task 4: Integration test and cleanup

**Files:**
- Modify: `tests/integration.rs`

- [ ] **Step 1: Write integration test for TOML highlighting**

Add to `tests/integration.rs`:

```rust
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
    assert!(
        stdout.contains("package"),
        "Should contain TOML content"
    );
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test integration 2>&1`
Expected: all integration tests pass.

- [ ] **Step 3: Run full test suite + clippy**

Run: `cargo test 2>&1 && cargo clippy 2>&1`
Expected: all tests pass, no clippy warnings.

- [ ] **Step 4: Commit**

```bash
cd /home/edgar/mdx
git add tests/integration.rs
git commit -m "test: add TOML highlighting integration test"
```
