use super::{MermaidEdgeStyle, NodeStyle};
use crate::render::Color;
use crate::theme::Theme;

pub fn parse_color(input: &str) -> Option<Color> {
    let input = input.trim();
    if let Some(hex) = input.strip_prefix('#') {
        parse_hex(hex)
    } else {
        parse_named(input)
    }
}

fn parse_hex(hex: &str) -> Option<Color> {
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

fn parse_named(name: &str) -> Option<Color> {
    match name.to_lowercase().as_str() {
        "red" => Some(Color::Rgb(255, 0, 0)),
        "green" => Some(Color::Rgb(0, 128, 0)),
        "blue" => Some(Color::Rgb(0, 0, 255)),
        "cyan" => Some(Color::Rgb(0, 255, 255)),
        "magenta" => Some(Color::Rgb(255, 0, 255)),
        "yellow" => Some(Color::Rgb(255, 255, 0)),
        "white" => Some(Color::Rgb(255, 255, 255)),
        "black" => Some(Color::Rgb(0, 0, 0)),
        "orange" => Some(Color::Rgb(255, 165, 0)),
        "purple" => Some(Color::Rgb(128, 0, 128)),
        "pink" => Some(Color::Rgb(255, 192, 203)),
        "gray" | "grey" => Some(Color::Rgb(128, 128, 128)),
        _ => None,
    }
}

fn color_to_rgb(color: &Color) -> (f64, f64, f64) {
    match color {
        Color::Rgb(r, g, b) => (*r as f64, *g as f64, *b as f64),
        Color::Red => (255.0, 0.0, 0.0),
        Color::Green => (0.0, 128.0, 0.0),
        Color::Blue => (0.0, 0.0, 255.0),
        Color::Yellow => (255.0, 255.0, 0.0),
        Color::Magenta => (255.0, 0.0, 255.0),
        Color::Cyan => (0.0, 255.0, 255.0),
        Color::White => (255.0, 255.0, 255.0),
        Color::BrightYellow => (255.0, 255.0, 128.0),
        Color::BrightCyan => (128.0, 255.0, 255.0),
        Color::BrightMagenta => (255.0, 128.0, 255.0),
        Color::DarkGray => (128.0, 128.0, 128.0),
    }
}

pub fn resolve_color(color: &Color, theme: &Theme) -> Color {
    let (r, g, b) = color_to_rgb(color);
    let mut best_color = color.clone();
    let mut best_dist = f64::MAX;
    for candidate in theme.all_colors() {
        let (cr, cg, cb) = color_to_rgb(candidate);
        let dist = (r - cr).powi(2) + (g - cg).powi(2) + (b - cb).powi(2);
        if dist < best_dist {
            best_dist = dist;
            best_color = candidate.clone();
        }
    }
    best_color
}

pub(crate) fn parse_node_style_props(props: &str) -> NodeStyle {
    let mut style = NodeStyle::default();
    for prop in props.split(',') {
        let prop = prop.trim();
        if let Some((key, value)) = prop.split_once(':') {
            match key.trim() {
                "fill" => style.fill = parse_color(value.trim()),
                "stroke" => style.stroke = parse_color(value.trim()),
                "color" => style.color = parse_color(value.trim()),
                _ => {}
            }
        }
    }
    style
}

pub(crate) fn parse_edge_style_props(props: &str) -> MermaidEdgeStyle {
    let mut style = MermaidEdgeStyle::default();
    for prop in props.split(',') {
        let prop = prop.trim();
        if let Some((key, value)) = prop.split_once(':') {
            match key.trim() {
                "stroke" => style.stroke = parse_color(value.trim()),
                "color" => style.label_color = parse_color(value.trim()),
                _ => {}
            }
        }
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Color;

    #[test]
    fn test_parse_hex_6_digit() {
        assert_eq!(parse_color("#ff9900"), Some(Color::Rgb(255, 153, 0)));
    }

    #[test]
    fn test_parse_hex_3_digit() {
        assert_eq!(parse_color("#f90"), Some(Color::Rgb(255, 153, 0)));
    }

    #[test]
    fn test_parse_named_red() {
        assert_eq!(parse_color("red"), Some(Color::Rgb(255, 0, 0)));
    }

    #[test]
    fn test_parse_named_blue() {
        assert_eq!(parse_color("blue"), Some(Color::Rgb(0, 0, 255)));
    }

    #[test]
    fn test_parse_invalid_returns_none() {
        assert_eq!(parse_color("not-a-color"), None);
        assert_eq!(parse_color("#xyz"), None);
        assert_eq!(parse_color(""), None);
    }

    #[test]
    fn test_parse_named_gray_and_grey() {
        assert_eq!(parse_color("gray"), parse_color("grey"));
    }

    #[test]
    fn test_resolve_pure_red_to_nearest_clay() {
        let theme = Theme::default_theme();
        let resolved = resolve_color(&Color::Rgb(255, 0, 0), theme);
        assert_eq!(resolved, Color::Rgb(180, 90, 60));
    }

    #[test]
    fn test_resolve_exact_match_returns_same() {
        let theme = Theme::default_theme();
        let olive = Color::Rgb(120, 160, 80);
        let resolved = resolve_color(&olive, theme);
        assert_eq!(resolved, olive);
    }

    #[test]
    fn test_resolve_white_to_nearest() {
        let theme = Theme::default_theme();
        let resolved = resolve_color(&Color::Rgb(255, 255, 255), theme);
        assert!(matches!(resolved, Color::Rgb(_, _, _)));
    }
}
