use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self { family: "FiraCode Nerd Font Mono".into(), size: 14.0 }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub font: FontConfig,
    pub scrollback: usize,
    pub welcome: bool,
    /// Name shown in the welcome banner; defaults to the macOS full name.
    pub user_name: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self { font: FontConfig::default(), scrollback: 10_000, welcome: true, user_name: None }
    }
}

pub fn path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/cardumen/config.toml"))
}

pub fn parse(text: &str) -> Result<Config, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

/// Never fails: a missing file or a bad file both fall back to defaults.
pub fn load() -> Config {
    let Some(path) = path() else { return Config::default() };
    match std::fs::read_to_string(&path) {
        Ok(text) => parse(&text).unwrap_or_else(|e| {
            eprintln!("cardumen: invalid {}, using defaults:\n{e}", path.display());
            Config::default()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(e) => {
            eprintln!("cardumen: cannot read {}: {e}", path.display());
            Config::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(parse("").unwrap(), Config::default());
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let c = parse("welcome = false\n[font]\nsize = 16").unwrap();
        assert_eq!(c.font.size, 16.0);
        assert_eq!(c.font.family, "FiraCode Nerd Font Mono");
        assert!(!c.welcome);
        assert_eq!(c.scrollback, 10_000);
    }

    #[test]
    fn invalid_file_reports_location() {
        let err = parse("[font]\nsize = \"big\"").unwrap_err();
        assert!(err.contains("line 2"), "{err}");
    }
}
