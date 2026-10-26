//! Display math typeset with `RaTeX` and rasterized with `resvg`.
//!
//! `RaTeX` parses and lays out the TeX source with `KaTeX` metrics and fonts,
//! and writes an SVG whose glyphs are outline paths, so no font has to be
//! installed. The SVG goes through the same rasterizer as Mermaid diagrams,
//! onto a canvas of whole cells with the formula centered in it, on a
//! transparent background. Glyphs take the theme's body text color.
//!
//! Typesetting runs on the image worker: until it finishes, and whenever it
//! fails, the pager shows the raw source.

use std::hash::{Hash, Hasher};

use ratex_types::color::Color;

use crate::mermaid::svg::Raster;

/// Size of one em in cell heights. Terminal fonts are about 0.8 of the line
/// height, so display math matches the size of the surrounding text.
const EM_PER_ROW: f64 = 0.8;

/// Blank space around the formula, in ems, so glyph ink that overhangs its
/// box (italic corrections, accents) is not cut off.
const PADDING_EM: f64 = 0.1;

/// Longest side of a rasterized formula, in pixels. A larger one would have
/// to shrink far below the text size to fit the content column, so it is
/// shown as source instead.
const MAX_RASTER_PX: f64 = 4096.0;

type Rgb = (u8, u8, u8);

/// One display math formula, ready to typeset for any cell size.
#[derive(Debug)]
pub(crate) struct MathImage {
    /// TeX source, without the `$$` delimiters.
    tex: String,
    color: Rgb,
    /// Identifies the source and color the formula is drawn from.
    key: u64,
}

impl PartialEq for MathImage {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.tex == other.tex
    }
}

impl MathImage {
    /// The formula `tex`, drawn in `theme`'s body text color.
    pub(crate) fn new(tex: &str, theme: &crate::theme::Theme) -> Self {
        let color = crate::mermaid::color::rgb(&theme.body);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Tagged so a formula never shares a key with a diagram: both are
        // cached in the same map of rasterized images.
        "math".hash(&mut hasher);
        tex.hash(&mut hasher);
        color.hash(&mut hasher);
        MathImage {
            tex: tex.to_string(),
            color,
            key: hasher.finish(),
        }
    }

    pub(crate) fn key(&self) -> u64 {
        self.key
    }

    /// Typeset and rasterize the formula for cells of `cell_px` pixels. The
    /// image is a whole number of cells in each direction.
    pub(crate) fn rasterize(&self, cell_px: (u16, u16)) -> anyhow::Result<Raster> {
        let (cw, ch) = if cell_px.0 == 0 || cell_px.1 == 0 {
            crate::kitty::FALLBACK_CELL_PX
        } else {
            cell_px
        };
        let svg = typeset(&self.tex, self.color, f64::from(ch) * EM_PER_ROW)?;
        crate::mermaid::svg::rasterize_centered(&svg, |(w, h)| {
            (
                w.div_ceil(u32::from(cw)) * u32::from(cw),
                h.div_ceil(u32::from(ch)) * u32::from(ch),
            )
        })
    }
}

/// Typeset `tex` in display style as a self-contained SVG with `em_px`
/// pixels per em. Fails on invalid TeX, an empty formula, or one larger than
/// [`MAX_RASTER_PX`].
pub(crate) fn typeset(tex: &str, color: Rgb, em_px: f64) -> anyhow::Result<String> {
    let ast = ratex_parser::parse(tex).map_err(|e| anyhow::anyhow!("{e}"))?;
    let options = ratex_layout::LayoutOptions::default().with_color(Color::rgb(
        f32::from(color.0) / 255.0,
        f32::from(color.1) / 255.0,
        f32::from(color.2) / 255.0,
    ));
    let tree = ratex_layout::layout(&ast, &options);
    ratex_layout::validate_layout(&tree).map_err(|e| anyhow::anyhow!(e))?;
    let list = ratex_layout::to_display_list(&tree);
    let (w_em, h_em) = (
        list.width + 2.0 * PADDING_EM,
        list.height + list.depth + 2.0 * PADDING_EM,
    );
    if !(w_em.is_finite() && h_em.is_finite())
        || list.width <= 0.0
        || list.height + list.depth <= 0.0
    {
        anyhow::bail!("empty formula");
    }
    if w_em.max(h_em) * em_px > MAX_RASTER_PX {
        anyhow::bail!("formula too large");
    }
    let options = ratex_svg::SvgOptions {
        font_size: em_px,
        padding: PADDING_EM * em_px,
        embed_glyphs: true,
        ..Default::default()
    };
    Ok(size_in_px(&ratex_svg::render_to_svg(&list, &options)))
}

