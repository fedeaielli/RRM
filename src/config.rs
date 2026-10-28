use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration loaded from `config.toml`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub processes: ProcessConfig,
    #[serde(default)]
    pub colors: ColorsConfig,
}

/// Palette overrides set from the Theme tab.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ColorsConfig {
    pub accent: Option<String>,
    pub good: Option<String>,
    pub warn: Option<String>,
    pub crit: Option<String>,
    pub rx: Option<String>,
    pub tx: Option<String>,
    pub banner: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub key: Option<String>,
    pub value: Option<String>,
}

/// Display-related configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayConfig {
    /// Refresh interval in milliseconds.
    pub refresh_ms: u64,
    /// Accent color as a hex string (e.g. `"#00BFFF"`).
    pub accent_color: String,
}

/// Process-tab configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProcessConfig {
    /// Number of processes to display.
    pub top_n: usize,
    /// Sort column: `"cpu"` or `"mem"`.
    pub sort_by: String,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            refresh_ms: 1000,
            accent_color: "#00BFFF".to_string(),
        }
    }
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            top_n: 15,
            sort_by: "cpu".to_string(),
        }
    }
}

impl Config {
    /// Load the configuration file.
    ///
    /// Checks `$XDG_CONFIG_HOME/rrm/config.toml` first, then
    /// `~/.config/rrm/config.toml`. Returns the default configuration when
    /// neither file exists.
    pub fn load() -> anyhow::Result<Config> {
        if let Some(xdg_config_home) = std::env::var_os("XDG_CONFIG_HOME") {
            let path = PathBuf::from(xdg_config_home)
                .join("rrm")
                .join("config.toml");
            if path.is_file() {
                return Self::from_file(&path);
            }
        }

        if let Some(home) = std::env::var_os("HOME") {
            let path = PathBuf::from(home)
                .join(".config")
                .join("rrm")
                .join("config.toml");
            if path.is_file() {
                return Self::from_file(&path);
            }
        }

        Ok(Config::default())
    }

    /// Write the configuration back to the config path, creating it if needed.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_path().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
        self.save_to(&path)
    }

    fn save_to(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(directory) = path.parent() {
            std::fs::create_dir_all(directory)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    fn from_file(path: &std::path::Path) -> anyhow::Result<Config> {
        let contents = std::fs::read_to_string(path)?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Parse `accent_color` as an RGB tuple.
    ///
    /// Falls back to the default accent color `(0, 191, 255)` when the string
    /// is not a valid `#RRGGBB` hex color.
    pub fn accent_color_rgb(&self) -> (u8, u8, u8) {
        parse_hex_color(&self.display.accent_color).unwrap_or((0, 191, 255))
    }
}

fn config_path() -> Option<PathBuf> {
    if let Some(xdg_config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(
            PathBuf::from(xdg_config_home)
                .join("rrm")
                .join("config.toml"),
        );
    }
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".config")
            .join("rrm")
            .join("config.toml")
    })
}

fn parse_hex_color(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn colors_roundtrip_through_save_and_load() {
        let mut config = Config::default();
        config.colors.rx = Some("#123456".to_string());
        let path = std::env::temp_dir().join(format!("rrm-config-{}.toml", std::process::id()));
        config.save_to(&path).expect("save should succeed");

        let reloaded = Config::from_file(&path).expect("load should succeed");
        assert_eq!(reloaded.colors.rx.as_deref(), Some("#123456"));
        assert_eq!(reloaded.display.refresh_ms, config.display.refresh_ms);
        let _ = std::fs::remove_file(path);
    }
}
