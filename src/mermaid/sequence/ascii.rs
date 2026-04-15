use super::layout::{PositionedFragment, SequenceLayout};
use super::ArrowStyle;
use crate::mermaid::ascii::Canvas;

pub fn render(layout: &SequenceLayout) -> Vec<String> {
    if layout.width == 0 || layout.height == 0 {
        return vec![];
    }

    // Extra width to accommodate self-message labels that extend past the diagram width
    let self_msg_extra: usize = layout
        .messages
        .iter()
        .filter(|m| m.self_message && !m.label.is_empty())
        .map(|m| {
            // Label starts at from_x + 4; we need (from_x + 4 + label.len()) - layout.width
            let needed = m.from_x + 4 + m.label.len();
            needed.saturating_sub(layout.width)
        })
        .max()
        .unwrap_or(0);

    // Extra width to accommodate notes that extend past the diagram width
    let note_extra: usize = layout
        .notes
        .iter()
        .map(|n| {
            let right_edge = n.x + n.width;
            right_edge.saturating_sub(layout.width)
        })
        .max()
        .unwrap_or(0);

    let canvas_width = layout.width + 4 + self_msg_extra.max(note_extra);
    let mut canvas = Canvas::new(canvas_width, layout.height + 2);

    // 1. Draw lifelines (dashed vertical under each participant box)
    draw_lifelines(&mut canvas, layout);

    // 2. Draw fragment boxes (before messages so messages appear on top)
    draw_fragments(&mut canvas, layout);

    // 3. Draw messages
    draw_messages(&mut canvas, layout);

    // 4. Draw activations (after messages so activation borders appear on top of arrows)
    draw_activations(&mut canvas, layout);

    // 5. Draw notes
    draw_notes(&mut canvas, layout);

    // 6. Draw participant boxes (last, on top)
    draw_participants(&mut canvas, layout);

    let mut lines = canvas.to_lines();
    while lines.last().map(|l: &String| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines
}

// ---------------------------------------------------------------------------
// Lifelines
// ---------------------------------------------------------------------------

fn draw_lifelines(canvas: &mut Canvas, layout: &SequenceLayout) {
    let lifeline_start = 3; // below participant box (height=3, rows 0..2)
    let lifeline_end = layout.height;

    for p in &layout.participants {
        let x = p.center_x;
        for y in lifeline_start..lifeline_end {
            // Dashed: alternate │ and space
            let ch = if y % 2 == 1 { '│' } else { '╎' };
            canvas.set(x, y, ch);
        }
    }
}

// ---------------------------------------------------------------------------
// Activations
// ---------------------------------------------------------------------------

fn draw_activations(canvas: &mut Canvas, layout: &SequenceLayout) {
    for act in &layout.activations {
        let cx = act.x;
        // 3 chars wide, centered: cx-1, cx, cx+1
        let left = cx.saturating_sub(1);
        let right = cx + 1;

        let y_start = act.y_start;
        let y_end = act.y_end;

        if y_start >= y_end {
            continue;
        }

        // Clear the area first (overwrite lifeline chars)
        for y in y_start..y_end {
            canvas.set(left, y, ' ');
            canvas.set(cx, y, ' ');
            canvas.set(right, y, ' ');
        }

        // Top border
        canvas.set(left, y_start, '┌');
        canvas.set(cx, y_start, '─');
        canvas.set(right, y_start, '┐');

        // Body
        for y in (y_start + 1)..y_end.saturating_sub(1) {
            canvas.set(left, y, '│');
            canvas.set(cx, y, ' ');
            canvas.set(right, y, '│');
        }

        // Bottom border (only if there's more than 1 row)
        if y_end > y_start + 1 {
            canvas.set(left, y_end - 1, '└');
            canvas.set(cx, y_end - 1, '─');
            canvas.set(right, y_end - 1, '┘');
        }
    }
}

// ---------------------------------------------------------------------------
// Fragment boxes
// ---------------------------------------------------------------------------

fn draw_fragments(canvas: &mut Canvas, layout: &SequenceLayout) {
    // Draw in reverse order so that inner (nested) fragments, which appear earlier
    // in the list, are drawn last (on top of outer fragment borders).
    for frag in layout.fragments.iter().rev() {
        draw_fragment(canvas, frag);
    }
}

fn draw_fragment(canvas: &mut Canvas, frag: &PositionedFragment) {
    let x = frag.x;
    let y = frag.y;
    let w = frag.width;
    let h = frag.height;

    if w < 2 || h < 2 {
        return;
    }

    // Build header string: ┌─ {kind} [{label}] ─...─┐
    let kind_str = fragment_kind_str(&frag.kind);
    let header_label = if frag.label.is_empty() {
        kind_str.to_string()
    } else {
        format!("{} [{}]", kind_str, frag.label)
    };

    // Top border row
    let top_inner = format!("─ {} ", header_label);
    canvas.set(x, y, '┌');
    // Write "─ {kind} [{label}] "
    let mut col = x + 1;
    for ch in top_inner.chars() {
        if col < x + w - 1 {
            canvas.set(col, y, ch);
            col += 1;
        }
    }
    // Fill remainder with ─
    while col < x + w - 1 {
        canvas.set(col, y, '─');
        col += 1;
    }
    canvas.set(x + w - 1, y, '┐');

    // Left and right borders
    for row in 1..h - 1 {
        canvas.set(x, y + row, '│');
        canvas.set(x + w - 1, y + row, '│');
    }

    // Section dividers
    for (div_y, div_label) in &frag.section_dividers {
        let dy = *div_y;
        if dy < y || dy >= y + h {
            continue;
        }
        canvas.set(x, dy, '├');
        let div_inner = if let Some(lbl) = div_label {
            format!("─ [{}] ", lbl)
        } else {
            "─".to_string()
        };
        let mut col = x + 1;
        for ch in div_inner.chars() {
            if col < x + w - 1 {
                canvas.set(col, dy, ch);
                col += 1;
            }
        }
        while col < x + w - 1 {
            canvas.set(col, dy, '─');
            col += 1;
        }
        canvas.set(x + w - 1, dy, '┤');
    }

    // Bottom border
    canvas.set(x, y + h - 1, '└');
    for i in 1..w - 1 {
        canvas.set(x + i, y + h - 1, '─');
    }
    canvas.set(x + w - 1, y + h - 1, '┘');
}

fn fragment_kind_str(kind: &super::FragmentKind) -> &'static str {
    match kind {
        super::FragmentKind::Loop => "loop",
        super::FragmentKind::Alt => "alt",
        super::FragmentKind::Opt => "opt",
        super::FragmentKind::Par => "par",
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

fn draw_messages(canvas: &mut Canvas, layout: &SequenceLayout) {
    for msg in &layout.messages {
        if msg.self_message {
            draw_self_message(canvas, msg);
        } else {
            draw_message(canvas, msg);
        }
    }
}

fn draw_message(canvas: &mut Canvas, msg: &super::layout::PositionedMessage) {
    let y = msg.y;
    let label_row = y;
    let arrow_row = y + 1;

    let left_x = msg.from_x.min(msg.to_x);
    let right_x = msg.from_x.max(msg.to_x);

    // Center label above arrow
    if !msg.label.is_empty() {
        let span = right_x.saturating_sub(left_x);
        let label_x = if span >= msg.label.len() {
            left_x + (span - msg.label.len()) / 2
        } else {
            left_x
        };
        canvas.draw_text(label_x, label_row, &msg.label);
    }

    // Arrow line: between the two participants
    let going_right = msg.from_x < msg.to_x;
    // For left-to-right: line goes from from_x+1 to to_x-1
    // For right-to-left: line goes from to_x+1 to from_x-1
    let (arrow_start, arrow_end) = if going_right {
        (msg.from_x + 1, msg.to_x.saturating_sub(1))
    } else {
        (msg.to_x + 1, msg.from_x.saturating_sub(1))
    };

    if arrow_start > arrow_end {
        return;
    }

    let (line_char, is_dashed) = arrow_line_chars(&msg.arrow);

    // Draw line body
    let mut col = arrow_start;
    while col <= arrow_end {
        let ch = if is_dashed {
            if col % 2 == 0 { line_char } else { ' ' }
        } else {
            line_char
        };
        canvas.set(col, arrow_row, ch);
        col += 1;
    }

    // Draw arrowhead
    if going_right {
        // Head at to_x - 1 and to_x (two chars for >> style)
        let head = arrow_head_right(&msg.arrow);
        let head_chars: Vec<char> = head.chars().collect();
        let head_len = head_chars.len();
        let head_start = msg.to_x.saturating_sub(head_len);
        for (i, &hc) in head_chars.iter().enumerate() {
            canvas.set(head_start + i, arrow_row, hc);
        }
    } else {
        // Head at from_x (left side)
        let head = arrow_head_left(&msg.arrow);
        let head_chars: Vec<char> = head.chars().collect();
        for (i, &hc) in head_chars.iter().enumerate() {
            canvas.set(msg.to_x + i, arrow_row, hc);
        }
    }
}

fn draw_self_message(canvas: &mut Canvas, msg: &super::layout::PositionedMessage) {
    let x = msg.from_x;
    let y = msg.y;

    // Row 0: ──┐ label
    canvas.set(x, y, '─');
    canvas.set(x + 1, y, '─');
    canvas.set(x + 2, y, '┐');
    if !msg.label.is_empty() {
        canvas.draw_text(x + 4, y, &msg.label);
    }

    // Row 1: vertical │
    canvas.set(x + 2, y + 1, '│');

    // Row 2: <─┘
    canvas.set(x, y + 2, '<');
    canvas.set(x + 1, y + 2, '─');
    canvas.set(x + 2, y + 2, '┘');
}

fn arrow_line_chars(arrow: &ArrowStyle) -> (char, bool) {
    match arrow {
        ArrowStyle::SolidArrow | ArrowStyle::SolidOpen | ArrowStyle::SolidCross => ('─', false),
        ArrowStyle::DashedArrow | ArrowStyle::DashedOpen | ArrowStyle::DashedCross => ('─', true),
    }
}

fn arrow_head_right(arrow: &ArrowStyle) -> &'static str {
    match arrow {
        ArrowStyle::SolidArrow | ArrowStyle::DashedArrow => ">>",
        ArrowStyle::SolidOpen | ArrowStyle::DashedOpen => ">",
        ArrowStyle::SolidCross | ArrowStyle::DashedCross => "x",
    }
}

fn arrow_head_left(arrow: &ArrowStyle) -> &'static str {
    match arrow {
        ArrowStyle::SolidArrow | ArrowStyle::DashedArrow => "<<",
        ArrowStyle::SolidOpen | ArrowStyle::DashedOpen => "<",
        ArrowStyle::SolidCross | ArrowStyle::DashedCross => "x",
    }
}

