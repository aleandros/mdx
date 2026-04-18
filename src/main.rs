mod highlight;
mod mermaid;
mod pager;
mod parser;
mod render;
mod theme;

use anyhow::Result;
use clap::Parser;
use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "mdx",
    version,
    about = "Terminal markdown renderer with mermaid diagrams"
)]
struct Args {
    /// Markdown file to render
    file: Option<PathBuf>,

    /// Force pager mode even when piped
    #[arg(short, long)]
    pager: bool,

    /// Force plain output even on TTY
    #[arg(long)]
    no_pager: bool,

    /// Override terminal width for wrapping
    #[arg(short, long)]
    width: Option<u16>,

    /// Syntax highlighting theme [default: base16-ocean.dark]
    /// Examples: base16-eighties.dark, base16-mocha.dark, InspiredGitHub.
    /// Use --theme=list to see all available themes
    #[arg(long)]
    theme: Option<String>,
}

fn read_input(args: &Args) -> Result<String> {
    read_input_with_tty_check(args, std::io::stdin().is_terminal())
}

fn read_input_with_tty_check(args: &Args, stdin_is_terminal: bool) -> Result<String> {
    match &args.file {
        Some(path) => Ok(std::fs::read_to_string(path)?),
        None if !stdin_is_terminal => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
        None => {
            anyhow::bail!("No input: provide a file argument or pipe markdown to stdin");
        }
    }
}

fn get_width(args: &Args) -> u16 {
    if let Some(w) = args.width {
        return w;
    }
    crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80)
}

fn use_pager(args: &Args) -> bool {
    if args.no_pager {
        return false;
    }
    if args.pager {
        return true;
    }
    std::io::stdout().is_terminal()
}

fn pipe_output(blocks: &[render::RenderedBlock], no_color: bool) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    for block in blocks {
        match block {
            render::RenderedBlock::Lines(lines) => {
                for line in lines {
                    writeln!(stdout, "{}", render::styled_line_to_ansi(line, no_color))?;
                }
            }
            render::RenderedBlock::Diagram { lines, .. } => {
                for line in lines {
                    writeln!(stdout, "{}", line)?;
                }
                writeln!(stdout)?;
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Handle --theme=list before reading input
    if args.theme.as_deref() == Some("list") {
        let h = highlight::Highlighter::new(None).map_err(|e| anyhow::anyhow!(e))?;
        for name in h.available_themes() {
            println!("{}", name);
        }
        return Ok(());
    }

    let input = read_input(&args)?;
    let width = get_width(&args);
    let no_color = std::env::var("NO_COLOR").is_ok();
    let highlighter =
        highlight::Highlighter::new(args.theme.clone()).map_err(|e| anyhow::anyhow!(e))?;
    let blocks = parser::parse_markdown(&input);
    let rendered = render::render_blocks(&blocks, width, &highlighter);
    if use_pager(&args) {
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = crossterm::terminal::disable_raw_mode();
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::LeaveAlternateScreen,
                crossterm::event::DisableMouseCapture
            );
            original_hook(info);
        }));
        pager::run_pager(rendered)?;
    } else {
        pipe_output(&rendered, no_color)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parse_file() {
        let args = Args::parse_from(["mdx", "README.md"]);
        assert_eq!(args.file, Some(PathBuf::from("README.md")));
        assert!(!args.pager);
        assert!(!args.no_pager);
        assert_eq!(args.width, None);
    }

    #[test]
    fn test_args_parse_flags() {
        let args = Args::parse_from(["mdx", "-p", "-w", "80", "test.md"]);
        assert!(args.pager);
        assert_eq!(args.width, Some(80));
    }

    #[test]
    fn test_read_input_file() {
        let dir = std::env::temp_dir().join("mdx_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.md");
        std::fs::write(&path, "# Hello").unwrap();
        let args = Args {
            file: Some(path),
            pager: false,
            no_pager: false,
            width: None,
            theme: None,
        };
        let input = read_input(&args).unwrap();
        assert_eq!(input, "# Hello");
    }

    #[test]
    fn test_read_input_no_file_no_stdin() {
        let args = Args {
            file: None,
            pager: false,
            no_pager: false,
            width: None,
            theme: None,
        };
        // Simulate TTY context: stdin is a terminal, no file provided → must error
        assert!(read_input_with_tty_check(&args, true).is_err());
    }
}
