mod embed;
mod highlight;
mod mermaid;
mod pager;
mod parser;
mod render;
mod self_update;
mod theme;
mod watch;

use anyhow::Result;
use clap::{CommandFactory, FromArgMatches, Parser};
use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;

const PAGER_KEYS_HELP: &str = "\
Pager keybindings:
  j/k, arrows        Scroll one line
  Space, PgDn/PgUp   Page down / up
  Ctrl-d / Ctrl-u     Half-page down / up
  Ctrl-f / Ctrl-b     Full page down / up
  g / G               Go to beginning / end
  h/l, Left/Right     Horizontal scroll
  /                   Forward search
  ?                   Backward search
  n / N               Next / previous match
  Tab / Shift-Tab     Cycle diagrams/images
  Enter               Expand/collapse diagram, open image
  q / Esc             Quit";

#[derive(Parser)]
#[command(
    name = "mdx",
    version,
    about = "Terminal markdown renderer with mermaid diagrams",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[command(flatten)]
    args: Args,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Update mdx to the latest version
    Update,
}

#[derive(clap::Args)]
struct Args {
    /// Markdown file to render
    file: Option<PathBuf>,

    /// Force pager mode even when piped
    #[arg(short, long)]
    pager: bool,

    /// Force plain output even on TTY
    #[arg(long)]
    no_pager: bool,

    /// Watch file for changes and re-render live
    #[arg(short = 'W', long)]
    watch: bool,

    /// Override terminal width for wrapping
    #[arg(short, long)]
    width: Option<u16>,

    /// Syntax highlighting theme [default: base16-ocean.dark]
    /// Examples: base16-eighties.dark, base16-mocha.dark, InspiredGitHub.
    /// Use --theme=list to see all available themes
    #[arg(long)]
    theme: Option<String>,

    /// UI color theme for headers, text, and chrome [default: clay]
    /// Use --ui-theme=list to see available themes
    #[arg(long)]
    ui_theme: Option<String>,

    /// Show raw mermaid source without rendering
    #[arg(long)]
    no_mermaid_rendering: bool,

    /// Show mermaid source followed by rendered diagram
    #[arg(long)]
    split_mermaid_rendering: bool,
}

fn read_input_from(file: Option<&std::path::Path>, stdin_is_terminal: bool) -> Result<String> {
    match file {
        Some(path) => Ok(std::fs::read_to_string(path)?),
        None if !stdin_is_terminal => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
        None => anyhow::bail!("No input: provide a file argument or pipe markdown to stdin"),
    }
}

fn resolve_width(override_width: Option<u16>) -> u16 {
    if let Some(w) = override_width {
        return w;
    }
    crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80)
}

fn resolve_mermaid_mode(no_mermaid: bool, split_mermaid: bool) -> render::MermaidMode {
    if no_mermaid {
        render::MermaidMode::Raw
    } else if split_mermaid {
        render::MermaidMode::Split
    } else {
        render::MermaidMode::Render
    }
}

