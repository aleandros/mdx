//! SVG backend for Mermaid diagrams, rasterized with resvg.
//!
//! The SVG is drawn from the same layouts as the ASCII renderers. Each cell
//! maps to a `cw x ch` pixel rectangle, so the image fills exactly the cell
//! rectangle the ASCII version takes. Lines run through cell centers, as
//! box-drawing characters do, and every run of label text is fitted to the
//! cells the layout reserved for it (`textLength`), so no font can make a
//! label overflow its shape.
//!
//! Lines that labels, boxes and markers sit on are cut out with a mask
//! instead of being painted over, because the image background is
//! transparent: the terminal's own background shows through.

use std::fmt::{self, Write as _};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

use super::er::{Cardinality, EntityLineKind};
use super::layout::{LayoutResult, PositionedEdge, PositionedNode, SubgraphBox};
use super::sequence::layout::{
    PositionedFragment, PositionedMessage, PositionedNote, PositionedParticipant, SequenceLayout,
};
use super::sequence::{ArrowStyle, FragmentKind};
use super::{EdgeStyle, NodeShape, NodeStyle, color, display_width};
use crate::render::StyledLine;

/// Font bundled with mdx, so labels look the same on every machine. System
/// fonts are loaded too, for glyphs it lacks (CJK, emoji).
const FALLBACK_FONT: &[u8] = include_bytes!("../../assets/fonts/DejaVuSansMono.ttf");
const FONT_FAMILY: &str = "DejaVu Sans Mono";

/// Horizontal advance of [`FALLBACK_FONT`] glyphs, in ems.
const FONT_ADVANCE_EM: f64 = 0.602;

/// Longest side of a rasterized diagram, in pixels. Larger diagrams are drawn
/// with smaller cells; the terminal scales the image to the placement.
const MAX_RASTER_PX: f64 = 4096.0;

type Rgb = (u8, u8, u8);

/// Theme colors the SVG uses where the diagram itself sets none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Palette {
    /// Body text color; stands in for the terminal's default foreground,
    /// which ASCII diagrams use for unstyled lines.
    text: Rgb,
    node_border: Rgb,
    node_fill: Rgb,
    node_text: Rgb,
    edge_label: Rgb,
    border: Rgb,
}

impl Palette {
    fn new(theme: &crate::theme::Theme) -> Self {
        Palette {
            text: color::rgb(&theme.body),
            node_border: color::rgb(&theme.diagram_node_border),
            node_fill: color::rgb(&theme.diagram_node_fill),
            node_text: color::rgb(&theme.diagram_node_text),
            edge_label: color::rgb(&theme.diagram_edge_label),
            border: color::rgb(&theme.diagram_border),
        }
    }
}

#[derive(Debug)]
enum Scene {
    /// Flowcharts and ER diagrams (which are laid out as flowcharts).
    Flowchart(LayoutResult),
    Sequence(SequenceLayout),
}

/// A laid-out diagram that can be drawn as SVG at any cell size.
#[derive(Debug)]
pub(crate) struct DiagramImage {
    scene: Scene,
    palette: Palette,
    /// Size of the ASCII rendering in cells; the image covers exactly this.
    cols: usize,
    rows: usize,
    /// Identifies the source, width and theme the diagram was rendered from.
    key: u64,
}

impl PartialEq for DiagramImage {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.cols == other.cols && self.rows == other.rows
    }
}

/// Cache key for a diagram rendered from `source` at `width` columns with
/// `theme`: equal keys always produce the same layout and colors.
pub(crate) fn source_key(source: &str, width: usize, theme: &crate::theme::Theme) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    width.hash(&mut hasher);
    // Custom themes can share a name, so hash every color.
    format!("{theme:?}").hash(&mut hasher);
    hasher.finish()
}

/// Columns and rows the ASCII `lines` occupy.
fn ascii_size(lines: &[StyledLine]) -> (usize, usize) {
    let cols = lines
        .iter()
        .map(|line| line.spans.iter().map(|s| display_width(&s.text)).sum())
        .max()
        .unwrap_or(0);
    (cols, lines.len())
}

impl DiagramImage {
    /// A flowchart or ER diagram covering the same cells as its ASCII `lines`.
    pub(crate) fn flowchart(
        layout: LayoutResult,
        lines: &[StyledLine],
        theme: &crate::theme::Theme,
        key: u64,
    ) -> Self {
        Self::new(Scene::Flowchart(layout), lines, theme, key)
    }

    /// A sequence diagram covering the same cells as its ASCII `lines`.
    pub(crate) fn sequence(
        layout: SequenceLayout,
        lines: &[StyledLine],
        theme: &crate::theme::Theme,
        key: u64,
    ) -> Self {
        Self::new(Scene::Sequence(layout), lines, theme, key)
    }

    fn new(scene: Scene, lines: &[StyledLine], theme: &crate::theme::Theme, key: u64) -> Self {
        let (cols, rows) = ascii_size(lines);
        DiagramImage {
            scene,
            palette: Palette::new(theme),
            cols,
            rows,
            key,
        }
    }

    pub(crate) fn key(&self) -> u64 {
        self.key
    }

    /// Size in cells as (columns, rows).
    pub(crate) fn cells(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    /// The diagram as an SVG document for cells of `cell_px` pixels.
    pub(crate) fn to_svg(&self, cell_px: (u16, u16)) -> String {
        let mut canvas = Canvas::new(cell_px, self.cols, self.rows);
        match &self.scene {
            Scene::Flowchart(layout) => draw_flowchart(&mut canvas, layout, &self.palette),
            Scene::Sequence(layout) => draw_sequence(&mut canvas, layout, &self.palette),
        }
        canvas.finish()
    }
}

// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "diagram coordinates are small cell counts, exact in f64"
)]
fn float(n: usize) -> f64 {
    n as f64
}

/// A number written with at most two decimals and no trailing zeros, so the
/// SVG is compact and stable for snapshots.
struct Num(f64);

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = format!("{:.2}", self.0);
        let text = text.trim_end_matches('0').trim_end_matches('.');
        f.write_str(if text == "-0" { "0" } else { text })
    }
}

struct Hex(Rgb);

impl fmt::Display for Hex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (r, g, b) = self.0;
        write!(f, "#{r:02x}{g:02x}{b:02x}")
    }
}

