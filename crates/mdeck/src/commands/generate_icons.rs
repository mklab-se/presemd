use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::config::{Config, ImageGenProvider};

/// Icon directory name relative to the markdown file.
const ICON_DIR: &str = "media/diagram-icons";

/// Run the generate-icons command.
pub fn run(file: &Path) -> Result<()> {
    let content = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read {}", file.display()))?;

    let base_path = file.parent().unwrap_or(Path::new("."));
    let icon_dir = base_path.join(ICON_DIR);

    // Collect all icon names used in @diagram blocks
    let icons = collect_diagram_icons(&content);
    if icons.is_empty() {
        println!("{}", "No diagram icons found in the presentation.".yellow());
        return Ok(());
    }

    println!(
        "Found {} unique icon type(s): {}",
        icons.len(),
        icons
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Check which icons are missing
    let missing: Vec<&String> = icons
        .iter()
        .filter(|name| !icon_dir.join(format!("{name}.png")).exists())
        .collect();

    if missing.is_empty() {
        println!(
            "{}",
            "All icons already exist in media/diagram-icons/. Nothing to generate."
                .green()
                .bold()
        );
        return Ok(());
    }

    println!(
        "{} icon(s) missing: {}",
        missing.len(),
        missing
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Load config and get image generation settings
    let config = Config::load_or_default();
    let gen_config = config.image_generation.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "No image_generation config found.\n\
             \n\
             Add to ~/.config/mdeck/config.yaml:\n\
             \n\
             image_generation:\n\
             \x20 provider: open-ai      # or gemini\n\
             \x20 api_key: \"your-key\"    # or set OPENAI_API_KEY env var\n"
        )
    })?;

    let api_key = gen_config.resolve_api_key().ok_or_else(|| {
        let env_var = gen_config.provider.env_var_name();
        anyhow::anyhow!(
            "No API key found. Set it in config.yaml or via {env_var} environment variable."
        )
    })?;

    // Create icon directory
    std::fs::create_dir_all(&icon_dir)
        .with_context(|| format!("Failed to create {}", icon_dir.display()))?;

    // Generate each missing icon
    let mut generated = 0;
    let mut failed = 0;

    for icon_name in &missing {
        print!("  Generating {icon_name}...");

        match generate_icon(&gen_config.provider, &api_key, icon_name, &icon_dir) {
            Ok(path) => {
                println!(" {}", format!("saved to {}", path.display()).green());
                generated += 1;
            }
            Err(e) => {
                println!(" {}", format!("failed: {e}").red());
                failed += 1;
            }
        }
    }

    println!();
    if generated > 0 {
        println!(
            "{}",
            format!("Generated {generated} icon(s) in {ICON_DIR}/").green()
        );
    }
    if failed > 0 {
        println!("{}", format!("{failed} icon(s) failed to generate.").red());
    }

    Ok(())
}

/// Extract all unique icon names from @diagram blocks in the content.
fn collect_diagram_icons(content: &str) -> BTreeSet<String> {
    let mut icons = BTreeSet::new();
    let mut in_diagram = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```@diagram") {
            in_diagram = true;
            continue;
        }
        if in_diagram && trimmed.starts_with("```") {
            in_diagram = false;
            continue;
        }
        if !in_diagram {
            continue;
        }

        // Look for (icon: NAME, ...) in the line
        if let Some(paren_start) = trimmed.rfind('(') {
            if trimmed.ends_with(')') {
                let meta = &trimmed[paren_start + 1..trimmed.len() - 1];
                for part in meta.split(',') {
                    let part = part.trim();
                    if let Some(val) = part
                        .strip_prefix("icon:")
                        .or_else(|| part.strip_prefix("icon :"))
                    {
                        let icon = val.trim();
                        if !icon.is_empty() {
                            icons.insert(icon.to_string());
                        }
                    }
                }
            }
        }
    }

    icons
}

/// Generate a single icon using the configured API.
fn generate_icon(
    provider: &ImageGenProvider,
    api_key: &str,
    icon_name: &str,
    output_dir: &Path,
) -> Result<PathBuf> {
    let prompt = format!(
        "A clean, minimalist flat-design icon of a {} for a technical architecture diagram. \
         Simple geometric shapes, professional style, centered on a solid dark gray (#2d2d2d) background, \
         suitable as a small icon in a presentation. No text, no labels. \
         Single color (#4fc3f7 light blue) on dark background.",
        icon_name_to_description(icon_name)
    );

    let png_bytes = match provider {
        ImageGenProvider::OpenAi => generate_openai(api_key, &prompt)?,
        ImageGenProvider::Gemini => generate_gemini(api_key, &prompt)?,
    };

    let output_path = output_dir.join(format!("{icon_name}.png"));
    std::fs::write(&output_path, &png_bytes)
        .with_context(|| format!("Failed to write {}", output_path.display()))?;

    Ok(output_path)
}