/// `svg` with its root `width` and `height` in pixels. `RaTeX` gives them in
/// points, which resvg would draw 4/3 larger than the view box.
fn size_in_px(svg: &str) -> String {
    let root_end = svg.find('>').unwrap_or(0);
    let (root, rest) = svg.split_at(root_end);
    format!("{}{rest}", root.replace("pt\"", "\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Rgb = (255, 255, 255);

    /// Every `$$ ... $$` formula in `markdown`, without the delimiters.
    fn display_formulas(markdown: &str) -> Vec<String> {
        crate::parser::parse_markdown(markdown)
            .into_iter()
            .filter_map(|block| match block {
                crate::parser::Block::Math { tex } => Some(tex),
                _ => None,
            })
            .collect()
    }

    fn corpus() -> Vec<String> {
        display_formulas(include_str!("../docs/examples/math-display.md"))
    }

    #[test]
    fn corpus_has_the_common_constructs() {
        let all = corpus().join("\n");
        for construct in [r"\frac", r"\sum_", r"\begin{pmatrix}", r"\begin{aligned}"] {
            assert!(all.contains(construct), "corpus lacks {construct}");
        }
    }

    #[test]
    fn typesetting_never_panics_on_the_corpus() {
        let corpus = corpus();
        assert!(corpus.len() >= 15, "corpus has {} formulas", corpus.len());
        // The corpus ends with formulas meant to fall back to their source.
        let failing: Vec<&str> = corpus
            .iter()
            .filter(|tex| typeset(tex, WHITE, 16.0).is_err())
            .map(|tex| tex.trim())
            .collect();
        assert_eq!(
            failing,
            [
                r"\frac{a}",
                r"\begin{pmatrix} a & b",
                r"\notarealcommand{x}"
            ]
        );
    }

    #[test]
    fn common_formulas_typeset_and_rasterize_to_whole_cells() {
        let theme = crate::theme::Theme::default_theme();
        for tex in [
            r"\frac{a}{b}",
            r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}",
            r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
            r"\begin{aligned} x &= 1 \\ y &= 2 \end{aligned}",
        ] {
            let raster = MathImage::new(tex, theme)
                .rasterize((10, 20))
                .unwrap_or_else(|e| panic!("{tex}: {e}"));
            assert!(
                raster.px.0 > 0 && raster.px.0.is_multiple_of(10),
                "{tex}: {:?}",
                raster.px
            );
            assert!(
                raster.px.1 > 0 && raster.px.1.is_multiple_of(20),
                "{tex}: {:?}",
                raster.px
            );
            assert!(raster.png.starts_with(b"\x89PNG"), "{tex}");
        }
    }

    #[test]
    fn taller_formulas_take_more_rows() {
        let theme = crate::theme::Theme::default_theme();
        let rows = |tex: &str| MathImage::new(tex, theme).rasterize((10, 20)).unwrap().px.1 / 20;
        assert_eq!(rows("a + b = c"), 1);
        assert!(rows(r"\frac{a}{b}") >= 2);
        assert!(rows(r"\begin{pmatrix} 1 \\ 2 \\ 3 \\ 4 \end{pmatrix}") > rows(r"\frac{a}{b}"));
    }

    #[test]
    fn glyphs_use_the_theme_text_color_on_a_transparent_background() {
        let svg = typeset("x", (12, 34, 56), 20.0).unwrap();
        assert!(svg.contains("12,34,56"), "{svg}");
        let theme = crate::theme::Theme::default_theme();
        let raster = MathImage::new("x", theme).rasterize((10, 20)).unwrap();
        let img = image::load_from_memory(&raster.png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "corners are transparent");
        let (r, g, b) = crate::mermaid::color::rgb(&theme.body);
        assert!(
            img.pixels().any(|p| p.0 == [r, g, b, 255]),
            "some pixel is fully the body color"
        );
    }

    #[test]
    fn invalid_or_empty_formulas_fail_without_panicking() {
        for tex in [
            r"\frac{a}{",
            r"\begin{pmatrix} a",
            r"\undefinedmacro",
            "",
            "   ",
        ] {
            assert!(typeset(tex, WHITE, 16.0).is_err(), "{tex:?} typeset");
        }
    }

    #[test]
    fn huge_formulas_fall_back_instead_of_shrinking() {
        let theme = crate::theme::Theme::default_theme();
        let tex = format!("{}x", "x + ".repeat(2000));
        assert!(MathImage::new(&tex, theme).rasterize((20, 40)).is_err());
    }

    #[test]
    fn svg_size_is_in_pixels() {
        let svg = typeset("x", WHITE, 20.0).unwrap();
        let root = &svg[..svg.find('>').unwrap()];
        assert!(!root.contains("pt\""), "{root}");
        let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
        // One em plus padding, not 4/3 of it.
        assert!(tree.size().height() < 20.0 * 1.5, "{:?}", tree.size());
    }

    #[test]
    fn keys_follow_source_and_color() {
        let theme = crate::theme::Theme::default_theme();
        let a = MathImage::new("x", theme);
        assert_eq!(a.key(), MathImage::new("x", theme).key());
        assert_ne!(a.key(), MathImage::new("y", theme).key());
        let other = crate::theme::Theme::by_name("frost").unwrap();
        assert_ne!(theme.body, other.body);
        assert_ne!(a.key(), MathImage::new("x", other).key());
    }
}
