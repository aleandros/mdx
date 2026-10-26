use crate::render::Color;
use std::path::PathBuf;

/// On-disk representation of a user-defined UI theme, loaded from
/// `<themes_dir>/<name>.toml`. Colors are hex strings (`"#rrggbb"`).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct UserTheme {
    /// Six hex colors, H1 through H6.
    heading: [String; 6],
    body: String,
    bold: String,
    italic: String,
    link: String,
    inline_code: String,
    horizontal_rule: String,
    diagram_border: String,
    diagram_collapsed: String,
    diagram_node_fill: String,
    diagram_node_border: String,
    diagram_node_text: String,
    diagram_edge_stroke: String,
    diagram_edge_label: String,
}

/// Parses `#rrggbb` / `#RRGGBB` into a `Color::Rgb`.
fn parse_hex_color(field: &str, value: &str) -> Result<Color, String> {
    let hex = value.strip_prefix('#').ok_or_else(|| {
        format!("field '{field}' has value '{value}', expected a hex color like \"#bd93f9\"")
    })?;
    if hex.len() != 6 {
        return Err(format!(
            "field '{field}' has value '{value}', expected exactly 6 hex digits after '#'"
        ));
    }
    let byte = |slice: &str| {
        u8::from_str_radix(slice, 16)
            .map_err(|_| format!("field '{field}' has value '{value}', which is not valid hex"))
    };
    let r = byte(&hex[0..2])?;
    let g = byte(&hex[2..4])?;
    let b = byte(&hex[4..6])?;
    Ok(Color::Rgb(r, g, b))
}

