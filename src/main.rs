mod config;
mod embed;
mod highlight;
mod mermaid;
mod pager;
mod parser;
mod preview;
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
    /// Render markdown into a bounded ANSI stream for embedding in other TUIs
    Embed(EmbedArgs),
    /// Preview all available UI themes with sample markdown
    PreviewThemes,
    /// Generate a default user config file at ~/.config/mdx/config.toml
    Init,
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

    /// Path to a config file (overrides user and project config)
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Show raw mermaid source without rendering
    #[arg(long)]
    no_mermaid_rendering: bool,

    /// Show mermaid source followed by rendered diagram
    #[arg(long)]
    split_mermaid_rendering: bool,
}

#[derive(clap::Args)]
struct EmbedArgs {
    /// Markdown file to render (stdin if omitted)
    file: Option<PathBuf>,

    /// Output width in columns; each line is cropped to fit
    #[arg(short, long)]
    width: Option<u16>,

    /// Maximum number of output lines
    #[arg(long)]
    height: Option<usize>,

    /// Syntax highlighting theme (use `list` to see options)
    #[arg(long)]
    theme: Option<String>,

    /// UI color theme (use `list` to see options)
    #[arg(long)]
    ui_theme: Option<String>,

    /// Path to a config file (overrides user and project config)
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

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

pub(crate) fn pipe_output(blocks: &[render::RenderedBlock], no_color: bool) -> Result<()> {
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

fn run_init() -> Result<()> {
    let path = config::Config::user_config_path()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    if path.exists() {
        anyhow::bail!("Config file already exists: {}", path.display());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, config::Config::generate_default())?;
    println!("Config file created: {}", path.display());
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::from_arg_matches(&Cli::command().after_help(PAGER_KEYS_HELP).get_matches())?;

    match cli.command {
        Some(Commands::Update) => return self_update::run(),
        Some(Commands::Embed(eargs)) => return run_embed(eargs),
        Some(Commands::PreviewThemes) => return preview::run(),
        Some(Commands::Init) => return run_init(),
        None => {}
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

fn run_embed(eargs: EmbedArgs) -> Result<()> {
    // theme=list / ui-theme=list short-circuits
    if eargs.theme.as_deref() == Some("list") {
        let h = highlight::Highlighter::new(None).map_err(|e| anyhow::anyhow!(e))?;
        for name in h.available_themes() {
            println!("{}", name);
        }
        return Ok(());
    }
    if eargs.ui_theme.as_deref() == Some("list") {
        for name in theme::Theme::available_names() {
            println!("{}", name);
        }
        return Ok(());
    }

    let width = resolve_width(eargs.width);
    let no_color = std::env::var("NO_COLOR").is_ok();
    let highlighter =
        highlight::Highlighter::new(eargs.theme.clone()).map_err(|e| anyhow::anyhow!(e))?;
    let ui_theme = resolve_ui_theme(eargs.ui_theme.as_deref())?;
    let mermaid_mode =
        resolve_mermaid_mode(eargs.no_mermaid_rendering, eargs.split_mermaid_rendering);

    let input = read_input_from(eargs.file.as_deref(), std::io::stdin().is_terminal())?;
    let opts = embed::EmbedOptions {
        width,
        height: eargs.height,
        no_color,
    };
    let mut stdout = std::io::stdout().lock();
    embed::run(
        &input,
        opts,
        &highlighter,
        ui_theme,
        mermaid_mode,
        &mut stdout,
    )
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
            config: None,
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
            config: None,
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
            config: None,
            no_mermaid_rendering: false,
            split_mermaid_rendering: false,
        };
        let result = validate_watch_args(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_embed_subcommand_basic() {
        let cli = Cli::parse_from(["mdx", "embed", "file.md"]);
        match cli.command {
            Some(Commands::Embed(args)) => {
                assert_eq!(args.file, Some(PathBuf::from("file.md")));
                assert_eq!(args.width, None);
                assert_eq!(args.height, None);
            }
            _ => panic!("expected Embed subcommand"),
        }
    }

    #[test]
    fn test_embed_subcommand_width_height() {
        let cli = Cli::parse_from(["mdx", "embed", "--width", "40", "--height", "10", "file.md"]);
        match cli.command {
            Some(Commands::Embed(args)) => {
                assert_eq!(args.width, Some(40));
                assert_eq!(args.height, Some(10));
            }
            _ => panic!("expected Embed subcommand"),
        }
    }

    #[test]
    fn test_embed_subcommand_rejects_pager_flag() {
        let result = Cli::try_parse_from(["mdx", "embed", "--pager", "file.md"]);
        assert!(result.is_err(), "embed must not accept --pager");
    }

    #[test]
    fn test_embed_subcommand_rejects_watch_flag() {
        let result = Cli::try_parse_from(["mdx", "embed", "--watch", "file.md"]);
        assert!(result.is_err(), "embed must not accept --watch");
    }

    #[test]
    fn test_init_subcommand() {
        let cli = Cli::parse_from(["mdx", "init"]);
        assert!(matches!(cli.command, Some(Commands::Init)));
    }

    #[test]
    fn test_config_flag() {
        let cli = Cli::parse_from(["mdx", "--config", "/tmp/my.toml", "test.md"]);
        assert_eq!(cli.args.config, Some(PathBuf::from("/tmp/my.toml")));
    }

    #[test]
    fn test_embed_config_flag() {
        let cli = Cli::parse_from(["mdx", "embed", "--config", "/tmp/my.toml", "file.md"]);
        match cli.command {
            Some(Commands::Embed(args)) => {
                assert_eq!(args.config, Some(PathBuf::from("/tmp/my.toml")));
            }
            _ => panic!("expected Embed subcommand"),
        }
    }
}
