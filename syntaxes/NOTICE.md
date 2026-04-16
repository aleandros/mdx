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

## TypeScriptReact.tmLanguage
- Source: https://github.com/Microsoft/TypeScript-Sublime-Plugin
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
- Source: https://codeberg.org/ziglang/sublime-zig-language (Syntaxes/)
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
