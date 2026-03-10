//! AI feature management
//!
//! `mdeck ai`                — show status (chat + image generation)
//! `mdeck ai test`           — test AI connection (chat and/or image generation)
//! `mdeck ai enable`         — enable AI for mdeck
//! `mdeck ai disable`        — disable AI for mdeck
//! `mdeck ai config`         — open config in editor
//! `mdeck ai style ...`      — manage image styles
//! `mdeck ai generate-image` — generate a single image from a prompt
//! `mdeck ai generate`       — generate all AI images for a presentation

use anyhow::Result;
use colored::Colorize;

use crate::cli::{AiCommands, StyleCommands};
use crate::config::{Config, DefaultsConfig};
use crate::prompt;

const APP_NAME: &str = "mdeck";

pub async fn run(cmd: Option<AiCommands>, quiet: bool) -> Result<()> {
    match cmd {
        None => status(),
        Some(AiCommands::Test { message }) => test(message).await,
        Some(AiCommands::Enable) => enable(),
        Some(AiCommands::Disable) => disable(),
        Some(AiCommands::Config) => open_config(),
        Some(AiCommands::Style { command }) => run_style(command),
        Some(AiCommands::GenerateImage(args)) => generate_image_cmd(args).await,
        Some(AiCommands::Generate { file, force, style }) => {
            crate::commands::generate::run(file, force, style, quiet).await
        }
    }
}

/// Check if AI features are active (configured via ailloy + enabled for this tool).
#[allow(dead_code)]
pub fn is_ai_active() -> bool {
    !is_disabled()
        && ailloy::config::Config::load()
            .ok()
            .and_then(|c| c.default_chat_node().ok().map(|_| ()))
            .is_some()
}

/// Check if ailloy has a default node for a capability.
pub fn has_capability(cap: &str) -> bool {
    ailloy::config::Config::load()
        .ok()
        .and_then(|c| c.default_node_for(cap).ok().map(|_| true))
        .unwrap_or(false)
}

fn status() -> Result<()> {
    let enabled = !is_disabled();

    // --- Chat completion ---
    println!("{}", "Chat Completion".bold().underline());
    if has_capability("chat") {
        let config = ailloy::config::Config::load()?;
        let (id, node) = config.default_node_for("chat")?;
        if enabled {
            println!("  {} configured and enabled\n", "✓".green().bold());
        } else {
            println!("  {} configured but disabled\n", "!".yellow().bold());
        }
        print_node_info(id, node);
    } else {
        println!("  {} not configured\n", "✗".red().bold());
        println!(
            "  Run {} to set up a chat provider.",
            format!("{APP_NAME} ai config").cyan()
        );
    }

    // --- Image generation ---
    println!("\n{}", "Image Generation".bold().underline());
    if has_capability("image") {
        let config = ailloy::config::Config::load()?;
        let (id, node) = config.default_node_for("image")?;
        if enabled {
            println!("  {} configured and enabled\n", "✓".green().bold());
        } else {
            println!("  {} configured but disabled\n", "!".yellow().bold());
        }
        print_node_info(id, node);
    } else {
        println!("  {} not configured\n", "✗".red().bold());
        println!(
            "  Run {} to set up an image provider.",
            format!("{APP_NAME} ai config").cyan()
        );
    }

    // --- Overall status ---
    if !enabled {
        println!(
            "\n  AI features are {}. Run {} to re-enable.",
            "disabled".yellow(),
            format!("{APP_NAME} ai enable").cyan()
        );
    }

    Ok(())
}

