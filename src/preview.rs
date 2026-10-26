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

pub(crate) fn run() -> Result<()> {
    let width = crossterm::terminal::size().map_or(80, |(w, _)| w);
    let color_mode = if std::env::var("NO_COLOR").is_ok() {
        render::ColorMode::Plain
    } else {
        render::ColorMode::Ansi
    };
    let highlighter = Highlighter::new(None).map_err(|e| anyhow::anyhow!(e))?;
    let blocks = crate::parser::parse_markdown(SAMPLE_MARKDOWN);

    for theme in Theme::all() {
        if color_mode == render::ColorMode::Plain {
            println!("\n  -- {} --\n", theme.name);
        } else {
            println!("\n\x1b[1m  ── {} ──\x1b[0m\n", theme.name);
        }

        let rendered = render::render_blocks(
            &blocks,
            width,
            &highlighter,
            theme,
            render::MermaidMode::Render,
        );

        crate::pipe_output(&rendered, color_mode)?;
    }
    Ok(())
}
