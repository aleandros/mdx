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
            _ => None,
        }
    }

    pub fn default_theme() -> &'static Theme {
        &CLAY
    }

    pub fn available_names() -> &'static [&'static str] {
        &["clay", "hearth"]
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
}
