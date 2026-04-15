use crate::render::{Color, SpanStyle, StyledSpan};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    pub(crate) theme_name: Option<String>,
}

impl Highlighter {
    pub fn new(theme_name: Option<String>) -> Self {
        Highlighter {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            theme_name,
        }
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

        let use_rgb = self.theme_name.is_some();
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
                    } else if use_rgb {
                        Some(Color::Rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        ))
                    } else {
                        Some(rgb_to_ansi_color(
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

/// Reference RGB values for the 16 standard ANSI colors.
const ANSI_COLORS: &[(Color, u8, u8, u8)] = &[
    (Color::Red, 205, 0, 0),
    (Color::Green, 0, 205, 0),
    (Color::Yellow, 205, 205, 0),
    (Color::Blue, 0, 0, 238),
    (Color::Magenta, 205, 0, 205),
    (Color::Cyan, 0, 205, 205),
    (Color::White, 229, 229, 229),
    (Color::BrightYellow, 255, 255, 85),
    (Color::BrightCyan, 85, 255, 255),
    (Color::BrightMagenta, 255, 85, 255),
    (Color::DarkGray, 127, 127, 127),
];

/// Maps an RGB color to the nearest ANSI Color using Euclidean distance in RGB space.
fn rgb_to_ansi_color(r: u8, g: u8, b: u8) -> Color {
    let (r, g, b) = (r as i32, g as i32, b as i32);
    ANSI_COLORS
        .iter()
        .map(|(color, cr, cg, cb)| {
            let dr = r - *cr as i32;
            let dg = g - *cg as i32;
            let db = b - *cb as i32;
            let dist = dr * dr + dg * dg + db * db;
            (dist, color)
        })
        .min_by_key(|(dist, _)| *dist)
        .unwrap()
        .1
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_ansi_pure_red() {
        let color = rgb_to_ansi_color(255, 0, 0);
        assert_eq!(color, Color::Red);
    }

    #[test]
    fn test_rgb_to_ansi_pure_green() {
        let color = rgb_to_ansi_color(0, 255, 0);
        assert_eq!(color, Color::Green);
    }

    #[test]
    fn test_rgb_to_ansi_pure_blue() {
        let color = rgb_to_ansi_color(0, 0, 255);
        assert_eq!(color, Color::Blue);
    }

    #[test]
    fn test_rgb_to_ansi_white() {
        let color = rgb_to_ansi_color(255, 255, 255);
        assert_eq!(color, Color::White);
    }

    #[test]
    fn test_rgb_to_ansi_dark_gray() {
        let color = rgb_to_ansi_color(100, 100, 100);
        assert_eq!(color, Color::DarkGray);
    }

    #[test]
    fn test_highlighter_new_default() {
        let h = Highlighter::new(None);
        // Should not panic, should construct successfully
        assert!(h.theme_name.is_none());
    }

    #[test]
    fn test_highlight_rust_code() {
        let h = Highlighter::new(None);
        let code = "fn main() {\n    println!(\"hello\");\n}\n";
        let result = h.highlight_code(code, Some("rust"));
        assert!(result.is_some(), "Rust should be a recognized language");
        let lines = result.unwrap();
        assert_eq!(lines.len(), 3, "Should have 3 lines of code");
        // At least some spans should have color (not all plain)
        let has_color = lines.iter().any(|line| {
            line.iter().any(|span| span.style.fg.is_some())
        });
        assert!(has_color, "Highlighted code should have colored spans");
    }

    #[test]
    fn test_highlight_unknown_language_returns_none() {
        let h = Highlighter::new(None);
        let result = h.highlight_code("some text", Some("not_a_real_language_xyz"));
        assert!(result.is_none(), "Unknown language should return None");
    }

    #[test]
    fn test_highlight_no_language_returns_none() {
        let h = Highlighter::new(None);
        let result = h.highlight_code("some text", None);
        assert!(result.is_none(), "No language should return None");
    }

    #[test]
    fn test_highlight_with_named_theme() {
        let h = Highlighter::new(Some("base16-ocean.dark".to_string()));
        let code = "fn main() {}\n";
        let result = h.highlight_code(code, Some("rust"));
        assert!(result.is_some());
        let lines = result.unwrap();
        // With a named theme, should produce Rgb colors
        let has_rgb = lines.iter().any(|line| {
            line.iter().any(|span| matches!(span.style.fg, Some(Color::Rgb(_, _, _))))
        });
        assert!(has_rgb, "Named theme should produce Rgb colors");
    }

    #[test]
    fn test_highlight_default_theme_uses_ansi_colors() {
        let h = Highlighter::new(None);
        let code = "fn main() {}\n";
        let result = h.highlight_code(code, Some("rust"));
        assert!(result.is_some());
        let lines = result.unwrap();
        // Default (ANSI) mode should NOT produce Rgb colors
        let has_rgb = lines.iter().any(|line| {
            line.iter().any(|span| matches!(span.style.fg, Some(Color::Rgb(_, _, _))))
        });
        assert!(!has_rgb, "Default ANSI mode should not produce Rgb colors");
    }
}