// ---------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------

fn draw_notes(canvas: &mut Canvas, layout: &SequenceLayout) {
    for note in &layout.notes {
        draw_note(canvas, note);
    }
}

fn draw_note(canvas: &mut Canvas, note: &super::layout::PositionedNote) {
    let x = note.x;
    let y = note.y;
    let w = note.width;
    let h = note.height;

    if w < 2 || h < 2 {
        return;
    }

    // Clear area first
    for row in 0..h {
        for col in 0..w {
            canvas.set(x + col, y + row, ' ');
        }
    }

    // Top border
    canvas.set(x, y, '┌');
    for i in 1..w - 1 {
        canvas.set(x + i, y, '─');
    }
    canvas.set(x + w - 1, y, '┐');

    // Side borders
    for row in 1..h - 1 {
        canvas.set(x, y + row, '│');
        canvas.set(x + w - 1, y + row, '│');
    }

    // Bottom border
    canvas.set(x, y + h - 1, '└');
    for i in 1..w - 1 {
        canvas.set(x + i, y + h - 1, '─');
    }
    canvas.set(x + w - 1, y + h - 1, '┘');

    // Text centered on middle row
    let mid_row = h / 2;
    let text_x = if w > note.text.len() + 2 {
        x + (w - note.text.len()) / 2
    } else {
        x + 1
    };
    canvas.draw_text(text_x, y + mid_row, &note.text);
}