/// Map icon names to natural-language descriptions for the AI prompt.
fn icon_name_to_description(name: &str) -> &str {
    match name {
        "user" => "person/user (head and shoulders silhouette)",
        "server" => "server rack or server computer",
        "database" | "db" => "database (cylinder shape)",
        "cloud" => "cloud computing / cloud service",
        "lock" | "auth" => "security lock / padlock",
        "api" | "gateway" => "API gateway (hexagonal shape)",
        "cache" => "cache / lightning bolt for speed",
        "container" => "container / Docker container (box within box)",
        "browser" | "web" => "web browser window",
        "mobile" | "phone" => "mobile phone / smartphone",
        "queue" => "message queue",
        "storage" => "file storage / disk",
        "function" | "lambda" => "serverless function / lambda",
        "network" | "lb" => "network / load balancer",
        "key" => "encryption key",
        "mail" | "email" => "email envelope",
        "logs" | "logging" => "log file / document with lines",
        "monitor" | "monitoring" => "monitoring dashboard / metrics display",
        _ => name,
    }
}

/// Generate an icon using OpenAI's DALL-E API.
fn generate_openai(api_key: &str, prompt: &str) -> Result<Vec<u8>> {
    let body = serde_json::json!({
        "model": "dall-e-3",
        "prompt": prompt,
        "n": 1,
        "size": "1024x1024",
        "response_format": "b64_json",
        "quality": "standard"
    });

    let response: serde_json::Value = ureq::post("https://api.openai.com/v1/images/generations")
        .header("Authorization", &format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send_json(&body)
        .context("Failed to call OpenAI API")?
        .body_mut()
        .read_json()
        .context("Failed to parse OpenAI response")?;

    let b64 = response["data"][0]["b64_json"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No image data in OpenAI response"))?;

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .context("Failed to decode base64 image data")?;

    Ok(bytes)
}

/// Generate an icon using Google Gemini's Imagen API.
fn generate_gemini(api_key: &str, prompt: &str) -> Result<Vec<u8>> {
    let body = serde_json::json!({
        "contents": [{
            "parts": [{
                "text": prompt
            }]
        }],
        "generationConfig": {
            "responseModalities": ["TEXT", "IMAGE"]
        }
    });

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-exp:generateContent?key={api_key}"
    );

    let response: serde_json::Value = ureq::post(&url)
        .header("Content-Type", "application/json")
        .send_json(&body)
        .context("Failed to call Gemini API")?
        .body_mut()
        .read_json()
        .context("Failed to parse Gemini response")?;

    // Gemini returns inline image data in the candidates
    let parts = response["candidates"][0]["content"]["parts"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No parts in Gemini response"))?;

    for part in parts {
        if let Some(inline_data) = part.get("inlineData") {
            if let Some(b64) = inline_data["data"].as_str() {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .context("Failed to decode base64 image data from Gemini")?;
                return Ok(bytes);
            }
        }
    }

    anyhow::bail!("No image data found in Gemini response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_icons_basic() {
        let content = r#"
# Title

```@diagram
# Components
- Gateway  (icon: api,      pos: 1,1)
- Auth     (icon: lock,     pos: 2,1)
- DB       (icon: database, pos: 3,2)

- Gateway -> Auth: validates
```
"#;
        let icons = collect_diagram_icons(content);
        assert_eq!(icons.len(), 3);
        assert!(icons.contains("api"));
        assert!(icons.contains("lock"));
        assert!(icons.contains("database"));
    }

    #[test]
    fn test_collect_icons_multiple_diagrams() {
        let content = r#"
```@diagram
- A (icon: server)
- B (icon: database)
```

Some text

```@diagram
- C (icon: server)
- D (icon: cloud)
```
"#;
        let icons = collect_diagram_icons(content);
        assert_eq!(icons.len(), 3); // server, database, cloud (server deduped)
        assert!(icons.contains("server"));
        assert!(icons.contains("database"));
        assert!(icons.contains("cloud"));
    }

    #[test]
    fn test_collect_icons_no_diagrams() {
        let content = "# Just a heading\n\nSome text.";
        let icons = collect_diagram_icons(content);
        assert!(icons.is_empty());
    }

    #[test]
    fn test_icon_name_to_description() {
        assert!(icon_name_to_description("user").contains("person"));
        assert!(icon_name_to_description("database").contains("cylinder"));
        assert_eq!(icon_name_to_description("custom-thing"), "custom-thing");
    }
}
