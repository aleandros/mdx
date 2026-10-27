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
use ratex_types::display_item::{DisplayItem, DisplayList};

use crate::mermaid::svg::Raster;

/// Size of one em in cell heights. Terminal fonts are about 0.8 of the line
/// height, so display math matches the size of the surrounding text.
const EM_PER_ROW: f64 = 0.8;

/// Blank space around the formula, in ems, so glyph ink that overhangs its
/// box (italic corrections, accents) is not cut off.
const PADDING_EM: f64 = 0.1;

/// Longest TeX source typeset, in bytes. Longer formulas show as source, so
/// pathological input cannot hold the image worker for seconds.
const MAX_TEX_BYTES: usize = 16 * 1024;

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
/// pixels per em. Fails on invalid TeX, an empty formula, one longer than
/// [`MAX_TEX_BYTES`] or larger than [`MAX_RASTER_PX`], and one that needs a
/// glyph the `KaTeX` fonts lack.
pub(crate) fn typeset(tex: &str, color: Rgb, em_px: f64) -> anyhow::Result<String> {
    if tex.len() > MAX_TEX_BYTES {
        anyhow::bail!("formula source too long");
    }
    let ast = ratex_parser::parse(tex).map_err(|e| anyhow::anyhow!("{e}"))?;
    let options = ratex_layout::LayoutOptions::default().with_color(Color::rgb(
        f32::from(color.0) / 255.0,
        f32::from(color.1) / 255.0,
        f32::from(color.2) / 255.0,
    ));
    let tree = ratex_layout::layout(&ast, &options);
    ratex_layout::validate_layout(&tree).map_err(|e| anyhow::anyhow!(e))?;
    let mut list = ratex_layout::to_display_list(&tree);
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
    keep_katex_glyphs(&mut list)?;
    let options = ratex_svg::SvgOptions {
        font_size: em_px,
        padding: PADDING_EM * em_px,
        embed_glyphs: true,
        ..Default::default()
    };
    Ok(size_in_px(&ratex_svg::render_to_svg(&list, &options)))
}

/// Make every glyph in `list` one `ratex-svg` draws from the embedded `KaTeX`
/// fonts, or fail. Glyphs it cannot outline there, it draws from system
/// fonts: loading those prints to stderr (over the pager's screen) and, for
/// emoji, reads a large color font whose bitmaps resvg here cannot draw.
///
/// Mirrors the lookup in `ratex-svg`: a glyph is drawn when it has an outline
/// in its own font or in Main-Regular. Glyphs those fonts have but cannot
/// outline (spaces such as U+2009) draw nothing, so they are dropped. A glyph
/// neither font has fails the formula, which then shows as source.
fn keep_katex_glyphs(list: &mut DisplayList) -> anyhow::Result<()> {
    use ab_glyph::Font as _;
    use ratex_font::FontId;
    let glyph = |item: &DisplayItem| match item {
        DisplayItem::GlyphPath {
            font, char_code, ..
        } => Some((
            FontId::parse(font).unwrap_or(FontId::MainRegular),
            *char_code,
        )),
        _ => None,
    };
    // Checked first: loading the fonts for these ids would reach the system.
    if list.items.iter().filter_map(glyph).any(|(font, _)| {
        matches!(
            font,
            FontId::CjkRegular | FontId::CjkFallback | FontId::EmojiFallback
        )
    }) {
        anyhow::bail!("formula needs a font other than KaTeX");
    }
    let fonts = ratex_font_loader::load_fonts_for_items_parsed("", &list.items)
        .map_err(|e| anyhow::anyhow!(e))?;
    let main = fonts
        .get(&FontId::MainRegular)
        .ok_or_else(|| anyhow::anyhow!("KaTeX Main-Regular font missing"))?;
    let has = |font: &ab_glyph::FontVec, ch: char| font.glyph_id(ch).0 != 0;
    let outlined = |font: &ab_glyph::FontVec, ch: char| {
        has(font, ch)
            && font
                .outline(font.glyph_id(ch))
                .is_some_and(|outline| !outline.curves.is_empty())
    };
    let mut missing = None;
    list.items.retain(|item| {
        let Some((font_id, char_code)) = glyph(item) else {
            return true;
        };
        let own = fonts.get(&font_id).unwrap_or(main);
        let ch = ratex_font::katex_ttf_glyph_char(font_id, char_code);
        if outlined(own, ch) || outlined(main, ch) {
            true
        } else {
            if !has(own, ch) && !has(main, ch) {
                missing = Some(ch);
            }
            false
        }
    });
    match missing {
        Some(ch) => anyhow::bail!("no KaTeX glyph for {ch:?}"),
        None => Ok(()),
    }
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
                r"\notarealcommand{x}",
                r"\text{速度} = \frac{d}{t}",
                r"\text{😀} + 1",
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
    fn glyphs_outside_the_katex_fonts_fall_back() {
        // These would come from system fonts, which RaTeX loads with output
        // on stderr; emoji would be drawn as bitmaps resvg here drops.
        for tex in [r"\text{α}", r"\text{日本}", r"\text{€}", r"\text{😀}", "😀"] {
            assert!(typeset(tex, WHITE, 16.0).is_err(), "{tex} typeset");
        }
        // KaTeX itself covers these. Blank glyphs (thin, en and em spaces,
        // a space in `\verb` or `\char`) draw nothing and are dropped. Any
        // `<text>` would mean a glyph left for system fonts.
        for tex in [
            r"\alpha",
            "α",
            r"\mathbb{R}",
            r"\text{é}",
            r"a \times b",
            "1\u{2009}000",
            "a\u{2002}b\u{2003}c\u{200A}d",
            r"\verb|a b|",
            r#"a\char"20 b"#,
            r#"a\char"A0 b"#,
        ] {
            let svg = typeset(tex, WHITE, 16.0).unwrap_or_else(|e| panic!("{tex}: {e}"));
            assert!(!svg.contains("<text"), "{tex}: {svg}");
        }
    }

    #[test]
    fn very_long_sources_are_not_typeset() {
        let tex = "x".repeat(MAX_TEX_BYTES + 1);
        assert!(typeset(&tex, WHITE, 16.0).is_err());
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
