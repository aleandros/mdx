use crate::render::Color;

#[allow(dead_code)]
pub fn parse_color(input: &str) -> Option<Color> {
    let input = input.trim();
    if let Some(hex) = input.strip_prefix('#') {
        parse_hex(hex)
    } else {
        parse_named(input)
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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
}
