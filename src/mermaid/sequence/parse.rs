use anyhow::bail;

use super::SequenceDiagram;

pub fn parse_sequence(input: &str) -> anyhow::Result<SequenceDiagram> {
    let mut lines = input.lines().peekable();

    let found_header = loop {
        match lines.next() {
            None => break false,
            Some(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with("%%") {
                    continue;
                }
                break trimmed == "sequenceDiagram";
            }
        }
    };

    if !found_header {
        bail!("Expected 'sequenceDiagram' declaration");
    }

    Ok(SequenceDiagram {
        participants: vec![],
        events: vec![],
        autonumber: false,
    })
}