async fn test(message: Option<String>) -> Result<()> {
    let has_chat = has_capability("chat");
    let has_image = has_capability("image");

    if !has_chat && !has_image {
        println!("{} No AI features configured.\n", "✗".red().bold());
        println!(
            "  Run {} to set up AI.",
            format!("{APP_NAME} ai config").cyan()
        );
        anyhow::bail!("No AI features configured");
    }

    // Build the list of available options — always prompt so the user sees what's available
    let mut options = Vec::new();
    if has_chat && has_image {
        options.push("Both chat completion and image generation");
        options.push("Chat completion only");
        options.push("Image generation only");
    } else if has_chat {
        options.push("Chat completion");
    } else {
        options.push("Image generation");
    }

    let choice = inquire::Select::new("What would you like to test?", options).prompt()?;

    let test_chat = choice.contains("Chat") || choice.contains("Both");
    let test_image = choice.contains("Image") || choice.contains("Both");

    let mut all_passed = true;

    if test_chat {
        println!("\n{}", "Testing chat completion...".bold());
        let msg = message
            .clone()
            .unwrap_or_else(|| "Say hello in one sentence.".to_string());

        let result: Result<ailloy::ChatResponse> = async {
            let client = ailloy::Client::for_capability("chat")?;
            client.chat(&[ailloy::Message::user(&msg)]).await
        }
        .await;

        match result {
            Ok(response) => {
                println!("  {}\n", "✓ PASS".green().bold());
                println!("  {}", response.content);
            }
            Err(e) => {
                println!("  {}\n", "✗ FAIL".red().bold());
                println!("  Error: {e}");
                all_passed = false;
            }
        }
    }

    if test_image {
        println!("\n{}", "Testing image generation...".bold());

        // Ask what kind of image to test
        let image_type = if has_image {
            let choices = vec!["Normal image", "Icon"];
            inquire::Select::new("What type of image?", choices)
                .prompt()
                .unwrap_or("Normal image")
        } else {
            "Normal image"
        };

        let config = Config::load_or_default();

        let test_prompt = if image_type == "Icon" {
            let style = config.resolve_icon_style();
            prompt::build_icon_prompt(style, "A database")
        } else {
            let style = config.resolve_image_style();
            prompt::build_image_prompt(
                style,
                "A bunch of papers, presentation slides, and notes scattered on a messy wooden \
                 desk \u{2014} a tribute to the old way of making presentations before mdeck.",
                prompt::Orientation::Horizontal,
            )
        };

        let result: Result<ailloy::ImageResponse> = async {
            let client = ailloy::Client::for_capability("image")?;
            client.generate_image(&test_prompt).await
        }
        .await;

        match result {
            Ok(response) => {
                let ext = image_ext(&response.format);
                let path = std::path::PathBuf::from(format!("/tmp/mdeck-ai-test.{ext}"));
                std::fs::write(&path, &response.data)?;

                println!("  {}", "✓ PASS".green().bold());
                println!(
                    "  Generated {}x{} {} image",
                    response.width,
                    response.height,
                    ext.to_uppercase()
                );
                if let Some(ref revised) = response.revised_prompt {
                    println!("  Revised prompt: {}", revised.dimmed());
                }
                println!();
                display_image_result(&path);
                offer_cleanup(&path);
            }
            Err(e) => {
                println!("  {}\n", "✗ FAIL".red().bold());
                println!("  Error: {e}");
                all_passed = false;
            }
        }
    }

    // Show what wasn't tested and why
    if !has_image {
        println!(
            "\n  {} Image generation not configured — run {} to add an image provider",
            "ℹ".blue().bold(),
            format!("{APP_NAME} ai config").cyan()
        );
    }
    if !has_chat {
        println!(
            "\n  {} Chat completion not configured — run {} to add a chat provider",
            "ℹ".blue().bold(),
            format!("{APP_NAME} ai config").cyan()
        );
    }

    if all_passed {
        Ok(())
    } else {
        println!(
            "\n  Run {} to check your configuration.",
            format!("{APP_NAME} ai config").cyan()
        );
        anyhow::bail!("One or more AI tests failed");
    }
}

// ── Style management ─────────────────────────────────────────────────────────