/// `text` escaped for XML. Control characters, which XML cannot carry, are
/// dropped.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// The longest prefix of `text` that fits in `max_cols` columns.
fn clip(text: &str, max_cols: usize) -> &str {
    let mut used = 0;
    for (i, ch) in text.char_indices() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w > max_cols {
            return &text[..i];
        }
        used += w;
    }
    text
}

type Point = (f64, f64);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dash {
    Solid,
    Dotted,
    Dashed,
}

struct Canvas {
    cw: f64,
    ch: f64,
    width: f64,
    height: f64,
    /// Base stroke width.
    stroke: f64,
    font_size: f64,
    /// Lines: edges, lifelines, borders. Drawn under `gaps`.
    wires: String,
    /// Areas cut out of `wires`: labels, boxes, markers.
    gaps: String,
    /// Node shapes and other boxes, drawn over the wires.
    shapes: String,
    labels: String,
}

impl Canvas {
    fn new(cell_px: (u16, u16), cols: usize, rows: usize) -> Self {
        let (cw, ch) = if cell_px.0 == 0 || cell_px.1 == 0 {
            crate::kitty::FALLBACK_CELL_PX
        } else {
            cell_px
        };
        let (mut cw, mut ch) = (f64::from(cw), f64::from(ch));
        let longest = (float(cols) * cw).max(float(rows) * ch);
        if longest > MAX_RASTER_PX {
            let scale = MAX_RASTER_PX / longest;
            cw *= scale;
            ch *= scale;
        }
        let unit = cw.min(ch);
        Canvas {
            cw,
            ch,
            width: float(cols) * cw,
            height: float(rows) * ch,
            stroke: (unit * 0.12).max(1.0),
            font_size: (ch * 0.8).min(cw / FONT_ADVANCE_EM),
            wires: String::new(),
            gaps: String::new(),
            shapes: String::new(),
            labels: String::new(),
        }
    }

    /// Smaller side of a cell, the unit for marker sizes.
    fn unit(&self) -> f64 {
        self.cw.min(self.ch)
    }

    /// Left edge of column `col`.
    fn left(&self, col: usize) -> f64 {
        float(col) * self.cw
    }

    /// Top edge of row `row`.
    fn top(&self, row: usize) -> f64 {
        float(row) * self.ch
    }

    /// Center of cell `(col, row)`.
    fn center(&self, (col, row): (usize, usize)) -> Point {
        ((float(col) + 0.5) * self.cw, (float(row) + 0.5) * self.ch)
    }

    fn dash_attr(&self, dash: Dash) -> String {
        match dash {
            Dash::Solid => String::new(),
            Dash::Dotted => format!(
                r#" stroke-dasharray="{} {}""#,
                Num(self.unit() * 0.25),
                Num(self.unit() * 0.25)
            ),
            Dash::Dashed => format!(
                r#" stroke-dasharray="{} {}""#,
                Num(self.unit() * 0.5),
                Num(self.unit() * 0.4)
            ),
        }
    }

