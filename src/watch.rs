// Watch mode: live-preview with file watching and block-level diffing.

use crate::render::MermaidMode;
use anyhow::Result;
use std::path::Path;

pub fn run_watch(
    _path: &Path,
    _width: u16,
    _highlighter: &crate::highlight::Highlighter,
    _theme: &'static crate::theme::Theme,
    _mermaid_mode: MermaidMode,
) -> Result<()> {
    anyhow::bail!("watch mode not yet implemented")
}