fn run_style(cmd: StyleCommands) -> Result<()> {
    match cmd {
        StyleCommands::Add {
            name,
            description,
            icon,
        } => {
            let mut config = Config::load_or_default();
            if icon {
                config.add_icon_style(&name, &description);
                config.save()?;
                println!(
                    "{} Icon style {} added.",
                    "✓".green().bold(),
                    name.cyan().bold()
                );
            } else {
                config.add_style(&name, &description);
                config.save()?;
                println!(
                    "{} Image style {} added.",
                    "✓".green().bold(),
                    name.cyan().bold()
                );
            }
            Ok(())
        }
        StyleCommands::Remove { name, icon } => {
            let mut config = Config::load_or_default();
            let removed = if icon {
                config.remove_icon_style(&name)
            } else {
                config.remove_style(&name)
            };
            if removed {
                config.save()?;
                let kind = if icon { "Icon style" } else { "Image style" };
                println!("{} {kind} {} removed.", "✓".green().bold(), name.cyan());
            } else {
                let kind = if icon { "icon style" } else { "image style" };
                println!(
                    "{} No {kind} named {} found.",
                    "!".yellow().bold(),
                    name.cyan()
                );
            }
            Ok(())
        }
        StyleCommands::List => {
            let config = Config::load_or_default();
            let styles = config.list_styles();
            let icon_styles = config.list_icon_styles();

            if styles.is_empty() && icon_styles.is_empty() {
                println!("No styles defined.");
                println!(
                    "  Use {} to add one.",
                    format!("{APP_NAME} ai style add <name> <description>").cyan()
                );
                return Ok(());
            }

            if !styles.is_empty() {
                println!("{}", "Image Styles".bold().underline());
                let default_name = config
                    .defaults
                    .as_ref()
                    .and_then(|d| d.image_style.as_deref());
                for (name, desc) in &styles {
                    let marker = if default_name == Some(name) {
                        " (default)".green().to_string()
                    } else {
                        String::new()
                    };
                    println!("  {}{marker}", name.cyan().bold());
                    println!("    {desc}");
                }
            }

            if !icon_styles.is_empty() {
                if !styles.is_empty() {
                    println!();
                }
                println!("{}", "Icon Styles".bold().underline());
                let default_name = config
                    .defaults
                    .as_ref()
                    .and_then(|d| d.icon_style.as_deref());
                for (name, desc) in &icon_styles {
                    let marker = if default_name == Some(name) {
                        " (default)".green().to_string()
                    } else {
                        String::new()
                    };
                    println!("  {}{marker}", name.cyan().bold());
                    println!("    {desc}");
                }
            }

            Ok(())
        }
        StyleCommands::Clear => {
            let mut config = Config::load_or_default();
            config.clear_styles();
            config.save()?;
            println!(
                "{} All styles cleared and defaults reset.",
                "✓".green().bold()
            );
            Ok(())
        }
        StyleCommands::SetDefault { name } => {
            let mut config = Config::load_or_default();
            if config.get_style(&name).is_none() {
                anyhow::bail!(
                    "No image style named '{}'. Use `{APP_NAME} ai style add` first.",
                    name
                );
            }
            config
                .defaults
                .get_or_insert_with(DefaultsConfig::default)
                .image_style = Some(name.clone());
            config.save()?;
            println!(
                "{} Default image style set to {}.",
                "✓".green().bold(),
                name.cyan().bold()
            );
            Ok(())
        }
        StyleCommands::SetIconDefault { name } => {
            let mut config = Config::load_or_default();
            if config.get_icon_style(&name).is_none() {
                anyhow::bail!(
                    "No icon style named '{}'. Use `{APP_NAME} ai style add --icon` first.",
                    name
                );
            }
            config
                .defaults
                .get_or_insert_with(DefaultsConfig::default)
                .icon_style = Some(name.clone());
            config.save()?;
            println!(
                "{} Default icon style set to {}.",
                "✓".green().bold(),
                name.cyan().bold()
            );
            Ok(())
        }
        StyleCommands::ShowDefaults => {
            let config = Config::load_or_default();

            println!("{}", "Default Image Style".bold().underline());
            if let Some(ref name) = config.defaults.as_ref().and_then(|d| d.image_style.clone()) {
                if let Some(desc) = config.get_style(name) {
                    println!("  {} {}", "Name:".bold(), name.cyan());
                    println!("  {desc}");
                } else {
                    println!(
                        "  {} (configured as '{}' but style not found, using hardcoded)",
                        "!".yellow().bold(),
                        name
                    );
                    println!("  {}", prompt::DEFAULT_IMAGE_STYLE.dimmed());
                }
            } else {
                println!("  {} (hardcoded)", "(none set)".dimmed());
                println!("  {}", prompt::DEFAULT_IMAGE_STYLE.dimmed());
            }

            println!("\n{}", "Default Icon Style".bold().underline());
            if let Some(ref name) = config.defaults.as_ref().and_then(|d| d.icon_style.clone()) {
                if let Some(desc) = config.get_icon_style(name) {
                    println!("  {} {}", "Name:".bold(), name.cyan());
                    println!("  {desc}");
                } else {
                    println!(
                        "  {} (configured as '{}' but style not found, using hardcoded)",
                        "!".yellow().bold(),
                        name
                    );
                    println!("  {}", prompt::DEFAULT_ICON_STYLE.dimmed());
                }
            } else {
                println!("  {} (hardcoded)", "(none set)".dimmed());
                println!("  {}", prompt::DEFAULT_ICON_STYLE.dimmed());
            }

            Ok(())
        }
    }
}