impl UserTheme {
    /// Converts the string-keyed TOML representation into a runtime `Theme`.
    fn into_theme(self, name: String) -> Result<Theme, String> {
        let mut heading = [const { Color::Rgb(0, 0, 0) }; 6];
        for (i, hex) in self.heading.iter().enumerate() {
            heading[i] = parse_hex_color(&format!("heading[{i}]"), hex)?;
        }
        Ok(Theme {
            name: Box::leak(name.into_boxed_str()),
            heading,
            body: parse_hex_color("body", &self.body)?,
            bold: parse_hex_color("bold", &self.bold)?,
            italic: parse_hex_color("italic", &self.italic)?,
            link: parse_hex_color("link", &self.link)?,
            inline_code: parse_hex_color("inline_code", &self.inline_code)?,
            horizontal_rule: parse_hex_color("horizontal_rule", &self.horizontal_rule)?,
            diagram_border: parse_hex_color("diagram_border", &self.diagram_border)?,
            diagram_collapsed: parse_hex_color("diagram_collapsed", &self.diagram_collapsed)?,
            diagram_node_fill: parse_hex_color("diagram_node_fill", &self.diagram_node_fill)?,
            diagram_node_border: parse_hex_color("diagram_node_border", &self.diagram_node_border)?,
            diagram_node_text: parse_hex_color("diagram_node_text", &self.diagram_node_text)?,
            diagram_edge_stroke: parse_hex_color("diagram_edge_stroke", &self.diagram_edge_stroke)?,
            diagram_edge_label: parse_hex_color("diagram_edge_label", &self.diagram_edge_label)?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct Theme {
    pub(crate) name: &'static str,
    pub(crate) heading: [Color; 6],
    pub(crate) body: Color,
    pub(crate) bold: Color,
    pub(crate) italic: Color,
    pub(crate) link: Color,
    pub(crate) inline_code: Color,
    pub(crate) horizontal_rule: Color,
    pub(crate) diagram_border: Color,
    pub(crate) diagram_collapsed: Color,
    pub(crate) diagram_node_fill: Color,
    pub(crate) diagram_node_border: Color,
    pub(crate) diagram_node_text: Color,
    pub(crate) diagram_edge_stroke: Color,
    pub(crate) diagram_edge_label: Color,
}

impl Theme {
    pub(crate) fn by_name(name: &str) -> Option<&'static Theme> {
        match name {
            "clay" => Some(&CLAY),
            "hearth" => Some(&HEARTH),
            "frost" => Some(&FROST),
            "nord" => Some(&NORD),
            "glacier" => Some(&GLACIER),
            "steel" => Some(&STEEL),
            "solarized-dark" => Some(&SOLARIZED_DARK),
            "solarized-light" => Some(&SOLARIZED_LIGHT),
            "paper" => Some(&PAPER),
            "snow" => Some(&SNOW),
            "latte" => Some(&LATTE),
            _ => None,
        }
    }

    pub(crate) fn default_theme() -> &'static Theme {
        &CLAY
    }

    /// Built-in theme names compiled into the binary.
    pub(crate) fn builtin_names() -> &'static [&'static str] {
        &[
            "clay",
            "hearth",
            "frost",
            "nord",
            "glacier",
            "steel",
            "solarized-dark",
            "solarized-light",
            "paper",
            "snow",
            "latte",
        ]
    }

    /// Directory scanned for user-defined UI themes:
    /// `$XDG_CONFIG_HOME/mdx/themes/<name>.toml`, falling back to
    /// `~/.config/mdx/themes/<name>.toml`.
    pub(crate) fn user_themes_dir() -> Option<PathBuf> {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".config"))
            })?;
        Some(config_dir.join("mdx").join("themes"))
    }

    /// Names of user-defined themes found as `*.toml` files in
    /// `user_themes_dir()`. Does not validate their contents.
    pub(crate) fn custom_names() -> Vec<String> {
        let Some(dir) = Self::user_themes_dir() else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("toml"))
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .collect();
        names.sort();
        names
    }

    /// All theme names: built-ins followed by discovered custom themes.
    pub(crate) fn available_names() -> Vec<String> {
        let mut names: Vec<String> = Self::builtin_names()
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        names.extend(Self::custom_names());
        names
    }

    /// Loads a user-defined theme named `<user_themes_dir>/<name>.toml`.
    /// Returns `Ok(None)` if no such file exists, `Err` if it exists but
    /// fails to parse, and leaks the theme to `'static` on success (fine
    /// for a short-lived CLI process).
    fn load_custom(name: &str) -> Result<Option<&'static Theme>, String> {
        let Some(dir) = Self::user_themes_dir() else {
            return Ok(None);
        };
        let path = dir.join(format!("{name}.toml"));
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read theme file {}: {e}", path.display()))?;
        let user_theme: UserTheme = toml::from_str(&content)
            .map_err(|e| format!("failed to parse theme file {}: {e}", path.display()))?;
        let theme = user_theme.into_theme(name.to_string())?;
        Ok(Some(Box::leak(Box::new(theme))))
    }

    /// Resolves a theme by name: built-ins first, then
    /// `<user_themes_dir>/<name>.toml`. Returns a descriptive error
    /// (including available names) when nothing matches, or when a
    /// matching custom theme file fails to parse.
    pub(crate) fn resolve(name: &str) -> Result<&'static Theme, String> {
        if let Some(theme) = Self::by_name(name) {
            return Ok(theme);
        }
        if let Some(theme) = Self::load_custom(name)? {
            return Ok(theme);
        }
        Err(format!(
            "Unknown UI theme '{name}'. Available: {}",
            Self::available_names().join(", ")
        ))
    }

    pub(crate) fn all() -> &'static [&'static Theme] {
        static ALL: &[&Theme] = &[
            &CLAY,
            &HEARTH,
            &FROST,
            &NORD,
            &GLACIER,
            &STEEL,
            &SOLARIZED_DARK,
            &SOLARIZED_LIGHT,
            &PAPER,
            &SNOW,
            &LATTE,
        ];
        ALL
    }

    pub(crate) fn all_colors(&self) -> Vec<&Color> {
        let mut colors: Vec<&Color> = Vec::new();
        for c in &self.heading {
            colors.push(c);
        }
        colors.extend_from_slice(&[
            &self.body,
            &self.bold,
            &self.italic,
            &self.link,
            &self.inline_code,
            &self.horizontal_rule,
            &self.diagram_border,
            &self.diagram_collapsed,
            &self.diagram_node_fill,
            &self.diagram_node_border,
            &self.diagram_node_text,
            &self.diagram_edge_stroke,
            &self.diagram_edge_label,
        ]);
        colors
    }
}

