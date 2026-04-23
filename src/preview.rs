use anyhow::Result;

use crate::highlight::Highlighter;
use crate::render;
use crate::theme::Theme;

const SAMPLE_MARKDOWN: &str = "\
# Heading Level 1

Body text with **bold words** and *italic phrases* in a paragraph.

## Heading Level 2

A link: [example](https://example.com) and inline code: `fn main()`.

### Heading Level 3

#### Heading Level 4

##### Heading Level 5

###### Heading Level 6

---
";

pub fn run() -> Result<()> {
    let width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
    let no_color = std::env::var("NO_COLOR").is_ok();
    let highlighter = Highlighter::new(None).map_err(|e| anyhow::anyhow!(e))?;
    let blocks = crate::parser::parse_markdown(SAMPLE_MARKDOWN);

    for theme in Theme::all() {
        // Theme name header
        println!("\n\x1b[1m  ── {} ──\x1b[0m\n", theme.name);

        let rendered = render::render_blocks(
            &blocks,
            width,
            &highlighter,
            theme,
            render::MermaidMode::Render,
        );

        for block in &rendered {
            match block {
                render::RenderedBlock::Lines(lines) => {
                    for line in lines {
                        println!("{}", render::styled_line_to_ansi(line, no_color));
                    }
                }
                render::RenderedBlock::Diagram { lines, .. } => {
                    for line in lines {
                        println!("{}", render::styled_line_to_ansi(line, no_color));
                    }
                    println!();
                }
                render::RenderedBlock::Image { alt, url } => {
                    if alt.is_empty() {
                        println!("[Image]({})", url);
                    } else {
                        println!("[Image: {}]({})", alt, url);
                    }
                }
            }
        }
    }
    Ok(())
}
