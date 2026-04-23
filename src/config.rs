#![allow(dead_code)]

use anyhow::Context;
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
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Config> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file {}", path.display()))?;
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("failed to parse config file {}", path.display()))?;
        Ok(config)
    }

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

    #[test]
    fn from_file_parses_valid_toml() {
        let dir = std::env::temp_dir().join("mdx_config_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("valid.toml");
        std::fs::write(
            &path,
            "theme = \"nord\"\nui_theme = \"frost\"\npager = true\n",
        )
        .unwrap();
        let config = Config::from_file(&path).unwrap();
        assert_eq!(config.theme.as_deref(), Some("nord"));
        assert_eq!(config.ui_theme.as_deref(), Some("frost"));
        assert_eq!(config.pager, Some(true));
        assert!(config.width.is_none());
    }

    #[test]
    fn from_file_rejects_unknown_keys() {
        let dir = std::env::temp_dir().join("mdx_config_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("unknown_key.toml");
        std::fs::write(&path, "colour = \"red\"\n").unwrap();
        let result = Config::from_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("failed to parse config file"), "err: {}", err);
    }

    #[test]
    fn from_file_rejects_invalid_toml() {
        let dir = std::env::temp_dir().join("mdx_config_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad_syntax.toml");
        std::fs::write(&path, "theme = \n").unwrap();
        let result = Config::from_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn from_file_errors_on_missing_file() {
        let path = std::path::Path::new("/tmp/mdx_config_nonexistent_file.toml");
        let result = Config::from_file(path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("failed to read config file"), "err: {}", err);
    }

    #[test]
    fn from_file_handles_empty_file() {
        let dir = std::env::temp_dir().join("mdx_config_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.toml");
        std::fs::write(&path, "").unwrap();
        let config = Config::from_file(&path).unwrap();
        assert!(config.theme.is_none());
        assert!(config.pager.is_none());
    }

    #[test]
    fn from_file_parses_all_fields() {
        let dir = std::env::temp_dir().join("mdx_config_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("all_fields.toml");
        std::fs::write(
            &path,
            "theme = \"base16-eighties.dark\"\nui_theme = \"nord\"\npager = false\nwidth = 100\nno_mermaid_rendering = true\nsplit_mermaid_rendering = false\n",
        ).unwrap();
        let config = Config::from_file(&path).unwrap();
        assert_eq!(config.theme.as_deref(), Some("base16-eighties.dark"));
        assert_eq!(config.ui_theme.as_deref(), Some("nord"));
        assert_eq!(config.pager, Some(false));
        assert_eq!(config.width, Some(100));
        assert_eq!(config.no_mermaid_rendering, Some(true));
        assert_eq!(config.split_mermaid_rendering, Some(false));
    }
}
