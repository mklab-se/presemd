//! AI feature management
//!
//! `mdeck ai`                — show status (chat + image generation)
//! `mdeck ai test`           — test AI connection (chat and/or image generation)
//! `mdeck ai enable`         — enable AI for mdeck
//! `mdeck ai disable`        — disable AI for mdeck
//! `mdeck ai config`         — interactive config wizard
//! `mdeck ai style ...`      — manage image styles
//! `mdeck ai generate-image` — generate a single image from a prompt
//! `mdeck ai generate`       — generate all AI images for a presentation

use std::io::{self, Write};

use anyhow::Result;
use colored::Colorize;
use futures::StreamExt;

use ailloy::config_tui;

use crate::cli::{AiCommands, StyleCommands};
use crate::config::{Config, DefaultsConfig};
use crate::prompt;

const APP_NAME: &str = "mdeck";

pub async fn run(cmd: Option<AiCommands>, quiet: bool) -> Result<()> {
    match cmd {
        None => config_tui::print_ai_status(APP_NAME, &["chat", "image"]),
        Some(AiCommands::Test { message }) => test(message).await,
        Some(AiCommands::Enable) => config_tui::enable_ai(APP_NAME),
        Some(AiCommands::Disable) => config_tui::disable_ai(APP_NAME),
        Some(AiCommands::Config) => {
            let mut config = ailloy::config::Config::load_global()?;
            config_tui::run_interactive_config(&mut config, &["chat", "image"]).await?;
            Ok(())
        }
        Some(AiCommands::Style { command }) => run_style(command).await,
        Some(AiCommands::GenerateImage(args)) => generate_image_cmd(args).await,
        Some(AiCommands::Generate { file, force, style }) => {
            crate::commands::generate::run(file, force, style, quiet).await
        }
        Some(AiCommands::Create(args)) => crate::commands::create::run(args, quiet).await,
        Some(AiCommands::Status) => config_tui::print_ai_status(APP_NAME, &["chat", "image"]),
        Some(AiCommands::Skill { emit, reference }) => {
            crate::commands::skill::run(emit, reference);
            Ok(())
        }
    }
}

/// Check if AI features are active (configured via ailloy + enabled for this tool).
#[allow(dead_code)]
pub fn is_ai_active() -> bool {
    config_tui::is_ai_active(APP_NAME)
}

