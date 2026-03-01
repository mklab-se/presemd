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
    pub ai: Option<AiConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_generation: Option<ImageGenConfig>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: AiProvider,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiProvider {
    #[default]
    Claude,
    Codex,
    Copilot,
    Ollama,
}

impl AiProvider {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::Copilot => "Copilot",
            Self::Ollama => "Ollama",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Claude => "Anthropic Claude via claude CLI",
            Self::Codex => "OpenAI Codex via codex CLI",
            Self::Copilot => "GitHub Copilot via gh CLI",
            Self::Ollama => "Local models via Ollama",
        }
    }

    pub fn binary_name(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Copilot => "gh",
            Self::Ollama => "ollama",
        }
    }

    pub fn default_model(&self) -> Option<&'static str> {
        match self {
            Self::Claude => Some("sonnet"),
            Self::Codex => None,
            Self::Copilot => None,
            Self::Ollama => None,
        }
    }

    pub fn all() -> &'static [AiProvider] {
        &[
            AiProvider::Claude,
            AiProvider::Codex,
            AiProvider::Copilot,
            AiProvider::Ollama,
        ]
    }

    pub fn is_available(&self) -> bool {
        std::process::Command::new(self.binary_name())
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenConfig {
    pub provider: ImageGenProvider,

    /// API key. If not set, falls back to environment variable
    /// (OPENAI_API_KEY for openai, GEMINI_API_KEY for gemini).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageGenProvider {
    #[default]
    OpenAi,
    Gemini,
}

impl ImageGenProvider {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI (DALL-E)",
            Self::Gemini => "Google Gemini (Imagen)",
        }
    }

    pub fn env_var_name(&self) -> &'static str {
        match self {
            Self::OpenAi => "OPENAI_API_KEY",
            Self::Gemini => "GEMINI_API_KEY",
        }
    }
}

impl std::fmt::Display for ImageGenProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl ImageGenConfig {
    /// Resolve API key from config or environment variable.
    pub fn resolve_api_key(&self) -> Option<String> {
        if let Some(key) = &self.api_key {
            if !key.is_empty() {
                return Some(key.clone());
            }
        }
        std::env::var(self.provider.env_var_name()).ok()
    }
}

impl std::fmt::Display for AiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
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