static CLAY: Theme = Theme {
    name: "clay",
    heading: [
        Color::Rgb(210, 140, 40),  // H1: Dark Honey
        Color::Rgb(180, 90, 60),   // H2: Clay Red
        Color::Rgb(120, 160, 80),  // H3: Olive
        Color::Rgb(160, 110, 70),  // H4: Sienna
        Color::Rgb(130, 140, 110), // H5: Driftwood
        Color::Rgb(110, 115, 100), // H6: Slate Moss
    ],
    body: Color::Rgb(190, 180, 160),
    bold: Color::Rgb(190, 180, 160),
    italic: Color::Rgb(190, 180, 160),
    link: Color::Rgb(120, 150, 100),
    inline_code: Color::Rgb(160, 120, 60),
    horizontal_rule: Color::Rgb(90, 80, 60),
    diagram_border: Color::Rgb(160, 120, 60),
    diagram_collapsed: Color::Rgb(120, 150, 100),
    diagram_node_fill: Color::Rgb(160, 120, 60),
    diagram_node_border: Color::Rgb(180, 90, 60),
    diagram_node_text: Color::Rgb(190, 180, 160),
    diagram_edge_stroke: Color::Rgb(120, 160, 80),
    diagram_edge_label: Color::Rgb(130, 140, 110),
};

static HEARTH: Theme = Theme {
    name: "hearth",
    heading: [
        Color::Rgb(240, 180, 60),  // H1: Sunflower
        Color::Rgb(200, 100, 50),  // H2: Rust
        Color::Rgb(100, 170, 90),  // H3: Forest
        Color::Rgb(190, 140, 90),  // H4: Caramel
        Color::Rgb(150, 140, 120), // H5: Sandstone
        Color::Rgb(130, 125, 110), // H6: Flint
    ],
    body: Color::Rgb(210, 200, 180),
    bold: Color::Rgb(210, 200, 180),
    italic: Color::Rgb(210, 200, 180),
    link: Color::Rgb(100, 170, 90),
    inline_code: Color::Rgb(200, 160, 80),
    horizontal_rule: Color::Rgb(110, 100, 80),
    diagram_border: Color::Rgb(170, 130, 70),
    diagram_collapsed: Color::Rgb(100, 170, 90),
    diagram_node_fill: Color::Rgb(200, 160, 80),
    diagram_node_border: Color::Rgb(200, 100, 50),
    diagram_node_text: Color::Rgb(210, 200, 180),
    diagram_edge_stroke: Color::Rgb(100, 170, 90),
    diagram_edge_label: Color::Rgb(150, 140, 120),
};

static FROST: Theme = Theme {
    name: "frost",
    heading: [
        Color::Rgb(100, 180, 255), // H1: Bright Blue
        Color::Rgb(90, 175, 175),  // H2: Teal
        Color::Rgb(135, 175, 215), // H3: Periwinkle
        Color::Rgb(95, 135, 175),  // H4: Steel Blue
        Color::Rgb(135, 135, 175), // H5: Lavender
        Color::Rgb(108, 112, 134), // H6: Muted Slate
    ],
    body: Color::Rgb(160, 176, 192),
    bold: Color::Rgb(160, 176, 192),
    italic: Color::Rgb(160, 176, 192),
    link: Color::Rgb(90, 175, 175),
    inline_code: Color::Rgb(135, 175, 215),
    horizontal_rule: Color::Rgb(74, 85, 104),
    diagram_border: Color::Rgb(135, 175, 215),
    diagram_collapsed: Color::Rgb(90, 175, 175),
    diagram_node_fill: Color::Rgb(135, 175, 215),
    diagram_node_border: Color::Rgb(100, 180, 255),
    diagram_node_text: Color::Rgb(160, 176, 192),
    diagram_edge_stroke: Color::Rgb(90, 175, 175),
    diagram_edge_label: Color::Rgb(135, 135, 175),
};

