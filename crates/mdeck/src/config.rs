use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FILENAME: &str = "config.yaml";
const APP_DIR: &str = "mdeck";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<DefaultsConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing: Option<RoutingWeightsConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub styles: Option<BTreeMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_styles: Option<BTreeMap<String, String>>,
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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_style: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_style: Option<String>,
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

    // ── Style helpers ──────────────────────────────────────────────────

    pub fn add_style(&mut self, name: &str, description: &str) {
        self.styles
            .get_or_insert_with(BTreeMap::new)
            .insert(name.to_string(), description.to_string());
    }

    pub fn remove_style(&mut self, name: &str) -> bool {
        let removed = self
            .styles
            .as_mut()
            .map(|m| m.remove(name).is_some())
            .unwrap_or(false);
        if removed {
            // Clear default if it referenced this style
            if let Some(ref defaults) = self.defaults {
                if defaults.image_style.as_deref() == Some(name) {
                    self.defaults.as_mut().unwrap().image_style = None;
                }
            }
            // Clean up empty map
            if self.styles.as_ref().is_some_and(|m| m.is_empty()) {
                self.styles = None;
            }
        }
        removed
    }

    pub fn clear_styles(&mut self) {
        self.styles = None;
        self.icon_styles = None;
        if let Some(ref mut defaults) = self.defaults {
            defaults.image_style = None;
            defaults.icon_style = None;
        }
    }

    pub fn get_style(&self, name: &str) -> Option<&str> {
        self.styles.as_ref()?.get(name).map(|s| s.as_str())
    }

    pub fn list_styles(&self) -> Vec<(&str, &str)> {
        self.styles
            .as_ref()
            .map(|m| m.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect())
            .unwrap_or_default()
    }

    pub fn add_icon_style(&mut self, name: &str, description: &str) {
        self.icon_styles
            .get_or_insert_with(BTreeMap::new)
            .insert(name.to_string(), description.to_string());
    }

    pub fn remove_icon_style(&mut self, name: &str) -> bool {
        let removed = self
            .icon_styles
            .as_mut()
            .map(|m| m.remove(name).is_some())
            .unwrap_or(false);
        if removed {
            if let Some(ref defaults) = self.defaults {
                if defaults.icon_style.as_deref() == Some(name) {
                    self.defaults.as_mut().unwrap().icon_style = None;
                }
            }
            if self.icon_styles.as_ref().is_some_and(|m| m.is_empty()) {
                self.icon_styles = None;
            }
        }
        removed
    }

    pub fn list_icon_styles(&self) -> Vec<(&str, &str)> {
        self.icon_styles
            .as_ref()
            .map(|m| m.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect())
            .unwrap_or_default()
    }

    pub fn get_icon_style(&self, name: &str) -> Option<&str> {
        self.icon_styles.as_ref()?.get(name).map(|s| s.as_str())
    }

    /// Resolve the effective image style description.
    /// Priority: defaults.image_style name → hardcoded default.
    pub fn resolve_image_style(&self) -> &str {
        if let Some(ref defaults) = self.defaults {
            if let Some(ref name) = defaults.image_style {
                if let Some(desc) = self.get_style(name) {
                    return desc;
                }
            }
        }
        crate::prompt::DEFAULT_IMAGE_STYLE
    }

    /// Resolve the effective icon style description.
    /// Priority: defaults.icon_style name → hardcoded default.
    pub fn resolve_icon_style(&self) -> &str {
        if let Some(ref defaults) = self.defaults {
            if let Some(ref name) = defaults.icon_style {
                if let Some(desc) = self.get_icon_style(name) {
                    return desc;
                }
            }
        }
        crate::prompt::DEFAULT_ICON_STYLE
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