// ── Ad-hoc image generation ──────────────────────────────────────────────────

async fn generate_image_cmd(args: crate::cli::GenerateImageArgs) -> Result<()> {
    if !has_capability("image") {
        anyhow::bail!(
            "Image generation not configured. Run `{APP_NAME} ai config` to set up an image provider."
        );
    }

    let config = Config::load_or_default();

    // Resolve style: explicit --style (name lookup or literal) > config default > hardcoded
    let style = if let Some(ref s) = args.style {
        if args.icon {
            config.get_icon_style(s).unwrap_or(s).to_string()
        } else {
            config.get_style(s).unwrap_or(s).to_string()
        }
    } else if args.icon {
        config.resolve_icon_style().to_string()
    } else {
        config.resolve_image_style().to_string()
    };

    let combined_prompt = if args.icon {
        prompt::build_icon_prompt(&style, &args.prompt)
    } else {
        prompt::build_image_prompt(&style, &args.prompt, prompt::Orientation::Horizontal)
    };

    println!("Generating image...");

    let client = ailloy::Client::for_capability("image")?;
    let response = client.generate_image(&combined_prompt).await?;

    let ext = image_ext(&response.format);

    let path = if let Some(ref output) = args.output {
        output.clone()
    } else {
        std::path::PathBuf::from(format!("/tmp/mdeck-generated.{ext}"))
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &response.data)?;

    println!(
        "{} Generated {}x{} {} image",
        "✓".green().bold(),
        response.width,
        response.height,
        ext.to_uppercase()
    );
    if let Some(ref revised) = response.revised_prompt {
        println!("  Revised prompt: {}", revised.dimmed());
    }
    println!();
    display_image_result(&path);

    if args.output.is_none() {
        offer_cleanup(&path);
    }

    Ok(())
}

// ── Shared helpers ───────────────────────────────────────────────────────────

pub fn image_ext(format: &ailloy::ImageFormat) -> &'static str {
    match format {
        ailloy::ImageFormat::Png => "png",
        ailloy::ImageFormat::Jpeg => "jpg",
        ailloy::ImageFormat::Webp => "webp",
    }
}

/// Display the generated test image — inline if the terminal supports it, plus a hyperlink.
pub fn display_image_result(path: &std::path::Path) {
    use std::io::Write;

    let display_path = path.display();
    let file_url = format!("file://{display_path}");

    // Try inline image display (terminal-specific protocols)
    if try_display_inline(path) {
        // Flush to ensure the image escape sequence is sent before the link
        let _ = std::io::stdout().flush();
        println!();
    }

    // OSC 8 clickable hyperlink: ESC ] 8 ; ; url BEL text ESC ] 8 ; ; BEL
    println!("  Image saved: \x1b]8;;{file_url}\x07{display_path}\x1b]8;;\x07");
}

/// Attempt to display an image inline using terminal-specific image protocols.
/// Returns true if we attempted display (we can't easily detect if it actually rendered).
fn try_display_inline(path: &std::path::Path) -> bool {
    use std::io::Write;

    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let term = std::env::var("TERM").unwrap_or_default();

    let Ok(data) = std::fs::read(path) else {
        return false;
    };

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);

    // Kitty graphics protocol — Kitty, Ghostty
    if term_program.contains("kitty")
        || term_program.contains("ghostty")
        || term.contains("xterm-kitty")
        || term.contains("xterm-ghostty")
    {
        display_kitty(&b64);
        let _ = std::io::stdout().flush();
        return true;
    }

    // iTerm2 inline image protocol — iTerm2, WezTerm
    if term_program.contains("iTerm") || term_program.contains("WezTerm") {
        display_iterm2(&b64);
        let _ = std::io::stdout().flush();
        return true;
    }

    false
}