static NORD: Theme = Theme {
    name: "nord",
    heading: [
        Color::Rgb(136, 192, 208), // H1: Nord Frost
        Color::Rgb(129, 161, 193), // H2: Nord Frost Dark
        Color::Rgb(163, 190, 140), // H3: Nord Green
        Color::Rgb(235, 203, 139), // H4: Nord Yellow
        Color::Rgb(180, 142, 173), // H5: Nord Purple
        Color::Rgb(94, 129, 172),  // H6: Nord Blue
    ],
    body: Color::Rgb(216, 222, 233),
    bold: Color::Rgb(216, 222, 233),
    italic: Color::Rgb(216, 222, 233),
    link: Color::Rgb(136, 192, 208),
    inline_code: Color::Rgb(129, 161, 193),
    horizontal_rule: Color::Rgb(76, 86, 106),
    diagram_border: Color::Rgb(129, 161, 193),
    diagram_collapsed: Color::Rgb(136, 192, 208),
    diagram_node_fill: Color::Rgb(129, 161, 193),
    diagram_node_border: Color::Rgb(136, 192, 208),
    diagram_node_text: Color::Rgb(216, 222, 233),
    diagram_edge_stroke: Color::Rgb(163, 190, 140),
    diagram_edge_label: Color::Rgb(180, 142, 173),
};

static GLACIER: Theme = Theme {
    name: "glacier",
    heading: [
        Color::Rgb(80, 200, 220),  // H1: Ice Cyan
        Color::Rgb(100, 160, 210), // H2: Arctic Blue
        Color::Rgb(70, 180, 170),  // H3: Teal
        Color::Rgb(150, 130, 200), // H4: Amethyst
        Color::Rgb(130, 150, 180), // H5: Pale Steel
        Color::Rgb(100, 115, 140), // H6: Slate
    ],
    body: Color::Rgb(185, 200, 215),
    bold: Color::Rgb(185, 200, 215),
    italic: Color::Rgb(185, 200, 215),
    link: Color::Rgb(70, 180, 170),
    inline_code: Color::Rgb(110, 150, 200),
    horizontal_rule: Color::Rgb(55, 65, 80),
    diagram_border: Color::Rgb(110, 150, 200),
    diagram_collapsed: Color::Rgb(70, 180, 170),
    diagram_node_fill: Color::Rgb(110, 150, 200),
    diagram_node_border: Color::Rgb(80, 200, 220),
    diagram_node_text: Color::Rgb(185, 200, 215),
    diagram_edge_stroke: Color::Rgb(70, 180, 170),
    diagram_edge_label: Color::Rgb(130, 150, 180),
};

static STEEL: Theme = Theme {
    name: "steel",
    heading: [
        Color::Rgb(140, 170, 210), // H1: Soft Blue
        Color::Rgb(110, 145, 180), // H2: Slate Blue
        Color::Rgb(150, 190, 140), // H3: Sage
        Color::Rgb(180, 160, 120), // H4: Khaki
        Color::Rgb(140, 150, 165), // H5: Pewter
        Color::Rgb(120, 128, 140), // H6: Gunmetal
    ],
    body: Color::Rgb(175, 180, 190),
    bold: Color::Rgb(175, 180, 190),
    italic: Color::Rgb(175, 180, 190),
    link: Color::Rgb(110, 145, 180),
    inline_code: Color::Rgb(150, 160, 180),
    horizontal_rule: Color::Rgb(60, 65, 75),
    diagram_border: Color::Rgb(150, 160, 180),
    diagram_collapsed: Color::Rgb(110, 145, 180),
    diagram_node_fill: Color::Rgb(150, 160, 180),
    diagram_node_border: Color::Rgb(140, 170, 210),
    diagram_node_text: Color::Rgb(175, 180, 190),
    diagram_edge_stroke: Color::Rgb(150, 190, 140),
    diagram_edge_label: Color::Rgb(140, 150, 165),
};

