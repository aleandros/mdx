#![allow(dead_code)]

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub theme: Option<String>,
    pub ui_theme: Option<String>,
    pub pager: Option<bool>,
    pub width: Option<u16>,
    pub no_mermaid_rendering: Option<bool>,
    pub split_mermaid_rendering: Option<bool>,
}

impl Config {
    pub fn merge(self, other: Config) -> Config {
        Config {
            theme: other.theme.or(self.theme),
            ui_theme: other.ui_theme.or(self.ui_theme),
            pager: other.pager.or(self.pager),
            width: other.width.or(self.width),
            no_mermaid_rendering: other.no_mermaid_rendering.or(self.no_mermaid_rendering),
            split_mermaid_rendering: other
                .split_mermaid_rendering
                .or(self.split_mermaid_rendering),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_other_some_overrides_self_none() {
        let base = Config::default();
        let overlay = Config {
            theme: Some("nord".to_string()),
            ..Config::default()
        };
        let merged = base.merge(overlay);
        assert_eq!(merged.theme.as_deref(), Some("nord"));
    }

    #[test]
    fn merge_other_some_overrides_self_some() {
        let base = Config {
            ui_theme: Some("clay".to_string()),
            ..Config::default()
        };
        let overlay = Config {
            ui_theme: Some("frost".to_string()),
            ..Config::default()
        };
        let merged = base.merge(overlay);
        assert_eq!(merged.ui_theme.as_deref(), Some("frost"));
    }

    #[test]
    fn merge_other_none_preserves_self_some() {
        let base = Config {
            pager: Some(true),
            width: Some(120),
            ..Config::default()
        };
        let overlay = Config::default();
        let merged = base.merge(overlay);
        assert_eq!(merged.pager, Some(true));
        assert_eq!(merged.width, Some(120));
    }

    #[test]
    fn merge_both_none_stays_none() {
        let base = Config::default();
        let overlay = Config::default();
        let merged = base.merge(overlay);
        assert!(merged.theme.is_none());
        assert!(merged.pager.is_none());
    }
}
