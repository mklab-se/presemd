//! AI feature management
//!
//! `mdeck ai`         — show status (chat + image generation)
//! `mdeck ai test`    — test AI connection (chat and/or image generation)
//! `mdeck ai enable`  — enable AI for mdeck
//! `mdeck ai disable` — disable AI for mdeck
//! `mdeck ai config`  — open config in editor

use anyhow::Result;
use colored::Colorize;

use crate::cli::AiCommands;

const APP_NAME: &str = "mdeck";

pub async fn run(cmd: Option<AiCommands>) -> Result<()> {
    match cmd {
        None => status(),
        Some(AiCommands::Test { message }) => test(message).await,
        Some(AiCommands::Enable) => enable(),
        Some(AiCommands::Disable) => disable(),
        Some(AiCommands::Config) => open_config(),
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
fn has_capability(cap: &str) -> bool {
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

        let result: Result<ailloy::ImageResponse> = async {
            let client = ailloy::Client::for_capability("image")?;
            client
                .generate_image(
                    "A Pixar-style 3D database icon against a transparent background. \
                     The classic cylinder database shape we know from architecture diagrams, \
                     but reimagined as a living, friendly character — as if a Pixar artist \
                     drew it with personality and soul. It has subtle eyes or a gentle face \
                     integrated into the design. Richly detailed with tiny discoveries: \
                     miniature data rows visible through a translucent shell, tiny glowing \
                     circuits, a small spider web in one corner, a micro garden growing on \
                     top, a little door on the side. Warm lighting, soft shadows, the kind \
                     of whimsical detail that rewards a closer look. No text, no labels.",
                )
                .await
        }
        .await;

        match result {
            Ok(response) => {
                let ext = match response.format {
                    ailloy::ImageFormat::Png => "png",
                    ailloy::ImageFormat::Jpeg => "jpg",
                    ailloy::ImageFormat::Webp => "webp",
                };
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

/// Display the generated test image — inline if the terminal supports it, plus a hyperlink.
fn display_image_result(path: &std::path::Path) {
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
