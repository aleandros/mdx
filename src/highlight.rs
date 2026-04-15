use crate::render::Color;

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
}
