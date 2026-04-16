use crate::render::{Color, SpanStyle, StyledSpan};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

/// Combined syntax set compiled at build time by build.rs.
/// Includes syntect defaults + custom grammars from syntaxes/ directory.
static SYNTAX_SET_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/syntaxes.packdump"));

#[derive(Debug)]
pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    theme_name: Option<String>,
}

impl Highlighter {
    pub fn new(theme_name: Option<String>) -> Result<Self, String> {
        let theme_set = ThemeSet::load_defaults();
        if let Some(ref name) = theme_name
            && !theme_set.themes.contains_key(name.as_str())
        {
            let mut available: Vec<&str> = theme_set.themes.keys().map(|s| s.as_str()).collect();
            available.sort();
            return Err(format!(
                "Unknown theme '{}'. Available themes:\n  {}",
                name,
                available.join("\n  ")
            ));
        }
        Ok(Highlighter {
            syntax_set: syntect::dumps::from_uncompressed_data(SYNTAX_SET_DATA)
                .expect("Failed to load embedded syntax packdump"),
            theme_set,
            theme_name,
        })
    }

    pub fn available_themes(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.theme_set.themes.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Highlight code, returning styled spans per line.
    /// Returns None if the language is unrecognized or absent.
    pub fn highlight_code(
        &self,
        code: &str,
        language: Option<&str>,
    ) -> Option<Vec<Vec<StyledSpan>>> {
        let lang = language?;
        let syntax = self.syntax_set.find_syntax_by_token(lang)?;

        let theme_key = self.theme_name.as_deref().unwrap_or("base16-ocean.dark");
        let theme = self.theme_set.themes.get(theme_key)?;

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut result = Vec::new();

        for line in code.lines() {
            let ranges = highlighter.highlight_line(line, &self.syntax_set).ok()?;
            let spans: Vec<StyledSpan> = ranges
                .into_iter()
                .map(|(style, text)| {
                    let fg = if style.foreground.a == 0 {
                        None
                    } else {
                        Some(Color::Rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        ))
                    };
                    StyledSpan {
                        text: text.to_string(),
                        style: SpanStyle {
                            fg,
                            ..Default::default()
                        },
                    }
                })
                .collect();
            result.push(spans);
        }

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlighter_new_default() {
        let h = Highlighter::new(None).unwrap();
        // Should not panic, should construct successfully
        assert!(h.theme_name.is_none());
    }

    #[test]
    fn test_highlighter_new_invalid_theme() {
        let result = Highlighter::new(Some("nonexistent_theme_xyz".to_string()));
        assert!(result.is_err(), "Invalid theme should return Err");
        let err = result.unwrap_err();
        assert!(
            err.contains("Unknown theme"),
            "Error should mention unknown theme: {}",
            err
        );
    }

    #[test]
    fn test_highlight_rust_code() {
        let h = Highlighter::new(None).unwrap();
        let code = "fn main() {\n    println!(\"hello\");\n}\n";
        let result = h.highlight_code(code, Some("rust"));
        assert!(result.is_some(), "Rust should be a recognized language");
        let lines = result.unwrap();
        assert_eq!(lines.len(), 3, "Should have 3 lines of code");
        // At least some spans should have color (not all plain)
        let has_color = lines
            .iter()
            .any(|line| line.iter().any(|span| span.style.fg.is_some()));
        assert!(has_color, "Highlighted code should have colored spans");
    }

    #[test]
    fn test_highlight_unknown_language_returns_none() {
        let h = Highlighter::new(None).unwrap();
        let result = h.highlight_code("some text", Some("not_a_real_language_xyz"));
        assert!(result.is_none(), "Unknown language should return None");
    }

    #[test]
    fn test_highlight_no_language_returns_none() {
        let h = Highlighter::new(None).unwrap();
        let result = h.highlight_code("some text", None);
        assert!(result.is_none(), "No language should return None");
    }

    #[test]
    fn test_highlight_with_named_theme() {
        let h = Highlighter::new(Some("base16-ocean.dark".to_string())).unwrap();
        let code = "fn main() {}\n";
        let result = h.highlight_code(code, Some("rust"));
        assert!(result.is_some());
        let lines = result.unwrap();
        // With a named theme, should produce Rgb colors
        let has_rgb = lines.iter().any(|line| {
            line.iter()
                .any(|span| matches!(span.style.fg, Some(Color::Rgb(_, _, _))))
        });
        assert!(has_rgb, "Named theme should produce Rgb colors");
    }

    #[test]
    fn test_highlight_default_theme_uses_rgb_colors() {
        let h = Highlighter::new(None).unwrap();
        let code = "fn main() {}\n";
        let result = h.highlight_code(code, Some("rust"));
        assert!(result.is_some());
        let lines = result.unwrap();
        // Default theme (base16-ocean.dark) should produce Rgb colors
        let has_rgb = lines.iter().any(|line| {
            line.iter()
                .any(|span| matches!(span.style.fg, Some(Color::Rgb(_, _, _))))
        });
        assert!(has_rgb, "Default theme should produce Rgb colors");
    }

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
        // Tokens for all bundled syntaxes that we vendored.
        // Note: tsx and vue are excluded because their grammars are in
        // .tmLanguage format, which syntect's add_from_folder doesn't load.
        let tokens = [
            "toml", "ts", "dockerfile", "kt", "swift",
            "zig", "tf",
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
}
