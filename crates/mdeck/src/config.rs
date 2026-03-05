use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const FILENAME: &str = "config.yaml";
const APP_DIR: &str = "mdeck";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<DefaultsConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing: Option<RoutingWeightsConfig>,
}

fn default_one() -> f64 {
    1.0
}

/// Configuration for routing cost weights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingWeightsConfig {
    #[serde(default = "default_one")]
    pub length: f64,
    #[serde(default = "default_one")]
    pub turn: f64,
    #[serde(default = "default_one")]
    pub lane_change: f64,
    #[serde(default = "default_one")]
    pub crossing: f64,
}

impl Default for RoutingWeightsConfig {
    fn default() -> Self {
        Self {
            length: 1.0,
            turn: 1.0,
            lane_change: 1.0,
            crossing: 1.0,
        }
    }
}

impl RoutingWeightsConfig {
    /// Convert to the internal `CostWeights` type.
    pub fn to_cost_weights(&self) -> crate::render::diagram::routing::types::CostWeights {
        crate::render::diagram::routing::types::CostWeights {
            length: self.length,
            turn: self.turn,
            lane_change: self.lane_change,
            crossing: self.crossing,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DefaultsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_mode: Option<String>,
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        dirs::config_dir()
            .map(|d| d.join(APP_DIR).join(FILENAME))
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!("No config found. Run `mdeck config show` to see defaults.")
            } else {
                anyhow::anyhow!("Failed to read config: {e}")
            }
        })?;
        let config: Config = serde_yaml::from_str(&contents)?;
        Ok(config)
    }

    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_default()
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let yaml = serde_yaml::to_string(self)?;
        let contents = format!("# MDeck configuration — https://github.com/mklab-se/mdeck\n{yaml}");
        std::fs::write(&path, contents)?;
        Ok(path)
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "defaults.theme" => {
                match value {
                    "light" | "dark" => {}
                    _ => anyhow::bail!("Invalid theme: {value}. Must be 'light' or 'dark'."),
                }
                self.defaults
                    .get_or_insert_with(DefaultsConfig::default)
                    .theme = Some(value.to_string());
            }
            "defaults.transition" => {
                match value {
                    "fade" | "slide" | "spatial" | "none" => {}
                    _ => anyhow::bail!(
                        "Invalid transition: {value}. Must be 'fade', 'slide', 'spatial', or 'none'."
                    ),
                }
                self.defaults
                    .get_or_insert_with(DefaultsConfig::default)
                    .transition = Some(value.to_string());
            }
            "defaults.aspect" => {
                match value {
                    "16:9" | "4:3" | "16:10" => {}
                    _ => anyhow::bail!(
                        "Invalid aspect ratio: {value}. Must be '16:9', '4:3', or '16:10'."
                    ),
                }
                self.defaults
                    .get_or_insert_with(DefaultsConfig::default)
                    .aspect = Some(value.to_string());
            }
            "defaults.start_mode" => {
                if value != "first" && value != "overview" && value.parse::<usize>().is_err() {
                    anyhow::bail!(
                        "Invalid start_mode: {value}. Must be 'first', 'overview', or a slide number."
                    );
                }
                self.defaults
                    .get_or_insert_with(DefaultsConfig::default)
                    .start_mode = Some(value.to_string());
            }
            _ => anyhow::bail!(
                "Unknown config key: {key}. Valid keys: defaults.theme, defaults.transition, defaults.aspect, defaults.start_mode"
            ),
        }
        Ok(())
    }
}