    fn polyline(&mut self, points: &[Point], color: Rgb, width: f64, dash: Dash) {
        if points.len() < 2 {
            return;
        }
        let dash = self.dash_attr(dash);
        let _ = write!(self.wires, r#"<polyline points=""#);
        for (i, (x, y)) in points.iter().enumerate() {
            let sep = if i == 0 { "" } else { " " };
            let _ = write!(self.wires, "{sep}{},{}", Num(*x), Num(*y));
        }
        let _ = writeln!(
            self.wires,
            r#"" stroke="{}" stroke-width="{}" stroke-linejoin="round"{dash}/>"#,
            Hex(color),
            Num(width)
        );
    }

    fn line(&mut self, from: Point, to: Point, color: Rgb, dash: Dash) {
        let width = self.stroke;
        self.polyline(&[from, to], color, width, dash);
    }

    /// A filled triangle with its tip at `tip`, pointing along `dir`.
    fn arrowhead(&mut self, tip: Point, dir: Point, color: Rgb) {
        let (len, half) = (self.unit() * 0.9, self.unit() * 0.45);
        let base = (tip.0 - dir.0 * len, tip.1 - dir.1 * len);
        let normal = (-dir.1, dir.0);
        let _ = writeln!(
            self.wires,
            r#"<polygon points="{},{} {},{} {},{}" fill="{}"/>"#,
            Num(tip.0),
            Num(tip.1),
            Num(base.0 + normal.0 * half),
            Num(base.1 + normal.1 * half),
            Num(base.0 - normal.0 * half),
            Num(base.1 - normal.1 * half),
            Hex(color)
        );
    }

    /// Outline (and optional translucent fill) of a polygon in the shapes
    /// layer, cut out of the wires below it.
    fn shape_polygon(&mut self, points: &[Point], stroke: Rgb, fill: Option<(Rgb, f64)>) {
        let mut list = String::new();
        for (i, (x, y)) in points.iter().enumerate() {
            let sep = if i == 0 { "" } else { " " };
            let _ = write!(list, "{sep}{},{}", Num(*x), Num(*y));
        }
        let _ = writeln!(self.gaps, r##"<polygon points="{list}" fill="#000"/>"##);
        let _ = writeln!(
            self.shapes,
            r#"<polygon points="{list}" stroke="{}" stroke-width="{}" stroke-linejoin="round"{}/>"#,
            Hex(stroke),
            Num(self.stroke),
            fill_attr(fill)
        );
    }

    /// A box through the centers of its border cells, like the box-drawing
    /// characters the ASCII renderer uses. Returns its pixel rectangle.
    fn cell_box(&self, x: usize, y: usize, w: usize, h: usize) -> (f64, f64, f64, f64) {
        let (x0, y0) = self.center((x, y));
        let (x1, y1) = self.center((x + w.saturating_sub(1), y + h.saturating_sub(1)));
        (x0, y0, x1 - x0, y1 - y0)
    }

    /// A box in the shapes layer, cut out of the wires below it.
    fn shape_rect(
        &mut self,
        (x, y, w, h): (f64, f64, f64, f64),
        rx: f64,
        stroke: Rgb,
        fill: Option<(Rgb, f64)>,
    ) {
        let _ = writeln!(
            self.gaps,
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="#000"/>"##,
            Num(x),
            Num(y),
            Num(w),
            Num(h),
            Num(rx)
        );
        let _ = writeln!(
            self.shapes,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" stroke="{}" stroke-width="{}"{}/>"#,
            Num(x),
            Num(y),
            Num(w),
            Num(h),
            Num(rx),
            Hex(stroke),
            Num(self.stroke),
            fill_attr(fill)
        );
    }

    /// Cut `cols` cells starting at `(col, row)` out of the wires.
    fn gap_cells(&mut self, col: usize, row: usize, cols: usize) {
        if cols == 0 {
            return;
        }
        let _ = writeln!(
            self.gaps,
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#000"/>"##,
            Num(self.left(col)),
            Num(self.top(row)),
            Num(float(cols) * self.cw),
            Num(self.ch)
        );
    }

    /// Text starting in cell `(col, row)`. Each run between spaces gets
    /// exactly the cells it takes in the ASCII rendering.
    fn text(&mut self, col: usize, row: usize, text: &str, color: Rgb) {
        let baseline = self.top(row) + self.ch / 2.0 + self.font_size * 0.36;
        let mut at = col;
        for run in text.split(' ') {
            let cols = display_width(run);
            if cols > 0 {
                let _ = writeln!(
                    self.labels,
                    r#"<text x="{}" y="{}" textLength="{}" lengthAdjust="spacingAndGlyphs" fill="{}">{}</text>"#,
                    Num(self.left(at)),
                    Num(baseline),
                    Num(float(cols) * self.cw),
                    Hex(color),
                    escape(run)
                );
            }
            at += cols + 1;
        }
    }

    /// Text that also cuts its cells out of the wires below it.
    fn label(&mut self, col: usize, row: usize, text: &str, color: Rgb) {
        self.gap_cells(col, row, display_width(text));
        self.text(col, row, text, color);
    }

    fn finish(self) -> String {
        let (w, h) = (Num(self.width), Num(self.height));
        let mut out = String::new();
        let _ = writeln!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#
        );
        let mask = if self.gaps.is_empty() {
            ""
        } else {
            let _ = writeln!(
                out,
                r##"<defs><mask id="gaps" maskUnits="userSpaceOnUse" x="0" y="0" width="{w}" height="{h}"><rect width="{w}" height="{h}" fill="#fff"/>"##
            );
            out.push_str(&self.gaps);
            out.push_str("</mask></defs>\n");
            r#" mask="url(#gaps)""#
        };
        let _ = writeln!(out, r#"<g{mask} fill="none">"#);
        out.push_str(&self.wires);
        out.push_str("</g>\n<g fill=\"none\">\n");
        out.push_str(&self.shapes);
        let _ = writeln!(
            out,
            r#"</g>
<g font-family="{FONT_FAMILY}, monospace" font-size="{}">"#,
            Num(self.font_size)
        );
        out.push_str(&self.labels);
        out.push_str("</g>\n</svg>\n");
        out
    }
}

fn fill_attr(fill: Option<(Rgb, f64)>) -> String {
    fill.map_or_else(String::new, |(color, opacity)| {
        format!(r#" fill="{}" fill-opacity="{}""#, Hex(color), Num(opacity))
    })
}

/// Unit vector from `from` to `to` along a horizontal or vertical segment.
fn direction(from: Point, to: Point) -> Point {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = dx.hypot(dy);
    if len == 0.0 {
        (0.0, 0.0)
    } else {
        (dx / len, dy / len)
    }
}

// ---------------------------------------------------------------------------
// Flowcharts and ER diagrams
// ---------------------------------------------------------------------------

/// Fill opacity for a node's fill color: a user-set fill shows stronger than
/// the theme's default tint.
const DEFAULT_FILL_OPACITY: f64 = 0.15;
const USER_FILL_OPACITY: f64 = 0.35;

fn node_colors(style: Option<&NodeStyle>, palette: &Palette) -> (Rgb, (Rgb, f64), Rgb) {
    let stroke = style
        .and_then(|s| s.stroke.as_ref().or(s.fill.as_ref()))
        .map_or(palette.node_border, color::rgb);
    let fill = style
        .and_then(|s| s.fill.as_ref())
        .map_or((palette.node_fill, DEFAULT_FILL_OPACITY), |c| {
            (color::rgb(c), USER_FILL_OPACITY)
        });
    let text = style
        .and_then(|s| s.color.as_ref())
        .map_or(palette.node_text, color::rgb);
    (stroke, fill, text)
}

fn draw_flowchart(canvas: &mut Canvas, layout: &LayoutResult, palette: &Palette) {
    for sg in &layout.subgraph_boxes {
        draw_subgraph(canvas, sg, palette);
    }
    for edge in &layout.edges {
        draw_edge(canvas, edge, &layout.nodes, palette);
    }
    for node in &layout.nodes {
        draw_node(canvas, node, palette);
    }
}

fn draw_subgraph(canvas: &mut Canvas, sg: &SubgraphBox, palette: &Palette) {
    // Same guard as the ASCII renderer.
    if sg.width < 4 || sg.height < 2 {
        return;
    }
    let color = sg.border_color.as_ref().map_or(palette.border, color::rgb);
    let (x, y, w, h) = canvas.cell_box(sg.x, sg.y, sg.width, sg.height);
    let rect = [(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)];
    let width = canvas.stroke;
    canvas.polyline(&rect, color, width, Dash::Solid);
    // " label " on the top border, clipped before the right corner.
    let room = sg.width.saturating_sub(2);
    let title = clip(&sg.label, room.saturating_sub(2));
    if !title.is_empty() {
        canvas.gap_cells(sg.x + 1, sg.y, display_width(title) + 2);
        canvas.text(sg.x + 2, sg.y, title, color);
    }
}

fn node_contains(node: &PositionedNode, (x, y): (usize, usize)) -> bool {
    (node.x..node.x + node.width).contains(&x) && (node.y..node.y + node.height).contains(&y)
}

/// The cell one step before `cell` against the direction `from -> cell`.
fn step_back(cell: (usize, usize), toward: (usize, usize)) -> Option<(usize, usize)> {
    let (x, y) = cell;
    match (toward.0.cmp(&x), toward.1.cmp(&y)) {
        (std::cmp::Ordering::Greater, _) => Some((x.checked_sub(1)?, y)),
        (std::cmp::Ordering::Less, _) => Some((x + 1, y)),
        (_, std::cmp::Ordering::Greater) => Some((x, y.checked_sub(1)?)),
        (_, std::cmp::Ordering::Less) => Some((x, y + 1)),
        _ => None,
    }
}

/// The cell an edge's line starts in. Forward edges leave from the first
/// cell outside their source, so they start one cell back, on the source's
/// border where the box line is. Edges that already start on a border (back
/// edges) start there, not inside the box.
fn edge_start(points: &[(usize, usize)], nodes: &[PositionedNode]) -> (usize, usize) {
    let first = points[0];
    if nodes.iter().any(|n| node_contains(n, first)) {
        return first;
    }
    points
        .get(1)
        .and_then(|&next| step_back(first, next))
        .filter(|&back| nodes.iter().any(|n| node_contains(n, back)))
        .unwrap_or(first)
}

fn draw_edge(
    canvas: &mut Canvas,
    edge: &PositionedEdge,
    nodes: &[PositionedNode],
    palette: &Palette,
) {
    let color = edge
        .edge_style
        .as_ref()
        .and_then(|s| s.stroke.as_ref())
        .map_or(palette.text, color::rgb);
    if edge.points.len() >= 2 {
        let mut points: Vec<Point> = edge.points.iter().map(|&p| canvas.center(p)).collect();
        points[0] = canvas.center(edge_start(&edge.points, nodes));
        let (width, dash) = match edge.style {
            EdgeStyle::Dotted => (canvas.stroke, Dash::Dotted),
            EdgeStyle::Thick => (canvas.stroke * 2.2, Dash::Solid),
            EdgeStyle::Arrow | EdgeStyle::Line => (canvas.stroke, Dash::Solid),
        };
        canvas.polyline(&points, color, width, dash);
        let last = points[points.len() - 1];
        let prev = points[points.len() - 2];
        if let Some(meta) = &edge.er_meta {
            let first = points[0];
            let second = points[1];
            draw_cardinality(
                canvas,
                first,
                direction(first, second),
                meta.left_card,
                color,
            );
            draw_cardinality(canvas, last, direction(last, prev), meta.right_card, color);
        } else if matches!(
            edge.style,
            EdgeStyle::Arrow | EdgeStyle::Dotted | EdgeStyle::Thick
        ) {
            canvas.arrowhead(last, direction(prev, last), color);
        }
    }
    if let Some((label, (x, y))) = super::ascii::edge_label_origin(edge) {
        let label_color = edge
            .edge_style
            .as_ref()
            .and_then(|s| s.label_color.as_ref().or(s.stroke.as_ref()))
            .map_or(palette.edge_label, color::rgb);
        canvas.label(x, y, label, label_color);
    }
}

/// Crow's foot marks for one end of an ER relationship. `at` is the point on
/// the entity's border and `out` points away from the entity along the edge.
/// The mark next to the entity gives the maximum (one or many), the one
/// further out the minimum (zero or one).
fn draw_cardinality(canvas: &mut Canvas, at: Point, out: Point, card: Cardinality, color: Rgb) {
    let unit = canvas.unit();
    let normal = (-out.1, out.0);
    let along = |d: f64| (at.0 + out.0 * d, at.1 + out.1 * d);
    let spread = unit * 0.55;
    let bar = |canvas: &mut Canvas, d: f64| {
        let c = along(d);
        canvas.line(
            (c.0 + normal.0 * spread, c.1 + normal.1 * spread),
            (c.0 - normal.0 * spread, c.1 - normal.1 * spread),
            color,
            Dash::Solid,
        );
    };
    let many = matches!(card, Cardinality::ZeroOrMany | Cardinality::OneOrMany);
    if many {
        let tip = along(unit);
        for side in [-1.0, 0.0, 1.0] {
            let foot = (
                at.0 + normal.0 * spread * side,
                at.1 + normal.1 * spread * side,
            );
            canvas.line(tip, foot, color, Dash::Solid);
        }
    } else {
        bar(canvas, unit * 0.6);
    }
    match card {
        Cardinality::ZeroOrOne | Cardinality::ZeroOrMany => {
            let c = along(unit * 1.6);
            let r = unit * 0.35;
            let _ = writeln!(
                canvas.gaps,
                r##"<circle cx="{}" cy="{}" r="{}" fill="#000"/>"##,
                Num(c.0),
                Num(c.1),
                Num(r)
            );
            let _ = writeln!(
                canvas.shapes,
                r#"<circle cx="{}" cy="{}" r="{}" stroke="{}" stroke-width="{}"/>"#,
                Num(c.0),
                Num(c.1),
                Num(r),
                Hex(color),
                Num(canvas.stroke)
            );
        }
        Cardinality::ExactlyOne | Cardinality::OneOrMany => bar(canvas, unit * 1.3),
    }
}

fn draw_node(canvas: &mut Canvas, node: &PositionedNode, palette: &Palette) {
    if node.width < 2 || node.height < 2 {
        return;
    }
    let (stroke, fill, text) = node_colors(node.node_style.as_ref(), palette);
    let rect = canvas.cell_box(node.x, node.y, node.width, node.height);
    match node.shape {
        NodeShape::Rect => canvas.shape_rect(rect, 0.0, stroke, Some(fill)),
        NodeShape::Rounded => canvas.shape_rect(rect, canvas.ch * 0.5, stroke, Some(fill)),
        // A stadium: round ends that stay clear of the label.
        NodeShape::Circle => canvas.shape_rect(rect, rect.3 / 2.0, stroke, Some(fill)),
        NodeShape::Diamond => {
            let outline = diamond_outline(canvas, node, rect);
            canvas.shape_polygon(&outline, stroke, Some(fill));
        }
        NodeShape::EntityBox => {
            draw_entity(canvas, node, rect, stroke, fill, text);
            return;
        }
    }
    let (col, row) = node_label_origin(node);
    canvas.text(col, row, &node.label, text);
}

/// Cell where a node's label starts: centered on the middle row. Diamonds
/// are widest there, and the layout makes them tall enough that their sides
/// clear the label.
fn node_label_origin(node: &PositionedNode) -> (usize, usize) {
    let label_w = display_width(&node.label);
    (
        node.x + node.width.saturating_sub(label_w) / 2,
        node.y + node.height / 2,
    )
}

/// A diamond node's outline: a hexagon for compact (LR/RL) diamonds, a
/// rhombus otherwise.
fn diamond_outline(
    canvas: &Canvas,
    node: &PositionedNode,
    (x, y, w, h): (f64, f64, f64, f64),
) -> Vec<Point> {
    if node.compact {
        let inset = canvas.cw;
        vec![
            (x, y + h / 2.0),
            (x + inset, y),
            (x + w - inset, y),
            (x + w, y + h / 2.0),
            (x + w - inset, y + h),
            (x + inset, y + h),
        ]
    } else {
        vec![
            (x + w / 2.0, y),
            (x + w, y + h / 2.0),
            (x + w / 2.0, y + h),
            (x, y + h / 2.0),
        ]
    }
}

fn draw_entity(
    canvas: &mut Canvas,
    node: &PositionedNode,
    rect: (f64, f64, f64, f64),
    stroke: Rgb,
    fill: (Rgb, f64),
    text: Rgb,
) {
    canvas.shape_rect(rect, 0.0, stroke, Some(fill));
    let Some(entity) = &node.entity else {
        canvas.text(node.x + 2, node.y + node.height / 2, &node.label, text);
        return;
    };
    let (x, _, w, _) = rect;
    let inner_cols = node.width.saturating_sub(2);
    for (i, line) in entity.rendered_lines.iter().enumerate() {
        let row = node.y + 1 + i;
        if row + 1 >= node.y + node.height {
            break;
        }
        match line.kind {
            EntityLineKind::Separator => {
                let (_, cy) = canvas.center((node.x, row));
                let _ = writeln!(
                    canvas.shapes,
                    r#"<line x1="{}" y1="{cy}" x2="{}" y2="{cy}" stroke="{}" stroke-width="{}"/>"#,
                    Num(x),
                    Num(x + w),
                    Hex(stroke),
                    Num(canvas.stroke),
                    cy = Num(cy)
                );
            }
            EntityLineKind::Header => {
                // Tint the header band from the top border to the separator.
                let top = rect.1;
                let bottom = canvas.center((node.x, row + 1)).1;
                let _ = writeln!(
                    canvas.shapes,
                    r#"<rect x="{}" y="{}" width="{}" height="{}"{}/>"#,
                    Num(x),
                    Num(top),
                    Num(w),
                    Num(bottom - top),
                    fill_attr(Some(fill))
                );
                canvas.text(node.x + 1, row, clip(&line.text, inner_cols), text);
            }
            EntityLineKind::AttrRow | EntityLineKind::CommentRow => {
                canvas.text(node.x + 1, row, clip(&line.text, inner_cols), text);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Sequence diagrams
// ---------------------------------------------------------------------------

/// Rows taken by a participant box; lifelines start below it.
const PARTICIPANT_ROWS: usize = 3;

fn draw_sequence(canvas: &mut Canvas, layout: &SequenceLayout, palette: &Palette) {
    for p in &layout.participants {
        let top = canvas.center((p.center_x, PARTICIPANT_ROWS - 1));
        let bottom = canvas.center((p.center_x, layout.height.saturating_sub(1)));
        if bottom.1 > top.1 {
            canvas.line(top, bottom, palette.text, Dash::Dashed);
        }
    }
    // Outer fragments first, so nested headers cut through their borders.
    for frag in layout.fragments.iter().rev() {
        draw_fragment(canvas, frag, palette);
    }
    for act in &layout.activations {
        if act.y_start >= act.y_end {
            continue;
        }
        let x = canvas.center((act.x, 0)).0;
        let top = canvas.center((act.x, act.y_start)).1;
        let bottom = canvas
            .center((act.x, act.y_end - 1))
            .1
            .max(top + canvas.ch / 2.0);
        let rect = (x - canvas.cw, top, canvas.cw * 2.0, bottom - top);
        canvas.shape_rect(rect, 0.0, palette.text, Some((palette.node_fill, 0.3)));
    }
    for msg in &layout.messages {
        if msg.self_message {
            draw_self_message(canvas, msg, layout, palette);
        } else {
            draw_message(canvas, msg, layout, palette);
        }
    }
    for note in &layout.notes {
        draw_note(canvas, note, palette);
    }
    for p in &layout.participants {
        draw_participant(canvas, p, palette);
    }
}

fn fragment_kind(kind: &FragmentKind) -> &'static str {
    match kind {
        FragmentKind::Loop => "loop",
        FragmentKind::Alt => "alt",
        FragmentKind::Opt => "opt",
        FragmentKind::Par => "par",
    }
}

fn draw_fragment(canvas: &mut Canvas, frag: &PositionedFragment, palette: &Palette) {
    if frag.width < 2 || frag.height < 2 {
        return;
    }
    let color = palette.border;
    let (x, y, w, h) = canvas.cell_box(frag.x, frag.y, frag.width, frag.height);
    let rect = [(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)];
    let width = canvas.stroke;
    canvas.polyline(&rect, color, width, Dash::Solid);
    // Titles start where the ASCII "─ " prefix ends and stop before the
    // right border.
    let room = frag.width.saturating_sub(4);
    let kind = fragment_kind(&frag.kind);
    let title = if frag.label.is_empty() {
        kind.to_string()
    } else {
        format!("{kind} [{}]", frag.label)
    };
    let title = clip(&title, room);
    canvas.gap_cells(frag.x + 2, frag.y, display_width(title) + 2);
    canvas.text(frag.x + 3, frag.y, title, color);
    for (div_y, div_label) in &frag.section_dividers {
        if *div_y < frag.y || *div_y >= frag.y + frag.height {
            continue;
        }
        let (_, cy) = canvas.center((frag.x, *div_y));
        canvas.line((x, cy), (x + w, cy), color, Dash::Dashed);
        if let Some(label) = div_label {
            let title = format!("[{label}]");
            let title = clip(&title, room);
            canvas.gap_cells(frag.x + 2, *div_y, display_width(title) + 2);
            canvas.text(frag.x + 3, *div_y, title, color);
        }
    }
}

/// Whether an activation box covers lifeline `x` at `row`.
fn activation_at(layout: &SequenceLayout, x: usize, row: usize) -> bool {
    layout
        .activations
        .iter()
        .any(|a| a.x == x && (a.y_start..a.y_end).contains(&row))
}

fn message_colors(msg: &PositionedMessage, palette: &Palette) -> (Rgb, Rgb) {
    let style = msg.edge_style.as_ref();
    let line = style
        .and_then(|s| s.stroke.as_ref())
        .map_or(palette.text, color::rgb);
    let label = style
        .and_then(|s| s.label_color.as_ref().or(s.stroke.as_ref()))
        .map_or(palette.edge_label, color::rgb);
    (line, label)
}

fn message_dash(arrow: &ArrowStyle) -> Dash {
    match arrow {
        ArrowStyle::SolidArrow | ArrowStyle::SolidOpen | ArrowStyle::SolidCross => Dash::Solid,
        ArrowStyle::DashedArrow | ArrowStyle::DashedOpen | ArrowStyle::DashedCross => Dash::Dashed,
    }
}

/// The head of a message arriving at `tip` along `dir`.
fn message_head(canvas: &mut Canvas, tip: Point, dir: Point, arrow: &ArrowStyle, color: Rgb) {
    let (len, half) = (canvas.unit() * 0.9, canvas.unit() * 0.45);
    let normal = (-dir.1, dir.0);
    match arrow {
        ArrowStyle::SolidArrow | ArrowStyle::DashedArrow => canvas.arrowhead(tip, dir, color),
        ArrowStyle::SolidOpen | ArrowStyle::DashedOpen => {
            let base = (tip.0 - dir.0 * len, tip.1 - dir.1 * len);
            let width = canvas.stroke;
            canvas.polyline(
                &[
                    (base.0 + normal.0 * half, base.1 + normal.1 * half),
                    tip,
                    (base.0 - normal.0 * half, base.1 - normal.1 * half),
                ],
                color,
                width,
                Dash::Solid,
            );
        }
        ArrowStyle::SolidCross | ArrowStyle::DashedCross => {
            let c = (tip.0 - dir.0 * half, tip.1 - dir.1 * half);
            for side in [-1.0, 1.0] {
                canvas.line(
                    (
                        c.0 - half * (dir.0 - normal.0 * side),
                        c.1 - half * (dir.1 - normal.1 * side),
                    ),
                    (
                        c.0 + half * (dir.0 - normal.0 * side),
                        c.1 + half * (dir.1 - normal.1 * side),
                    ),
                    color,
                    Dash::Solid,
                );
            }
        }
    }
}

fn draw_message(
    canvas: &mut Canvas,
    msg: &PositionedMessage,
    layout: &SequenceLayout,
    palette: &Palette,
) {
    let (line_color, label_color) = message_colors(msg, palette);
    let left_x = msg.from_x.min(msg.to_x);
    let right_x = msg.from_x.max(msg.to_x);
    if !msg.label.is_empty() {
        let span = right_x.saturating_sub(left_x);
        let label_w = display_width(&msg.label);
        let label_x = if span >= label_w {
            left_x + (span - label_w) / 2
        } else {
            left_x
        };
        canvas.label(label_x, msg.y, &msg.label, label_color);
    }
    let row = msg.y + 1;
    let toward = if msg.to_x > msg.from_x { 1.0 } else { -1.0 };
    // Start and end on the activation box's side when one covers the
    // lifeline, as the ASCII arrow does.
    let edge = |x: usize, sign: f64| {
        let (cx, cy) = canvas.center((x, row));
        if activation_at(layout, x, row) {
            (cx + sign * canvas.cw, cy)
        } else {
            (cx, cy)
        }
    };
    let start = edge(msg.from_x, toward);
    let end = edge(msg.to_x, -toward);
    canvas.line(start, end, line_color, message_dash(&msg.arrow));
    message_head(canvas, end, (toward, 0.0), &msg.arrow, line_color);
}

fn draw_self_message(
    canvas: &mut Canvas,
    msg: &PositionedMessage,
    layout: &SequenceLayout,
    palette: &Palette,
) {
    let (line_color, label_color) = message_colors(msg, palette);
    let side = |row: usize| {
        let (cx, cy) = canvas.center((msg.from_x, row));
        if activation_at(layout, msg.from_x, row) {
            (cx + canvas.cw, cy)
        } else {
            (cx, cy)
        }
    };
    let start = side(msg.y);
    let end = side(msg.y + 2);
    let right = canvas
        .center((msg.from_x + 2, msg.y))
        .0
        .max(start.0 + canvas.cw);
    let width = canvas.stroke;
    canvas.polyline(
        &[start, (right, start.1), (right, end.1), end],
        line_color,
        width,
        message_dash(&msg.arrow),
    );
    message_head(canvas, end, (-1.0, 0.0), &msg.arrow, line_color);
    if !msg.label.is_empty() {
        canvas.label(msg.from_x + 4, msg.y, &msg.label, label_color);
    }
}

fn draw_note(canvas: &mut Canvas, note: &PositionedNote, palette: &Palette) {
    if note.width < 2 || note.height < 2 {
        return;
    }
    let rect = canvas.cell_box(note.x, note.y, note.width, note.height);
    canvas.shape_rect(
        rect,
        0.0,
        palette.edge_label,
        Some((palette.edge_label, 0.12)),
    );
    let text_w = display_width(&note.text);
    let text_x = if note.width > text_w + 2 {
        note.x + (note.width - text_w) / 2
    } else {
        note.x + 1
    };
    let text = clip(&note.text, note.width.saturating_sub(2));
    canvas.text(text_x, note.y + note.height / 2, text, palette.text);
}

fn draw_participant(canvas: &mut Canvas, p: &PositionedParticipant, palette: &Palette) {
    if p.width < 2 {
        return;
    }
    let (stroke, fill, text) = node_colors(p.style.as_ref(), palette);
    let rect = canvas.cell_box(p.x, p.y, p.width, PARTICIPANT_ROWS);
    canvas.shape_rect(rect, canvas.ch * 0.25, stroke, Some(fill));
    let label_x = p.x + 1 + (p.width - 2).saturating_sub(display_width(&p.label)) / 2;
    canvas.text(label_x, p.y + 1, &p.label, text);
}

// ---------------------------------------------------------------------------
// Rasterizing
// ---------------------------------------------------------------------------

/// A rasterized diagram: its size in pixels and the PNG data.
#[derive(Debug)]
pub(crate) struct Raster {
    pub(crate) px: (u32, u32),
    pub(crate) png: Vec<u8>,
}

/// The fonts SVG text is drawn with: the bundled font, plus the system's
/// for glyphs it lacks. Loaded once, on first use.
fn fonts() -> Arc<resvg::usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    Arc::clone(FONTS.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_font_data(FALLBACK_FONT.to_vec());
        db.load_system_fonts();
        db.set_monospace_family(FONT_FAMILY);
        Arc::new(db)
    }))
}

/// Rasterize `svg` to PNG at its own size.
pub(crate) fn rasterize(svg: &str) -> anyhow::Result<Raster> {
    let options = resvg::usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(svg, &options)?;
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| anyhow::anyhow!("empty diagram image"))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    Ok(Raster {
        px: (size.width(), size.height()),
        png: encode_png(&pixmap)?,
    })
}

/// PNG with fast compression: diagrams are mostly flat color, and encoding
/// at the default level takes as long as drawing them.
fn encode_png(pixmap: &resvg::tiny_skia::Pixmap) -> anyhow::Result<Vec<u8>> {
    use image::ImageEncoder as _;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let mut rgba = Vec::with_capacity(pixmap.data().len());
    for pixel in pixmap.pixels() {
        let c = pixel.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Adaptive)
        .write_image(
            &rgba,
            pixmap.width(),
            pixmap.height(),
            image::ExtendedColorType::Rgba8,
        )?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::render_diagram;

    const CELL: (u16, u16) = (10, 20);

    fn theme() -> &'static crate::theme::Theme {
        crate::theme::Theme::default_theme()
    }

    fn image(source: &str) -> DiagramImage {
        render_diagram(source, theme(), 120).unwrap().image
    }

    /// Every mermaid block in `docs/examples`, as (name, source).
    fn corpus() -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut paths: Vec<_> = std::fs::read_dir("docs/examples")
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "md"))
            .collect();
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path).unwrap();
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            for (i, block) in crate::parser::parse_markdown(&text).into_iter().enumerate() {
                if let crate::parser::Block::MermaidBlock { content } = block {
                    out.push((format!("{stem}-{i}"), content));
                }
            }
        }
        out
    }

    #[test]
    fn snapshot_flowchart() {
        let svg = image(
            "graph LR\n    A[Start] --> B(Rounded)\n    B -->|yes| C{Check?}\n    C -.-> D((Done))\n    C ==> E[Fail & retry]\n    subgraph S [Group]\n      D\n    end\n",
        )
        .to_svg(CELL);
        insta::assert_snapshot!(svg);
    }

    #[test]
    fn snapshot_flowchart_td_diamond() {
        let svg = image("graph TD\n    A[Request] --> B{Auth OK?}\n    B -->|No| C[Reject]\n    B -->|Yes| D[Serve]\n")
            .to_svg(CELL);
        insta::assert_snapshot!(svg);
    }

    #[test]
    fn snapshot_er() {
        let svg = image(
            "erDiagram\n    User ||--o{ Order : places\n    Order }|..|| Product : contains\n    User {\n      string id PK\n      string email\n    }\n    Order {\n      string id PK\n      string userId FK\n    }\n",
        )
        .to_svg(CELL);
        insta::assert_snapshot!(svg);
    }

    #[test]
    fn snapshot_sequence() {
        let svg = image(
            "sequenceDiagram\n    participant A as Alice\n    participant B as Bob\n    A->>B: Hello\n    activate B\n    B-->>A: Hi\n    B->>B: think\n    deactivate B\n    A->B: open\n    B--xA: lost\n    Note over A: a note\n    loop Every minute\n        A->>B: Ping\n    end\n    alt ok\n        B->>A: yes\n    else failed\n        B->>A: no\n    end\n",
        )
        .to_svg(CELL);
        insta::assert_snapshot!(svg);
    }

    #[test]
    fn image_covers_the_ascii_rectangle() {
        for (name, source) in corpus() {
            let Ok(diagram) = render_diagram(&source, theme(), 120) else {
                continue;
            };
            let cols = diagram
                .lines
                .iter()
                .map(|l| {
                    l.spans
                        .iter()
                        .map(|s| display_width(&s.text))
                        .sum::<usize>()
                })
                .max()
                .unwrap_or(0);
            assert_eq!(diagram.image.cells(), (cols, diagram.lines.len()), "{name}");
            let svg = diagram.image.to_svg(CELL);
            let size = format!(
                r#"width="{}" height="{}""#,
                cols * 10,
                diagram.lines.len() * 20
            );
            assert!(svg.contains(&size), "{name}: expected {size}");
        }
    }

    /// Corners of the pixel box `text` takes when it starts in cell
    /// `(col, row)`: its cells, at the height of capital letters.
    fn label_corners(canvas: &Canvas, (col, row): (usize, usize), text: &str) -> [Point; 4] {
        let left = canvas.left(col);
        let right = canvas.left(col + display_width(text));
        let mid = canvas.top(row) + canvas.ch / 2.0;
        let half = canvas.font_size * 0.4;
        [
            (left, mid - half),
            (right, mid - half),
            (right, mid + half),
            (left, mid + half),
        ]
    }

    /// Whether `p` is inside the convex polygon `outline` (either winding).
    fn inside(outline: &[Point], p: Point) -> bool {
        let mut positive = false;
        let mut negative = false;
        for (i, a) in outline.iter().enumerate() {
            let b = outline[(i + 1) % outline.len()];
            let cross = (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
            positive |= cross > 1e-9;
            negative |= cross < -1e-9;
        }
        !(positive && negative)
    }

    /// Whether `p` is inside the stadium (rectangle with half-circle ends)
    /// that fills `rect`, which is wider than it is tall.
    fn inside_stadium(rect: (f64, f64, f64, f64), p: Point) -> bool {
        let (left, top, width, height) = rect;
        let radius = height / 2.0;
        let spine_x = p.0.clamp(left + radius, left + width - radius);
        (p.0 - spine_x).hypot(p.1 - (top + radius)) <= radius
    }

    #[test]
    fn node_labels_stay_inside_their_shapes() {
        for (name, source) in corpus() {
            let Ok(diagram) = render_diagram(&source, theme(), 120) else {
                continue;
            };
            let Scene::Flowchart(layout) = &diagram.image.scene else {
                continue;
            };
            for cell in [(10, 20), (8, 17), (20, 40)] {
                let canvas = Canvas::new(cell, diagram.image.cols, diagram.image.rows);
                for node in &layout.nodes {
                    if node.shape == NodeShape::EntityBox || node.label.is_empty() {
                        continue;
                    }
                    let rect = canvas.cell_box(node.x, node.y, node.width, node.height);
                    let (x, y, w, h) = rect;
                    let outline = match node.shape {
                        NodeShape::Diamond => diamond_outline(&canvas, node, rect),
                        _ => vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)],
                    };
                    for corner in label_corners(&canvas, node_label_origin(node), &node.label) {
                        // Rounded corners (radius half a row) stay above
                        // and below the middle row, so the box is enough.
                        let fits = if node.shape == NodeShape::Circle {
                            inside_stadium(rect, corner)
                        } else {
                            inside(&outline, corner)
                        };
                        assert!(
                            fits,
                            "{name}: label {:?} leaves its {:?} at {cell:?}",
                            node.label, node.shape
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn edges_start_on_their_source_border() {
        let node = PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: NodeShape::Rect,
            x: 2,
            y: 2,
            width: 5,
            height: 3,
            compact: false,
            node_style: None,
            entity: None,
        };
        let nodes = [node];
        // Forward edge out of the right side: from the first outside cell
        // back onto the border.
        assert_eq!(edge_start(&[(7, 3), (12, 3)], &nodes), (6, 3));
        // Out of the bottom.
        assert_eq!(edge_start(&[(4, 5), (4, 9)], &nodes), (4, 4));
        // A back edge that already starts on the border stays there, so ER
        // marks are not drawn inside the box.
        assert_eq!(edge_start(&[(4, 2), (4, 0)], &nodes), (4, 2));
        // Nothing behind the start: unchanged.
        assert_eq!(edge_start(&[(10, 10), (12, 10)], &nodes), (10, 10));
    }

    #[test]
    fn text_runs_keep_their_cells() {
        let mut canvas = Canvas::new(CELL, 40, 2);
        canvas.text(3, 1, "ab  数据 x", (1, 2, 3));
        let runs: Vec<&str> = canvas.labels.lines().collect();
        assert_eq!(runs.len(), 3, "{}", canvas.labels);
        assert!(runs[0].starts_with(r#"<text x="30" y="35.76" textLength="20""#));
        assert!(runs[1].starts_with(r#"<text x="70" y="35.76" textLength="40""#));
        assert!(runs[2].starts_with(r#"<text x="120" y="35.76" textLength="10""#));
        assert!(runs[1].ends_with(">数据</text>"));
    }

    #[test]
    fn labels_are_escaped() {
        let mut canvas = Canvas::new(CELL, 40, 2);
        canvas.text(0, 0, "<a&b>\"\u{7}", (0, 0, 0));
        assert!(canvas.labels.contains(">&lt;a&amp;b&gt;&quot;</text>"));
    }

    #[test]
    fn large_diagrams_use_smaller_cells() {
        let canvas = Canvas::new((20, 40), 297, 10);
        assert!(canvas.width <= MAX_RASTER_PX + 1e-6);
        assert!(
            (canvas.cw / canvas.ch - 0.5).abs() < 1e-9,
            "keeps the cell aspect ratio"
        );
    }

    #[test]
    fn unknown_cell_size_uses_the_fallback() {
        let canvas = Canvas::new((0, 0), 3, 2);
        assert_eq!((canvas.width, canvas.height), (30.0, 40.0));
    }

    #[test]
    fn rasterizes_to_png_of_the_svg_size() {
        let svg = image("graph LR\n    A --> B\n").to_svg(CELL);
        let raster = rasterize(&svg).unwrap();
        let png = image::load_from_memory(&raster.png).unwrap();
        assert_eq!((png.width(), png.height()), raster.px);
        let (cols, rows) = image("graph LR\n    A --> B\n").cells();
        assert_eq!(
            raster.px,
            (
                u32::try_from(cols * 10).unwrap(),
                u32::try_from(rows * 20).unwrap()
            )
        );
    }

    #[test]
    fn invalid_svg_is_an_error() {
        assert!(rasterize("<svg").is_err());
        assert!(
            rasterize(r#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0"/>"#).is_err()
        );
    }

    #[test]
    fn source_key_tracks_source_width_and_theme() {
        let paper = crate::theme::Theme::by_name("paper").unwrap();
        let key = source_key("graph TD\nA-->B", 80, theme());
        assert_eq!(key, source_key("graph TD\nA-->B", 80, theme()));
        assert_ne!(key, source_key("graph TD\nA-->C", 80, theme()));
        assert_ne!(key, source_key("graph TD\nA-->B", 100, theme()));
        assert_ne!(key, source_key("graph TD\nA-->B", 80, paper));
    }

    /// Rasterization time per example diagram at 120 columns. Run with
    /// `cargo test --release -- --ignored --nocapture raster_timing`.
    #[test]
    #[ignore = "timing report, run on a release build"]
    fn raster_timing() {
        let _ = rasterize(&image("graph LR\n    A --> B\n").to_svg(CELL));
        for cell in [(10, 20), (16, 34)] {
            let mut times = Vec::new();
            for (name, source) in corpus() {
                let Ok(diagram) = render_diagram(&source, theme(), 120) else {
                    continue;
                };
                let svg = diagram.image.to_svg(cell);
                let mut runs: Vec<std::time::Duration> = (0..5)
                    .map(|_| {
                        let start = std::time::Instant::now();
                        rasterize(&svg).unwrap();
                        start.elapsed()
                    })
                    .collect();
                runs.sort();
                times.push((runs[2], name));
            }
            times.sort();
            for (t, name) in &times {
                eprintln!("{cell:?} {name}: {t:?}");
            }
        }
    }
}