// ---------------------------------------------------------------------------
// Participant boxes
// ---------------------------------------------------------------------------

fn draw_participants(canvas: &mut Canvas, layout: &SequenceLayout) {
    for p in &layout.participants {
        draw_participant_box(canvas, p);
    }
}

fn draw_participant_box(canvas: &mut Canvas, p: &super::layout::PositionedParticipant) {
    let x = p.x;
    let y = p.y; // always 0
    let w = p.width;
    let h = 3;

    // Clear area first (overwrite any lifeline chars)
    for row in 0..h {
        for col in 0..w {
            canvas.set(x + col, y + row, ' ');
        }
    }

    // Top border
    canvas.set(x, y, '┌');
    for i in 1..w - 1 {
        canvas.set(x + i, y, '─');
    }
    canvas.set(x + w - 1, y, '┐');

    // Middle row with label
    canvas.set(x, y + 1, '│');
    canvas.set(x + w - 1, y + 1, '│');
    // Center the label with 1 space padding on each side
    let label_x = x + 1 + (w - 2 - p.label.len()) / 2;
    canvas.draw_text(label_x, y + 1, &p.label);

    // Bottom border
    canvas.set(x, y + 2, '└');
    for i in 1..w - 1 {
        canvas.set(x + i, y + 2, '─');
    }
    canvas.set(x + w - 1, y + 2, '┘');
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::sequence::{layout::layout as seq_layout, parse::parse_sequence};

    fn render_fixture(input: &str) -> String {
        let diagram = parse_sequence(input).unwrap();
        let laid_out = seq_layout(&diagram);
        let lines = render(&laid_out);
        lines.join("\n")
    }

    #[test]
    fn test_basic_render_has_participants() {
        let output = render_fixture(
            "sequenceDiagram\n    participant Alice\n    participant Bob\n    Alice->>Bob: Hello\n",
        );
        assert!(output.contains("Alice"), "Should contain participant Alice");
        assert!(output.contains("Bob"), "Should contain participant Bob");
    }

    #[test]
    fn test_basic_render_has_message_label() {
        let output = render_fixture(
            "sequenceDiagram\n    participant Alice\n    participant Bob\n    Alice->>Bob: Hello\n",
        );
        assert!(output.contains("Hello"), "Should contain message label");
    }

    #[test]
    fn test_render_has_arrow_chars() {
        let output = render_fixture(
            "sequenceDiagram\n    participant A\n    participant B\n    A->>B: Test\n",
        );
        assert!(
            output.contains(">>") || output.contains("─"),
            "Should contain arrow characters, got:\n{}",
            output
        );
    }

    #[test]
    fn test_render_has_lifelines() {
        let output = render_fixture(
            "sequenceDiagram\n    participant A\n    participant B\n    A->>B: Test\n",
        );
        let lines: Vec<&str> = output.lines().collect();
        // Lines below participant box (row 3+) should have │ for lifelines
        let has_lifeline = lines.iter().skip(3).any(|l| l.contains('│'));
        assert!(
            has_lifeline,
            "Should contain lifeline characters below participant boxes"
        );
    }

    #[test]
    fn test_render_self_message() {
        let output = render_fixture(
            "sequenceDiagram\n    participant A\n    A->>A: Think\n",
        );
        assert!(output.contains("Think"), "Should contain self-message label");
        assert!(output.contains("┐"), "Should contain self-message corner");
        assert!(output.contains("┘"), "Should contain self-message return corner");
    }

    #[test]
    fn test_render_note() {
        let output = render_fixture(
            "sequenceDiagram\n    participant A\n    Note over A: Hello\n",
        );
        assert!(output.contains("Hello"), "Should contain note text");
    }

    #[test]
    fn test_render_fragment() {
        let output = render_fixture(
            "\
sequenceDiagram
    participant A
    participant B
    loop Every minute
        A->>B: Ping
    end
",
        );
        assert!(output.contains("loop"), "Should contain fragment kind");
        assert!(
            output.contains("Every minute"),
            "Should contain fragment label"
        );
    }
}