fn resolve_ui_theme(name: Option<&str>) -> Result<&'static theme::Theme> {
    match name {
        Some(n) => theme::Theme::by_name(n).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown UI theme '{}'. Available: {}",
                n,
                theme::Theme::available_names().join(", ")
            )
        }),
        None => Ok(theme::Theme::default_theme()),
    }
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
                    writeln!(stdout, "{}", render::styled_line_to_ansi(line, no_color))?;
                }
                writeln!(stdout)?;
            }
            render::RenderedBlock::Image { alt, url } => {
                if alt.is_empty() {
                    writeln!(stdout, "[Image]({})", url)?;
                } else {
                    writeln!(stdout, "[Image: {}]({})", alt, url)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_watch_args(args: &Args) -> Result<()> {
    if !args.watch {
        return Ok(());
    }
    if args.file.is_none() {
        anyhow::bail!("--watch requires a file argument (cannot watch stdin)");
    }
    if args.no_pager {
        anyhow::bail!("--watch and --no-pager are incompatible");
    }
    Ok(())
}

fn setup_panic_hook() {
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
}

fn main() -> Result<()> {
    let cli = Cli::from_arg_matches(&Cli::command().after_help(PAGER_KEYS_HELP).get_matches())?;

    if let Some(Commands::Update) = cli.command {
        return self_update::run();
    }

    let args = cli.args;

    // Handle --theme=list before reading input
    if args.theme.as_deref() == Some("list") {
        let h = highlight::Highlighter::new(None).map_err(|e| anyhow::anyhow!(e))?;
        for name in h.available_themes() {
            println!("{}", name);
        }
        return Ok(());
    }

    // Handle --ui-theme=list before reading input
    if args.ui_theme.as_deref() == Some("list") {
        for name in theme::Theme::available_names() {
            println!("{}", name);
        }
        return Ok(());
    }

    // Validate watch args early
    validate_watch_args(&args)?;

    let width = resolve_width(args.width);
    let no_color = std::env::var("NO_COLOR").is_ok();
    let highlighter =
        highlight::Highlighter::new(args.theme.clone()).map_err(|e| anyhow::anyhow!(e))?;
    let ui_theme = resolve_ui_theme(args.ui_theme.as_deref())?;
    let mermaid_mode =
        resolve_mermaid_mode(args.no_mermaid_rendering, args.split_mermaid_rendering);

    // Watch mode — dispatch before reading input
    if args.watch {
        let path = args.file.as_ref().unwrap();
        setup_panic_hook();
        return watch::run_watch(path, width, &highlighter, ui_theme, mermaid_mode);
    }

    let input = read_input_from(args.file.as_deref(), std::io::stdin().is_terminal())?;
    let blocks = parser::parse_markdown(&input);
    let rendered = render::render_blocks(&blocks, width, &highlighter, ui_theme, mermaid_mode);
    if use_pager(&args) {
        setup_panic_hook();
        pager::run_pager(rendered, ui_theme)?;
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
        let cli = Cli::parse_from(["mdx", "README.md"]);
        assert!(cli.command.is_none());
        assert_eq!(cli.args.file, Some(PathBuf::from("README.md")));
        assert!(!cli.args.pager);
        assert!(!cli.args.no_pager);
        assert_eq!(cli.args.width, None);
    }

    #[test]
    fn test_args_parse_flags() {
        let cli = Cli::parse_from(["mdx", "-p", "-w", "80", "test.md"]);
        assert!(cli.args.pager);
        assert_eq!(cli.args.width, Some(80));
    }

    #[test]
    fn test_read_input_from_file() {
        let dir = std::env::temp_dir().join("mdx_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.md");
        std::fs::write(&path, "# Hello").unwrap();
        let input = read_input_from(Some(&path), true).unwrap();
        assert_eq!(input, "# Hello");
    }

    #[test]
    fn test_read_input_from_no_file_no_stdin_errors() {
        // TTY stdin with no file → must error
        assert!(read_input_from(None, true).is_err());
    }

    #[test]
    fn test_args_parse_ui_theme() {
        let cli = Cli::parse_from(["mdx", "--ui-theme", "hearth", "test.md"]);
        assert_eq!(cli.args.ui_theme, Some("hearth".to_string()));
    }

    #[test]
    fn test_args_ui_theme_default_is_none() {
        let cli = Cli::parse_from(["mdx", "test.md"]);
        assert_eq!(cli.args.ui_theme, None);
    }

    #[test]
    fn test_args_no_mermaid_rendering() {
        let cli = Cli::parse_from(["mdx", "--no-mermaid-rendering", "test.md"]);
        assert!(cli.args.no_mermaid_rendering);
        assert!(!cli.args.split_mermaid_rendering);
    }

    #[test]
    fn test_args_split_mermaid_rendering() {
        let cli = Cli::parse_from(["mdx", "--split-mermaid-rendering", "test.md"]);
        assert!(cli.args.split_mermaid_rendering);
        assert!(!cli.args.no_mermaid_rendering);
    }

    #[test]
    fn test_args_watch_flag() {
        let cli = Cli::parse_from(["mdx", "--watch", "file.md"]);
        assert!(cli.args.watch);
    }

    #[test]
    fn test_args_watch_short_flag() {
        let cli = Cli::parse_from(["mdx", "-W", "file.md"]);
        assert!(cli.args.watch);
    }

    #[test]
    fn test_args_watch_default_false() {
        let cli = Cli::parse_from(["mdx", "file.md"]);
        assert!(!cli.args.watch);
    }

    #[test]
    fn test_update_subcommand() {
        let cli = Cli::parse_from(["mdx", "update"]);
        assert!(matches!(cli.command, Some(Commands::Update)));
    }

    #[test]
    fn test_watch_requires_file() {
        let args = Args {
            file: None,
            pager: false,
            no_pager: false,
            watch: true,
            width: None,
            theme: None,
            ui_theme: None,
            no_mermaid_rendering: false,
            split_mermaid_rendering: false,
        };
        let result = validate_watch_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires a file"));
    }

    #[test]
    fn test_watch_conflicts_with_no_pager() {
        let args = Args {
            file: Some(PathBuf::from("test.md")),
            pager: false,
            no_pager: true,
            watch: true,
            width: None,
            theme: None,
            ui_theme: None,
            no_mermaid_rendering: false,
            split_mermaid_rendering: false,
        };
        let result = validate_watch_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("incompatible"));
    }

    #[test]
    fn test_watch_valid_args() {
        let args = Args {
            file: Some(PathBuf::from("test.md")),
            pager: false,
            no_pager: false,
            watch: true,
            width: None,
            theme: None,
            ui_theme: None,
            no_mermaid_rendering: false,
            split_mermaid_rendering: false,
        };
        let result = validate_watch_args(&args);
        assert!(result.is_ok());
    }
}