/// Display image using the iTerm2 inline image protocol (iTerm2, WezTerm).
fn display_iterm2(b64: &str) {
    // ESC ] 1337 ; File=[args] : base64data BEL
    print!("\x1b]1337;File=inline=1;width=20;preserveAspectRatio=1:{b64}\x07");
}

/// Display image using the Kitty graphics protocol.
/// Sends base64 PNG data in chunks of up to 4096 bytes.
fn display_kitty(b64: &str) {
    // Kitty protocol: ESC_APC G <key>=<value>,... ; <base64 data> ESC \
    // First chunk: a=T (transmit+display), f=100 (PNG), m=1 (more chunks follow)
    // Last chunk: m=0 (final)
    let chunk_size = 4096;
    let chunks: Vec<&str> = b64
        .as_bytes()
        .chunks(chunk_size)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect();

    for (i, chunk) in chunks.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == chunks.len() - 1;
        let more = if is_last { 0 } else { 1 };

        if is_first {
            print!("\x1b_Ga=T,f=100,m={more};{chunk}\x1b\\");
        } else {
            print!("\x1b_Gm={more};{chunk}\x1b\\");
        }
    }
}

/// Ask the user whether to keep or delete the test image.
fn offer_cleanup(path: &std::path::Path) {
    println!();
    let keep = inquire::Confirm::new("Keep the generated image?")
        .with_default(false)
        .prompt()
        .unwrap_or(false);

    if keep {
        println!("  Image kept at {}", path.display());
    } else if std::fs::remove_file(path).is_ok() {
        println!("  Image deleted.");
    }
}

fn enable() -> Result<()> {
    let marker = disabled_marker_path();
    if marker.exists() {
        std::fs::remove_file(&marker)?;
    }
    println!("{} AI features enabled for {APP_NAME}.", "✓".green().bold());
    Ok(())
}

fn disable() -> Result<()> {
    let marker = disabled_marker_path();
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&marker, "")?;
    println!(
        "{} AI features disabled for {APP_NAME}.",
        "!".yellow().bold()
    );
    Ok(())
}

fn open_config() -> Result<()> {
    let path = ailloy::config::Config::config_path()?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if !path.exists() {
        std::fs::write(
            &path,
            "# ailloy AI configuration — https://github.com/mklab-se/ailloy\n\n\
             nodes:\n  default:\n    provider: openai\n    model: gpt-4o\n    # api_key: sk-...\n",
        )?;
    }

    let editor = resolve_editor();
    println!("Opening {} in {editor}...", path.display());
    let status = std::process::Command::new(&editor).arg(&path).status()?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    Ok(())
}

/// Resolve the best available editor: $VISUAL → $EDITOR → code → vi
fn resolve_editor() -> String {
    if let Ok(v) = std::env::var("VISUAL") {
        if !v.is_empty() {
            return v;
        }
    }
    if let Ok(v) = std::env::var("EDITOR") {
        if !v.is_empty() {
            return v;
        }
    }
    // Detect VS Code on PATH
    if which("code") {
        return "code".to_string();
    }
    "vi".to_string()
}

fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn disabled_marker_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(APP_NAME)
        .join("ai_disabled")
}

fn is_disabled() -> bool {
    disabled_marker_path().exists()
}

fn print_node_info(id: &str, node: &ailloy::config::AiNode) {
    println!("  {} {}", "Node:".bold(), id.cyan());
    println!("  {} {:?}", "Provider:".bold(), node.provider);
    if let Some(ref model) = node.model {
        println!("  {} {}", "Model:".bold(), model);
    }
    if let Some(ref alias) = node.alias {
        println!("  {} {}", "Alias:".bold(), alias);
    }
}
