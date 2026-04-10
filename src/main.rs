mod mermaid;
mod parser;

use anyhow::Result;
use clap::Parser;
use std::io::{IsTerminal, Read};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mdx", version, about = "Terminal markdown renderer with mermaid diagrams")]
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

fn main() -> Result<()> {
    let args = Args::parse();
    let input = read_input(&args)?;
    println!("Read {} bytes", input.len());
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
        let args = Args { file: Some(path), pager: false, no_pager: false, width: None };
        let input = read_input(&args).unwrap();
        assert_eq!(input, "# Hello");
    }

    #[test]
    fn test_read_input_no_file_no_stdin() {
        let args = Args { file: None, pager: false, no_pager: false, width: None };
        // Simulate TTY context: stdin is a terminal, no file provided → must error
        assert!(read_input_with_tty_check(&args, true).is_err());
    }
}