static SOLARIZED_DARK: Theme = Theme {
    name: "solarized-dark",
    heading: [
        Color::Rgb(38, 139, 210),  // H1: Blue
        Color::Rgb(42, 161, 152),  // H2: Cyan
        Color::Rgb(133, 153, 0),   // H3: Green
        Color::Rgb(181, 137, 0),   // H4: Yellow
        Color::Rgb(108, 113, 196), // H5: Violet
        Color::Rgb(101, 123, 131), // H6: Base00
    ],
    body: Color::Rgb(131, 148, 150),
    bold: Color::Rgb(131, 148, 150),
    italic: Color::Rgb(131, 148, 150),
    link: Color::Rgb(42, 161, 152),
    inline_code: Color::Rgb(181, 137, 0),
    horizontal_rule: Color::Rgb(88, 110, 117),
    diagram_border: Color::Rgb(181, 137, 0),
    diagram_collapsed: Color::Rgb(42, 161, 152),
    diagram_node_fill: Color::Rgb(181, 137, 0),
    diagram_node_border: Color::Rgb(38, 139, 210),
    diagram_node_text: Color::Rgb(131, 148, 150),
    diagram_edge_stroke: Color::Rgb(42, 161, 152),
    diagram_edge_label: Color::Rgb(108, 113, 196),
};

static SOLARIZED_LIGHT: Theme = Theme {
    name: "solarized-light",
    heading: [
        Color::Rgb(38, 139, 210),  // H1: Blue
        Color::Rgb(42, 161, 152),  // H2: Cyan
        Color::Rgb(133, 153, 0),   // H3: Green
        Color::Rgb(181, 137, 0),   // H4: Yellow
        Color::Rgb(108, 113, 196), // H5: Violet
        Color::Rgb(147, 161, 161), // H6: Base1
    ],
    body: Color::Rgb(101, 123, 131),
    bold: Color::Rgb(101, 123, 131),
    italic: Color::Rgb(101, 123, 131),
    link: Color::Rgb(42, 161, 152),
    inline_code: Color::Rgb(181, 137, 0),
    horizontal_rule: Color::Rgb(147, 161, 161),
    diagram_border: Color::Rgb(181, 137, 0),
    diagram_collapsed: Color::Rgb(42, 161, 152),
    diagram_node_fill: Color::Rgb(181, 137, 0),
    diagram_node_border: Color::Rgb(38, 139, 210),
    diagram_node_text: Color::Rgb(101, 123, 131),
    diagram_edge_stroke: Color::Rgb(42, 161, 152),
    diagram_edge_label: Color::Rgb(108, 113, 196),
};

static PAPER: Theme = Theme {
    name: "paper",
    heading: [
        Color::Rgb(130, 80, 40),   // H1: Dark Brown
        Color::Rgb(150, 60, 30),   // H2: Burnt Sienna
        Color::Rgb(40, 105, 55),   // H3: Forest
        Color::Rgb(80, 65, 140),   // H4: Plum
        Color::Rgb(90, 105, 120),  // H5: Slate
        Color::Rgb(130, 130, 130), // H6: Gray
    ],
    body: Color::Rgb(55, 50, 45),
    bold: Color::Rgb(55, 50, 45),
    italic: Color::Rgb(55, 50, 45),
    link: Color::Rgb(40, 105, 55),
    inline_code: Color::Rgb(120, 85, 40),
    horizontal_rule: Color::Rgb(192, 184, 168),
    diagram_border: Color::Rgb(120, 85, 40),
    diagram_collapsed: Color::Rgb(40, 105, 55),
    diagram_node_fill: Color::Rgb(120, 85, 40),
    diagram_node_border: Color::Rgb(150, 60, 30),
    diagram_node_text: Color::Rgb(55, 50, 45),
    diagram_edge_stroke: Color::Rgb(40, 105, 55),
    diagram_edge_label: Color::Rgb(90, 105, 120),
};

static SNOW: Theme = Theme {
    name: "snow",
    heading: [
        Color::Rgb(25, 105, 160),  // H1: Deep Blue
        Color::Rgb(10, 120, 120),  // H2: Teal
        Color::Rgb(70, 120, 40),   // H3: Olive
        Color::Rgb(90, 75, 190),   // H4: Indigo
        Color::Rgb(100, 115, 130), // H5: Cool Gray
        Color::Rgb(130, 130, 145), // H6: Silver
    ],
    body: Color::Rgb(40, 55, 70),
    bold: Color::Rgb(40, 55, 70),
    italic: Color::Rgb(40, 55, 70),
    link: Color::Rgb(10, 120, 120),
    inline_code: Color::Rgb(80, 65, 140),
    horizontal_rule: Color::Rgb(176, 192, 208),
    diagram_border: Color::Rgb(80, 65, 140),
    diagram_collapsed: Color::Rgb(10, 120, 120),
    diagram_node_fill: Color::Rgb(80, 65, 140),
    diagram_node_border: Color::Rgb(25, 105, 160),
    diagram_node_text: Color::Rgb(40, 55, 70),
    diagram_edge_stroke: Color::Rgb(10, 120, 120),
    diagram_edge_label: Color::Rgb(100, 115, 130),
};