/// Check if ailloy has a default node for a capability.
pub fn has_capability(cap: &str) -> bool {
    if config_tui::is_ai_disabled(APP_NAME) {
        return false;
    }
    ailloy::config::Config::load()
        .ok()
        .and_then(|c| c.default_node_for(cap).ok().map(|_| true))
        .unwrap_or(false)
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

async fn run_style(cmd: StyleCommands) -> Result<()> {
    match cmd {
        StyleCommands::Add {
            name,
            description,
            icon,
            interactive,
        }
        | StyleCommands::Set {
            name,
            description,
            icon,
            interactive,
        } => {
            if interactive {
                return run_interactive_style(name, icon).await;
            }
            let name = name.ok_or_else(|| {
                anyhow::anyhow!("Style name is required. Use -i for interactive mode.")
            })?;
            let description = description.ok_or_else(|| {
                anyhow::anyhow!("Style description is required. Use -i for interactive mode.")
            })?;
            let mut config = Config::load_or_default();
            if icon {
                config.add_icon_style(&name, &description);
                config.save()?;
                println!(
                    "{} Icon style {} saved.",
                    "✓".green().bold(),
                    name.cyan().bold()
                );
            } else {
                config.add_style(&name, &description);
                config.save()?;
                println!(
                    "{} Image style {} saved.",
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

// ── Interactive style creation ───────────────────────────────────────────────

const STYLE_SYSTEM_PROMPT: &str = "\
You are a style design assistant for mdeck, a markdown-based presentation tool. \
Your job is to help the user craft a concise image generation style description \
that will be used as a prefix for all AI-generated images in their presentations.

A style description should be 1-3 sentences that define the visual aesthetic: \
color palette, mood, artistic technique, level of detail, and composition preferences. \
It must NOT describe specific subjects — only the visual style.

Here are examples of good style descriptions:
- \"Modern, clean, and visually striking. Professional color palette with subtle gradients. \
Polished and contemporary, suitable for business or technical presentations.\"
- \"Warm watercolor illustrations with soft edges and muted earth tones. \
Hand-drawn feel with visible brushstrokes and gentle lighting.\"
- \"Retro 80s synthwave aesthetic with neon pinks, purples, and electric blues. \
Grid-based perspective with glowing edges and chrome reflections.\"

Ask focused questions (one or two at a time) about their preferred aesthetic. \
When you and the user agree on a style, output it wrapped exactly like this:

[STYLE: <the complete style description>]

If the user wants changes, refine and propose again. Keep the conversation friendly and concise.";

/// Extract a style description from the `[STYLE: <description>]` marker.
fn extract_style_description(text: &str) -> Option<String> {
    let marker = "[STYLE:";
    let start = text.find(marker)?;
    let after = &text[start + marker.len()..];
    let end = after.find(']')?;
    let desc = after[..end].trim();
    if desc.is_empty() {
        None
    } else {
        Some(desc.to_string())
    }
}

/// Stream a chat response from the AI, printing tokens in real-time.
/// Returns the full assembled response text.
async fn stream_chat_response(
    client: &ailloy::Client,
    history: &[ailloy::Message],
) -> Result<String> {
    let mut stream = client.chat_stream(history).await?;
    let mut assembled = String::new();

    while let Some(event) = stream.next().await {
        match event? {
            ailloy::StreamEvent::Delta(text) => {
                assembled.push_str(&text);
                print!("{text}");
                io::stdout().flush()?;
            }
            ailloy::StreamEvent::Done(_) => {
                println!();
            }
        }
    }

    Ok(assembled)
}

/// Read a line of user input with a `> ` prompt.
fn read_user_input() -> Result<Option<String>> {
    eprint!("{} ", ">".bold());
    io::stderr().flush()?;

    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(0) => Ok(None), // EOF
        Ok(_) => {
            let trimmed = input.trim().to_string();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        Err(e) => Err(e.into()),
    }
}

async fn run_interactive_style(name: Option<String>, icon: bool) -> Result<()> {
    if !has_capability("chat") {
        anyhow::bail!(
            "Chat AI not configured. Run `{APP_NAME} ai config` to set up a chat provider."
        );
    }

    let kind = if icon { "icon" } else { "image" };

    eprintln!("{} Interactive {} style creator", "mdeck".bold(), kind);
    eprintln!(
        "Type {} to exit, {} for help.",
        "/quit".bold(),
        "/help".bold()
    );

    let client = ailloy::Client::for_capability("chat")?;

    let mut history: Vec<ailloy::Message> = vec![ailloy::Message::system(STYLE_SYSTEM_PROMPT)];

    // Build the initial greeting message based on what we know
    let greeting = if let Some(ref n) = name {
        format!(
            "Greet me briefly and tell me you'll help me create {} style called \"{}\". \
             Ask what kind of visual aesthetic I'm going for.",
            if icon { "an icon" } else { "an image" },
            n
        )
    } else {
        format!(
            "Greet me briefly and tell me you'll help me create {} style for my presentations. \
             Ask what kind of visual aesthetic I'm going for.",
            if icon { "an icon" } else { "an image" },
        )
    };
    history.push(ailloy::Message::user(&greeting));

    eprintln!();
    let response = stream_chat_response(&client, &history).await?;
    history.push(ailloy::Message::assistant(&response));
    println!();

    // REPL loop
    loop {
        let input = match read_user_input()? {
            Some(s) => s,
            None => continue,
        };

        match input.as_str() {
            "/quit" | "/exit" | "/q" => break,
            "/clear" => {
                history = vec![ailloy::Message::system(STYLE_SYSTEM_PROMPT)];
                eprintln!("{}", "History cleared.".dimmed());
                continue;
            }
            "/help" => {
                eprintln!("{}", "Commands:".bold());
                eprintln!("  {} — Exit the session", "/quit".bold());
                eprintln!("  {} — Clear conversation history", "/clear".bold());
                eprintln!("  {} — Show this help", "/help".bold());
                continue;
            }
            s if s.starts_with('/') => {
                eprintln!(
                    "{} Unknown command: {}. Type {} for help.",
                    "!".yellow().bold(),
                    input,
                    "/help".bold()
                );
                continue;
            }
            _ => {}
        }

        history.push(ailloy::Message::user(&input));

        let response = stream_chat_response(&client, &history).await?;
        history.push(ailloy::Message::assistant(&response));

        // Check for [STYLE: ...] marker
        if let Some(description) = extract_style_description(&response) {
            println!();

            // Resolve the style name
            let style_name = if let Some(ref n) = name {
                n.clone()
            } else {
                // Ask the user for a name
                eprint!("{} Name for this style: ", "?".green().bold());
                io::stderr().flush()?;
                let mut name_input = String::new();
                io::stdin().read_line(&mut name_input)?;
                let name_input = name_input.trim().to_string();
                if name_input.is_empty() {
                    eprintln!("{} No name provided, style not saved.", "!".yellow().bold());
                    continue;
                }
                name_input
            };

            // Confirm before saving
            eprint!(
                "{} Save {} style {}? [Y/n] ",
                "?".green().bold(),
                kind,
                style_name.cyan().bold()
            );
            io::stderr().flush()?;
            let mut confirm = String::new();
            io::stdin().read_line(&mut confirm)?;
            let confirm = confirm.trim().to_lowercase();

            if confirm.is_empty() || confirm == "y" || confirm == "yes" {
                let mut config = Config::load_or_default();
                if icon {
                    config.add_icon_style(&style_name, &description);
                } else {
                    config.add_style(&style_name, &description);
                }
                config.save()?;
                println!(
                    "{} {} style {} saved.",
                    "✓".green().bold(),
                    if icon { "Icon" } else { "Image" },
                    style_name.cyan().bold()
                );
                println!("  {}", description.dimmed());
                break;
            } else {
                // Tell the model the user wants to refine
                history.push(ailloy::Message::user(
                    "I'm not happy with that style yet. Ask me what I'd like to change.",
                ));
                let followup = stream_chat_response(&client, &history).await?;
                history.push(ailloy::Message::assistant(&followup));
            }
        }

        println!();
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_style_description() {
        let text =
            "Here is the style: [STYLE: Modern, clean with subtle gradients and warm tones.]";
        assert_eq!(
            extract_style_description(text),
            Some("Modern, clean with subtle gradients and warm tones.".to_string())
        );
    }

    #[test]
    fn test_extract_style_description_empty() {
        assert_eq!(extract_style_description("[STYLE: ]"), None);
    }

    #[test]
    fn test_extract_style_description_missing() {
        assert_eq!(extract_style_description("No marker here"), None);
    }

    #[test]
    fn test_extract_style_description_multiline() {
        let text = "I suggest:\n[STYLE: Warm watercolor illustrations with soft edges and muted earth tones. Hand-drawn feel with visible brushstrokes.]\nWhat do you think?";
        assert_eq!(
            extract_style_description(text),
            Some(
                "Warm watercolor illustrations with soft edges and muted earth tones. Hand-drawn feel with visible brushstrokes.".to_string()
            )
        );
    }
}
