use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use notify::{Config, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};

use crate::render::MermaidMode;

#[allow(dead_code)]
pub fn content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[allow(dead_code)]
fn start_watcher(path: &Path) -> Result<(Box<dyn Watcher + Send>, mpsc::Receiver<()>)> {
    let (tx, rx) = mpsc::channel();

    let tx1 = tx.clone();
    let result = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if res.is_ok() {
                let _ = tx1.send(());
            }
        },
        Config::default(),
    );

    let mut watcher: Box<dyn Watcher + Send> = match result {
        Ok(w) => Box::new(w),
        Err(_) => {
            let config = Config::default().with_poll_interval(Duration::from_millis(500));
            Box::new(PollWatcher::new(
                move |res: notify::Result<notify::Event>| {
                    if res.is_ok() {
                        let _ = tx.send(());
                    }
                },
                config,
            )?)
        }
    };

    watcher.watch(path, RecursiveMode::NonRecursive)?;
    Ok((watcher, rx))
}

#[allow(dead_code)]
fn read_file_with_retry(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => Some(content),
        Err(_) => {
            std::thread::sleep(Duration::from_millis(50));
            std::fs::read_to_string(path).ok()
        }
    }
}

pub fn run_watch(
    _path: &Path,
    _width: u16,
    _highlighter: &crate::highlight::Highlighter,
    _theme: &'static crate::theme::Theme,
    _mermaid_mode: MermaidMode,
) -> Result<()> {
    anyhow::bail!("watch mode not yet implemented")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_content_hash_differs_for_different_content() {
        let h1 = content_hash("hello");
        let h2 = content_hash("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_content_hash_empty() {
        let h1 = content_hash("");
        let h2 = content_hash("x");
        assert_ne!(h1, h2);
    }
}