static LATTE: Theme = Theme {
    name: "latte",
    heading: [
        Color::Rgb(30, 102, 245),  // H1: Blue
        Color::Rgb(23, 146, 153),  // H2: Teal
        Color::Rgb(64, 160, 43),   // H3: Green
        Color::Rgb(223, 142, 29),  // H4: Yellow
        Color::Rgb(136, 57, 239),  // H5: Mauve
        Color::Rgb(108, 111, 133), // H6: Overlay
    ],
    body: Color::Rgb(76, 79, 105),
    bold: Color::Rgb(76, 79, 105),
    italic: Color::Rgb(76, 79, 105),
    link: Color::Rgb(23, 146, 153),
    inline_code: Color::Rgb(254, 100, 11),
    horizontal_rule: Color::Rgb(188, 192, 204),
    diagram_border: Color::Rgb(254, 100, 11),
    diagram_collapsed: Color::Rgb(23, 146, 153),
    diagram_node_fill: Color::Rgb(254, 100, 11),
    diagram_node_border: Color::Rgb(30, 102, 245),
    diagram_node_text: Color::Rgb(76, 79, 105),
    diagram_edge_stroke: Color::Rgb(23, 146, 153),
    diagram_edge_label: Color::Rgb(136, 57, 239),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clay_is_default() {
        let theme = Theme::default_theme();
        assert_eq!(theme.name, "clay");
    }

    #[test]
    fn test_lookup_by_name() {
        assert!(Theme::by_name("clay").is_some());
        assert!(Theme::by_name("hearth").is_some());
        assert!(Theme::by_name("nonexistent").is_none());
    }

    #[test]
    fn test_available_names() {
        let names = Theme::available_names();
        assert!(names.iter().any(|n| n == "clay"));
        assert!(names.iter().any(|n| n == "hearth"));
    }

    #[test]
    fn test_clay_heading_count() {
        let theme = Theme::by_name("clay").unwrap();
        assert_eq!(theme.heading.len(), 6);
    }

    #[test]
    fn test_hearth_heading_count() {
        let theme = Theme::by_name("hearth").unwrap();
        assert_eq!(theme.heading.len(), 6);
    }

    #[test]
    fn test_frost_lookup() {
        assert!(Theme::by_name("frost").is_some());
        assert_eq!(Theme::by_name("frost").unwrap().heading.len(), 6);
    }

    #[test]
    fn test_nord_lookup() {
        assert!(Theme::by_name("nord").is_some());
        assert_eq!(Theme::by_name("nord").unwrap().heading.len(), 6);
    }

    #[test]
    fn test_glacier_lookup() {
        assert!(Theme::by_name("glacier").is_some());
        assert_eq!(Theme::by_name("glacier").unwrap().heading.len(), 6);
    }

    #[test]
    fn test_steel_lookup() {
        assert!(Theme::by_name("steel").is_some());
        assert_eq!(Theme::by_name("steel").unwrap().heading.len(), 6);
    }

    #[test]
    fn test_paper_lookup() {
        assert!(Theme::by_name("paper").is_some());
        assert_eq!(Theme::by_name("paper").unwrap().heading.len(), 6);
    }

    #[test]
    fn test_snow_lookup() {
        assert!(Theme::by_name("snow").is_some());
        assert_eq!(Theme::by_name("snow").unwrap().heading.len(), 6);
    }

    #[test]
    fn test_latte_lookup() {
        assert!(Theme::by_name("latte").is_some());
        assert_eq!(Theme::by_name("latte").unwrap().heading.len(), 6);
    }

    #[test]
    fn test_solarized_dark_lookup() {
        assert!(Theme::by_name("solarized-dark").is_some());
        assert_eq!(Theme::by_name("solarized-dark").unwrap().heading.len(), 6);
    }

    #[test]
    fn test_solarized_light_lookup() {
        assert!(Theme::by_name("solarized-light").is_some());
        assert_eq!(Theme::by_name("solarized-light").unwrap().heading.len(), 6);
    }

    #[test]
    fn test_all_returns_every_theme() {
        let all = Theme::all();
        let names = Theme::builtin_names();
        assert_eq!(
            all.len(),
            names.len(),
            "all() and builtin_names() must match in length"
        );
        for theme in all {
            assert!(
                names.contains(&theme.name),
                "Theme '{}' in all() but not in builtin_names()",
                theme.name
            );
        }
    }

    #[test]
    fn test_resolve_builtin_theme() {
        let theme = Theme::resolve("nord").unwrap();
        assert_eq!(theme.name, "nord");
    }

    #[test]
    fn test_resolve_unknown_theme_lists_available() {
        let err = Theme::resolve("totally-not-a-theme").unwrap_err();
        assert!(err.contains("Unknown UI theme"));
        assert!(err.contains("clay"));
    }

    #[test]
    fn test_resolve_custom_theme_from_disk() {
        let dir = std::env::temp_dir().join(format!("mdx_theme_test_{}", std::process::id()));
        if !crate::test_env::in_child("theme::tests::test_resolve_custom_theme_from_disk", &dir) {
            return;
        }
        let dir = std::path::PathBuf::from(std::env::var("XDG_CONFIG_HOME").unwrap());
        let themes_dir = dir.join("mdx").join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(
            themes_dir.join("my-custom.toml"),
            r##"
heading = ["#ff79c6", "#bd93f9", "#8be9fd", "#50fa7b", "#ffb86c", "#6272a4"]
body = "#f8f8f2"
bold = "#f8f8f2"
italic = "#f8f8f2"
link = "#8be9fd"
inline_code = "#50fa7b"
horizontal_rule = "#6272a4"
diagram_border = "#bd93f9"
diagram_collapsed = "#8be9fd"
diagram_node_fill = "#bd93f9"
diagram_node_border = "#ff79c6"
diagram_node_text = "#f8f8f2"
diagram_edge_stroke = "#50fa7b"
diagram_edge_label = "#ffb86c"
"##,
        )
        .unwrap();

        let result = Theme::resolve("my-custom");

        let theme = result.expect("custom theme should load");
        assert_eq!(theme.name, "my-custom");
        assert_eq!(theme.heading[0], Color::Rgb(255, 121, 198));
        assert_eq!(theme.body, Color::Rgb(248, 248, 242));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_custom_theme_bad_hex_errors() {
        let dir = std::env::temp_dir().join(format!("mdx_theme_test_bad_{}", std::process::id()));
        if !crate::test_env::in_child(
            "theme::tests::test_resolve_custom_theme_bad_hex_errors",
            &dir,
        ) {
            return;
        }
        let dir = std::path::PathBuf::from(std::env::var("XDG_CONFIG_HOME").unwrap());
        let themes_dir = dir.join("mdx").join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(
            themes_dir.join("broken.toml"),
            r##"
heading = ["not-a-color", "#bd93f9", "#8be9fd", "#50fa7b", "#ffb86c", "#6272a4"]
body = "#f8f8f2"
bold = "#f8f8f2"
italic = "#f8f8f2"
link = "#8be9fd"
inline_code = "#50fa7b"
horizontal_rule = "#6272a4"
diagram_border = "#bd93f9"
diagram_collapsed = "#8be9fd"
diagram_node_fill = "#bd93f9"
diagram_node_border = "#ff79c6"
diagram_node_text = "#f8f8f2"
diagram_edge_stroke = "#50fa7b"
diagram_edge_label = "#ffb86c"
"##,
        )
        .unwrap();

        let result = Theme::resolve("broken");

        let err = result.expect_err("bad hex color should fail to parse");
        assert!(err.contains("heading[0]"), "err: {err}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
