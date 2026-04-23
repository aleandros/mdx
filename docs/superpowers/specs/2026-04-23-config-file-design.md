# Config File Support Design

**Date:** 2026-04-23
**Status:** Approved

## Overview

Add user-level and project-level TOML configuration files to `mdx`, allowing users to set default values for themes and flags without passing them on every invocation.

## Config File Format

Flat TOML. All keys optional. Key names match CLI long flags with hyphens converted to underscores.

```toml
# Syntax highlighting theme for code blocks
theme = "base16-ocean.dark"

# UI theme for headers, text, borders
ui_theme = "nord"

# Force pager mode
pager = true

# Terminal width override
width = 100

# Mermaid diagram rendering
no_mermaid_rendering = false
split_mermaid_rendering = true
```

Missing keys mean "defer to next layer." Invalid values (unknown theme name, `width = 0`) produce error messages referencing the config file path and key.

## File Locations

- **User config:** `$XDG_CONFIG_HOME/mdx/config.toml` (falls back to `~/.config/mdx/config.toml`)
- **Project config:** `.mdx.toml` in the directory containing the closest `.git` (walking upward from CWD)
- **Explicit:** `--config <path>` flag

## Precedence

Last wins:

1. Built-in defaults (`theme = "base16-ocean.dark"`, `ui_theme = "clay"`, others false/unset)
2. User config (`~/.config/mdx/config.toml`)
3. Project config (`.mdx.toml` next to `.git`)
4. `--config <path>` (if passed, replaces steps 2 and 3 — only this file is loaded)
5. CLI flags (always win)

Auto-discovered configs (user, project) are silently skipped if missing. `--config <path>` errors if the file doesn't exist.

## Config Module (`src/config.rs`)

### Struct

```rust
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
```

All fields `Option<T>` — `None` means "defer to next layer."

### Key Functions

- `Config::load(cli_config_path: Option<&Path>) -> Result<Config>` — orchestrates discovery and merging. If `cli_config_path` is `Some`, loads only that file (errors if missing). Otherwise loads user config then project config, merges them.
- `Config::from_file(path: &Path) -> Result<Config>` — parses a single TOML file.
- `Config::merge(self, other: Config) -> Config` — `other`'s `Some` values override `self`'s.
- `Config::generate_default() -> String` — returns the commented-out default config string for `init`.

### Integration with `main.rs`

After CLI parsing, before resolving individual options:

```rust
let config = Config::load(args.config.as_deref())?;
let theme = args.theme.or(config.theme).unwrap_or_else(|| "base16-ocean.dark".into());
let ui_theme = args.ui_theme.or(config.ui_theme).unwrap_or_else(|| "clay".into());
// ... same pattern for other fields
```

Clap parsing stays untouched. Config is a parallel source feeding the same resolution logic.

## CLI Changes

### New `--config` flag

Added to both `Args` and `EmbedArgs`:

```rust
#[arg(long, value_name = "PATH")]
pub config: Option<PathBuf>,
```

### New `init` subcommand

```rust
enum Commands {
    Update,
    Embed(EmbedArgs),
    PreviewThemes,
    Init,
}
```

Behavior:
- Writes `~/.config/mdx/config.toml` (XDG-resolved) with all keys commented out showing defaults
- Creates `~/.config/mdx/` directory if it doesn't exist
- Refuses if `config.toml` already exists — prints message pointing to existing file, exits non-zero
- Prints the path written on success

Generated file contents:

```toml
# Syntax highlighting theme for code blocks
# theme = "base16-ocean.dark"

# UI theme for headers, text, borders
# ui_theme = "clay"

# Force pager mode
# pager = false

# Terminal width override (omit to use terminal width)
# width = 100

# Mermaid diagram rendering
# no_mermaid_rendering = false
# split_mermaid_rendering = false
```

## Error Handling

**Parse errors** — file path + `toml` crate's line/column error:
```
Error: failed to parse config file /home/user/.config/mdx/config.toml
  expected a value, found newline at line 3 column 8
```

**Unknown keys** — caught by `#[serde(deny_unknown_fields)]`:
```
Error: unknown field `ui-theme` in /home/user/.config/mdx/config.toml
  did you mean `ui_theme`?
```

**Invalid values** — validated after merge, same as CLI flags. Error references source file:
```
Error: unknown UI theme "norf" (from /home/user/project/.mdx.toml)
  available themes: clay, hearth, frost, nord, ...
```

**`--config` missing file:**
```
Error: config file not found: /path/to/custom.toml
```

**`init` when file exists:**
```
Config file already exists: /home/user/.config/mdx/config.toml
```

## New Dependencies

- `serde` with `derive` feature
- `toml`

## Testing

### Unit tests (`src/config.rs`)

- `Config::merge` — `Some` overrides `None`, two `Some` picks second, two `None` stays `None`
- `Config::from_file` — valid TOML parses, invalid TOML errors, unknown keys rejected
- `Config::generate_default` — output is valid TOML (parse-roundtrips without error)

### Integration tests (`tests/integration.rs`)

- Config discovery — temp dirs with `.git` + `.mdx.toml`, verify project config picked up
- Precedence — set `ui_theme` in user config, override in project config, override with `--ui-theme` flag
- `--config` flag — point to temp file (works), point to nonexistent file (errors)
- `init` subcommand — clean temp `$HOME` (file created), run again (refuses)
- Embed mode — verify `--config` works with `mdx embed`
