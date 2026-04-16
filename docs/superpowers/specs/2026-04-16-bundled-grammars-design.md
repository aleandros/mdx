# Bundle Additional Syntax Grammars

## Overview

Bundle additional `.sublime-syntax` grammar files into the mdx binary at compile time, sourced from bat's curated collection. This replaces syntect's limited default grammar set with a richer one covering TOML, TypeScript, Dockerfile, and more, while also replacing the weak default Bash grammar.

## Languages to add

| Language | Token(s) | Source repo | License |
|----------|----------|-------------|---------|
| TOML | `toml` | jasonwilliams/sublime_toml_highlighting | MIT |
| TypeScript | `typescript`, `ts` | Microsoft/TypeScript-Sublime-Plugin | Apache-2.0 |
| TSX | `tsx` | Microsoft/TypeScript-Sublime-Plugin | Apache-2.0 |
| Dockerfile | `dockerfile`, `docker` | asbjornenge/Docker.tmbundle | MIT |
| Kotlin | `kotlin`, `kt` | vkostyukov/kotlin-sublime-package | Apache-2.0 |
| Swift | `swift` | quiqueg/Swift-Sublime-Package | MIT |
| Zig | `zig` | ziglang/sublime-zig-language (Codeberg) | MIT |
| Terraform/HCL | `terraform`, `hcl`, `tf` | alexlouden/Terraform.tmLanguage | MIT |
| Vue | `vue` | vuejs/vue-syntax-highlight | MIT |
| Svelte | `svelte` | corneliusio/svelte-sublime | MIT |
| SCSS | `scss` | braver/SublimeSass | MIT |
| Bash (replacement) | `bash`, `sh` | sublimehq/Packages | Permissive |

The Bash grammar from Sublime's official Packages replaces syntect's bundled default, which produces poor tokenization (renders most content as a single comment color).

## Build pipeline

```
syntaxes/*.sublime-syntax  +  syntect defaults
        |                           |
      build.rs (SyntaxSetBuilder)
        |
   OUT_DIR/syntaxes.packdump
        |
   include_bytes!() in highlight.rs
```

### build.rs

1. Load syntect defaults: `SyntaxSet::load_defaults_newlines()`
2. Convert to builder: `.into_builder()`
3. Add custom grammars: `.add_from_folder("syntaxes/", true)`
4. Build combined set: `.build()`
5. Serialize: `syntect::dumps::dump_to_uncompressed_file()` to `OUT_DIR/syntaxes.packdump`

Custom grammars that share a name with a default (e.g., Bash) override the default.

### highlight.rs

Replace `SyntaxSet::load_defaults_newlines()` with:

```rust
syntect::dumps::from_uncompressed_data(
    include_bytes!(concat!(env!("OUT_DIR"), "/syntaxes.packdump"))
).unwrap()
```

## Changes to existing files

### `Cargo.toml`

Add features to syntect:

```toml
syntect = { version = "5", default-features = false, features = [
    "default-syntaxes", "default-themes", "regex-fancy",
    "yaml-load", "dump-create", "dump-load"
] }
```

Add build dependency for syntect (build.rs needs it):

```toml
[build-dependencies]
syntect = { version = "5", default-features = false, features = [
    "default-syntaxes", "regex-fancy", "yaml-load", "dump-create"
] }
```

### `build.rs` (new)

Compiles combined syntax set at build time.

### `src/highlight.rs`

Replace `SyntaxSet::load_defaults_newlines()` with `from_uncompressed_data(include_bytes!(...))`.

Remove the `default-syntaxes` feature from the main `[dependencies]` syntect entry since the packdump includes everything. Keep it in `[build-dependencies]`.

## New files

### `syntaxes/`

Contains vendored `.sublime-syntax` files. One file per syntax, named clearly (e.g., `TOML.sublime-syntax`, `TypeScript.sublime-syntax`).

### `syntaxes/NOTICE.md`

Documents provenance and license compliance for each vendored file:

- Source repository URL and commit hash
- License type for each file
- Date vendored
- Instructions for updating (re-download from bat submodules, verify licenses)

Example entry:

```
## TOML.sublime-syntax
- Source: https://github.com/jasonwilliams/sublime_toml_highlighting
- Commit: <hash>
- License: MIT
- Vendored: 2026-04-16
```

## License compliance

All vendored grammars use permissive licenses (MIT or Apache-2.0) compatible with mdx's MIT license.

Requirements:
- **MIT-licensed files:** Include original copyright notice. The `syntaxes/NOTICE.md` file satisfies this by documenting each source and its license.
- **Apache-2.0 files (TypeScript, Kotlin):** Apache-2.0 requires: (a) a copy of the license, (b) notice of any modifications, (c) preservation of NOTICE files if present. Include a copy of the Apache-2.0 license text in `syntaxes/LICENSE-APACHE` and note any modifications in `NOTICE.md`.
- **Sublime Packages (Bash):** The permissive license ("permission to copy, use, modify, sell and distribute is granted") has no attribution requirement, but we document provenance in `NOTICE.md` for good practice.
- **mdx LICENSE file:** The project's MIT license file should include a note referencing `syntaxes/NOTICE.md` for third-party attributions.

## Fallback behavior

Unchanged — unrecognized languages still fall back to dim monochrome.

## Binary size impact

The combined packdump adds approximately 500KB-1MB to the binary. Acceptable for a CLI tool.

## Testing

- **Unit test:** Verify `find_syntax_by_token` returns `Some` for each new language token: `toml`, `ts`, `tsx`, `dockerfile`, `kotlin`, `kt`, `swift`, `zig`, `hcl`, `tf`, `scss`, `vue`, `svelte`
- **Unit test:** Verify `bash` highlighting produces more than one distinct color (proves better grammar vs the old single-color output)
- **Integration test:** Render a TOML code block and verify ANSI color codes appear in output
- **Build test:** Verify `cargo build` succeeds (build.rs compiles the packdump)
