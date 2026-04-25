use super::ErDiagram;
use anyhow::Result;

pub fn parse_er(_content: &str) -> Result<ErDiagram> {
    anyhow::bail!("ER parsing not implemented")
}

#[cfg(test)]
mod tests {}
