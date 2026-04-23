use crate::render::Color;

#[allow(dead_code)]
pub struct Theme {
    pub name: &'static str,
    pub heading: [Color; 6],
    pub body: Color,
    pub bold: Color,
    pub italic: Color,
    pub link: Color,
    pub inline_code: Color,
    pub horizontal_rule: Color,
    pub diagram_border: Color,
    pub diagram_collapsed: Color,
    pub diagram_node_fill: Color,
    pub diagram_node_border: Color,
    pub diagram_node_text: Color,
    pub diagram_edge_stroke: Color,
    pub diagram_edge_label: Color,
}

impl Theme {
    pub fn by_name(name: &str) -> Option<&'static Theme> {
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

    pub fn default_theme() -> &'static Theme {
        &CLAY
    }

    pub fn available_names() -> &'static [&'static str] {
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

    #[allow(dead_code)]
    pub fn all() -> &'static [&'static Theme] {
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

    pub fn all_colors(&self) -> Vec<&Color> {
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
        assert!(names.contains(&"clay"));
        assert!(names.contains(&"hearth"));
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
        let names = Theme::available_names();
        assert_eq!(
            all.len(),
            names.len(),
            "all() and available_names() must match in length"
        );
        for theme in all {
            assert!(
                names.contains(&theme.name),
                "Theme '{}' in all() but not in available_names()",
                theme.name
            );
        }
    }
}
