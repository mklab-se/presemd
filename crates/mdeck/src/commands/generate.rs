//! `mdeck ai generate <file>` — scan a presentation for AI image markers and generate them.

use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::Result;
use colored::Colorize;

use crate::commands::ai;
use crate::config::Config;
use crate::parser::{self, Layout};
use crate::prompt::{self, Orientation};

pub const DEFAULT_IMAGE_STYLE: &str = prompt::DEFAULT_IMAGE_STYLE;
pub const DEFAULT_ICON_STYLE: &str = prompt::DEFAULT_ICON_STYLE;

// ── Marker types ─────────────────────────────────────────────────────────────

/// An image marker found in the markdown file.
struct ImageMarker {
    /// 0-indexed line number in the raw file.
    line_index: usize,
    /// The alt text / prompt (may be empty for auto-prompt).
    alt_text: String,
    /// 1-indexed slide number.
    slide_number: usize,
    /// Image orientation based on layout context.
    orientation: Orientation,
}

/// Result of a single image generation task.
struct GenerationResult {
    /// Task index (for display ordering).
    index: usize,
    /// The prompt text used.
    prompt_text: String,
    /// 0-indexed line number in the raw file.
    line_index: usize,
    /// The original line text.
    old_line: String,
    /// The API result.
    result: anyhow::Result<ailloy::ImageResponse>,
    /// Whether this is an icon (vs. a regular image).
    is_icon: bool,
}

/// A diagram icon marker found in the markdown file.
struct IconMarker {
    /// 0-indexed line number in the raw file.
    line_index: usize,
    /// The prompt for the icon.
    prompt_text: String,
    /// 1-indexed slide number.
    slide_number: usize,
}

pub async fn run(
    file: PathBuf,
    force: bool,
    style_override: Option<String>,
    quiet: bool,
) -> Result<()> {
    if !ai::has_capability("image") {
        anyhow::bail!(
            "Image generation not configured. Run `mdeck ai config` to set up an image provider."
        );
    }

    let content = std::fs::read_to_string(&file)?;
    let lines: Vec<&str> = content.lines().collect();
    let base_path = file.parent().unwrap_or(Path::new("."));

    // Parse the presentation for layout info
    let presentation = parser::parse(&content, base_path);

    // Resolve styles
    let config = Config::load_or_default();

    let image_style = resolve_style(
        &style_override,
        presentation.meta.image_style.as_deref(),
        config
            .defaults
            .as_ref()
            .and_then(|d| d.image_style.as_deref()),
        |name| config.get_style(name),
        DEFAULT_IMAGE_STYLE,
    );

    let icon_style = resolve_style(
        &style_override,
        presentation.meta.icon_style.as_deref(),
        config
            .defaults
            .as_ref()
            .and_then(|d| d.icon_style.as_deref()),
        |name| config.get_icon_style(name),
        DEFAULT_ICON_STYLE,
    );

    // Scan for markers
    let (image_markers, icon_markers) = scan_markers(&lines, &presentation)?;

    if image_markers.is_empty() && icon_markers.is_empty() {
        if !quiet {
            println!("No image-generation markers found in {}.", file.display());
        }
        return Ok(());
    }

    // Auto-prompt for empty alt texts
    let has_chat = ai::has_capability("chat");
    let mut image_markers = image_markers;
    for marker in &mut image_markers {
        if marker.alt_text.is_empty() {
            if !has_chat {
                anyhow::bail!(
                    "Slide {} has an image with no prompt (empty alt text). \
                     Chat capability is required for auto-prompting. \
                     Either add alt text or configure a chat provider.",
                    marker.slide_number
                );
            }
            marker.alt_text = auto_prompt(&presentation, marker.slide_number).await?;
        }
    }

    // Display plan
    if !force {
        println!(
            "Found {} image(s) and {} icon(s) to generate:",
            image_markers.len(),
            icon_markers.len()
        );
        for m in &image_markers {
            let prompt_preview = truncate(&m.alt_text, 60);
            println!(
                "  Slide {}: \"{}\" ({:?})",
                m.slide_number, prompt_preview, m.orientation
            );
        }
        for m in &icon_markers {
            let prompt_preview = truncate(&m.prompt_text, 60);
            println!(
                "  Slide {}: icon \"{}\" (Square)",
                m.slide_number, prompt_preview
            );
        }
        println!();

        let proceed = inquire::Confirm::new("Generate these images?")
            .with_default(true)
            .prompt()?;
        if !proceed {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Create output directories
    let images_dir = base_path.join("images");
    let icons_dir = base_path.join("media").join("diagram-icons");
    if !image_markers.is_empty() {
        std::fs::create_dir_all(&images_dir)?;
    }
    if !icon_markers.is_empty() {
        std::fs::create_dir_all(&icons_dir)?;
    }

    let total = image_markers.len() + icon_markers.len();
    let mut success_count = 0;

    // Track replacements: (line_index, old_text, new_text)
    let mut replacements: Vec<(usize, String, String)> = Vec::new();

    let client = ailloy::Client::for_capability("image")?;

    // Build all generation tasks, then run them concurrently
    use futures::stream::StreamExt;
    use std::io::Write;
    use std::pin::Pin;

    const MAX_CONCURRENT: usize = 4;

    let mut all_futures: Vec<Pin<Box<dyn Future<Output = GenerationResult> + '_>>> = Vec::new();

    for (i, marker) in image_markers.iter().enumerate() {
        let combined =
            prompt::build_image_prompt(&image_style, &marker.alt_text, marker.orientation);
        let client = &client;
        let alt = marker.alt_text.clone();
        let line_index = marker.line_index;
        let old_line = lines[marker.line_index].to_string();
        all_futures.push(Box::pin(async move {
            let result = client.generate_image(&combined).await;
            GenerationResult {
                index: i,
                prompt_text: alt,
                line_index,
                old_line,
                result,
                is_icon: false,
            }
        }));
    }

    let image_count = image_markers.len();
    for (i, marker) in icon_markers.iter().enumerate() {
        let combined = prompt::build_icon_prompt(&icon_style, &marker.prompt_text);
        let client = &client;
        let prompt_text = marker.prompt_text.clone();
        let line_index = marker.line_index;
        let old_line = lines[marker.line_index].to_string();
        all_futures.push(Box::pin(async move {
            let result = client.generate_image(&combined).await;
            GenerationResult {
                index: image_count + i,
                prompt_text,
                line_index,
                old_line,
                result,
                is_icon: true,
            }
        }));
    }

    println!(
        "  Generating {} image(s) with up to {} concurrent requests...\n",
        total, MAX_CONCURRENT
    );

    let mut buffered = futures::stream::iter(all_futures).buffer_unordered(MAX_CONCURRENT);

    while let Some(res) = buffered.next().await {
        let idx = res.index + 1;
        let kind = if res.is_icon { "icon " } else { "" };
        let prompt_preview = truncate(&res.prompt_text, 50);

        match res.result {
            Ok(response) => {
                let ext = ai::image_ext(&response.format);
                let (dir, prefix) = if res.is_icon {
                    (&icons_dir, "")
                } else {
                    (&images_dir, "images/")
                };
                let filename = generate_filename(has_chat, &res.prompt_text, ext, dir).await;
                let filepath = dir.join(&filename);
                std::fs::write(&filepath, &response.data)?;

                println!(
                    "  [{}/{}] {}{} {}",
                    idx,
                    total,
                    kind,
                    prompt_preview,
                    "✓".green().bold()
                );
                ai::display_image_result(&filepath);

                // Record replacement
                if res.is_icon {
                    let icon_name = filename
                        .strip_suffix(&format!(".{ext}"))
                        .unwrap_or(&filename);
                    let new_line = replace_icon_marker(&res.old_line, icon_name);
                    replacements.push((res.line_index, res.old_line.clone(), new_line));
                } else {
                    let new_line = res
                        .old_line
                        .replace("image-generation", &format!("{prefix}{filename}"));
                    replacements.push((res.line_index, res.old_line.clone(), new_line));
                }
                success_count += 1;
            }
            Err(e) => {
                println!(
                    "  [{}/{}] {}{} {}",
                    idx,
                    total,
                    kind,
                    prompt_preview,
                    "✗".red().bold()
                );
                eprintln!("    Error: {e}");
            }
        }
        let _ = std::io::stdout().flush();
    }

    // Rewrite the markdown file
    if success_count > 0 {
        let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        for (line_idx, _old, new) in &replacements {
            new_lines[*line_idx] = new.clone();
        }

        // Preserve original line ending style
        let line_ending = if content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let mut new_content = new_lines.join(line_ending);
        if content.ends_with('\n') || content.ends_with("\r\n") {
            new_content.push_str(line_ending);
        }
        std::fs::write(&file, new_content)?;
    }

    println!();
    if success_count == total {
        println!(
            "{} Generated {}/{} images successfully.",
            "✓".green().bold(),
            success_count,
            total
        );
    } else {
        println!(
            "{} Generated {}/{} images ({} failed).",
            "!".yellow().bold(),
            success_count,
            total,
            total - success_count
        );
    }
    if success_count > 0 {
        println!("Updated: {}", file.display());
    }

    Ok(())
}

// ── Scanning ─────────────────────────────────────────────────────────────────

fn scan_markers(
    lines: &[&str],
    presentation: &parser::Presentation,
) -> Result<(Vec<ImageMarker>, Vec<IconMarker>)> {
    let mut image_markers = Vec::new();
    let mut icon_markers = Vec::new();

    // Build a map from line ranges to slide info
    // We'll scan lines directly for the markers
    let mut current_slide = 0usize;
    let mut in_diagram = false;

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Track slide boundaries (rough heuristic based on separators and headings)
        // This is imperfect but good enough for mapping lines to slides
        if trimmed == "---" || trimmed.starts_with("# ") {
            // We might be at a new slide
        }

        // Check for image-generation markers: ![...](image-generation)
        if let Some(captures) = parse_image_gen_line(trimmed) {
            let slide_num = find_slide_for_line(line_idx, lines, presentation);
            let layout = if slide_num > 0 && slide_num <= presentation.slides.len() {
                presentation.slides[slide_num - 1].layout
            } else {
                Layout::Content
            };

            let orientation = orientation_for_layout(layout);

            image_markers.push(ImageMarker {
                line_index: line_idx,
                alt_text: captures,
                slide_number: slide_num,
                orientation,
            });
        }

        // Track diagram blocks
        if trimmed.starts_with("```") && trimmed.contains("@architecture") {
            in_diagram = true;
            current_slide = find_slide_for_line(line_idx, lines, presentation);
            continue;
        }
        if in_diagram && trimmed == "```" {
            in_diagram = false;
            continue;
        }

        // Check for icon markers inside diagrams
        if in_diagram && trimmed.contains("icon: generate-image") {
            if let Some(prompt_text) = extract_icon_prompt(trimmed) {
                icon_markers.push(IconMarker {
                    line_index: line_idx,
                    prompt_text,
                    slide_number: current_slide,
                });
            }
        }
    }

    Ok((image_markers, icon_markers))
}

/// Parse a line like `![alt text](image-generation)` and return the alt text.
fn parse_image_gen_line(line: &str) -> Option<String> {
    let line = line.trim();
    if !line.contains("](image-generation)") {
        return None;
    }
    // Extract alt text from ![alt](image-generation) possibly with directives after
    let start = line.find("![")?;
    let alt_start = start + 2;
    let alt_end = line[alt_start..].find(']')? + alt_start;
    let after_bracket = &line[alt_end + 1..];
    if after_bracket.starts_with("(image-generation)") {
        let alt = line[alt_start..alt_end].to_string();
        // Strip inline directives from alt text (e.g. "@fill")
        let alt = alt
            .split_whitespace()
            .filter(|w| !w.starts_with('@'))
            .collect::<Vec<_>>()
            .join(" ");
        Some(alt)
    } else {
        None
    }
}

/// Extract the prompt from a diagram line like `Gateway (icon: generate-image, prompt: "An API gateway", pos: 1,1)`.
fn extract_icon_prompt(line: &str) -> Option<String> {
    // Look for prompt: "..." or prompt: '...'
    let prompt_start = line.find("prompt:")?;
    let after = &line[prompt_start + "prompt:".len()..];
    let after = after.trim_start();

    let (quote, rest) = if let Some(stripped) = after.strip_prefix('"') {
        ('"', stripped)
    } else if let Some(stripped) = after.strip_prefix('\'') {
        ('\'', stripped)
    } else {
        return None;
    };

    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

/// Find which slide (1-indexed) a line belongs to by matching against raw source.
fn find_slide_for_line(
    line_idx: usize,
    lines: &[&str],
    presentation: &parser::Presentation,
) -> usize {
    // Build cumulative line count per slide using raw_source
    let mut offset = 0;
    // Skip frontmatter
    if !lines.is_empty() && lines[0].trim() == "---" {
        // Find closing ---
        for (i, line) in lines.iter().enumerate().skip(1) {
            if line.trim() == "---" {
                offset = i + 1;
                break;
            }
        }
    }

    let mut slide_start = offset;
    for (slide_idx, slide) in presentation.slides.iter().enumerate() {
        let slide_lines = slide.raw_source.lines().count();
        // Account for separators between slides (blank lines, ---)
        let slide_end = slide_start + slide_lines;
        if line_idx >= slide_start && line_idx < slide_end + 3 {
            return slide_idx + 1;
        }
        slide_start = slide_end;
        // Skip separator lines
        while slide_start < lines.len()
            && (lines[slide_start].trim().is_empty() || lines[slide_start].trim() == "---")
        {
            slide_start += 1;
        }
    }
    // Fallback
    presentation.slides.len().max(1)
}

fn orientation_for_layout(layout: Layout) -> Orientation {
    match layout {
        Layout::Image => Orientation::Horizontal,
        Layout::Bullet | Layout::Code | Layout::Quote | Layout::Content => {
            // These could be side-panel layouts if they have an image
            Orientation::Vertical
        }
        Layout::TwoColumn => Orientation::Vertical,
        _ => Orientation::Horizontal,
    }
}

// ── Style resolution ─────────────────────────────────────────────────────────

fn resolve_style<'a>(
    cli_override: &'a Option<String>,
    frontmatter: Option<&'a str>,
    config_default_name: Option<&str>,
    lookup: impl Fn(&str) -> Option<&'a str>,
    hardcoded: &'a str,
) -> String {
    // --style CLI flag
    if let Some(s) = cli_override {
        // Try as a named style first, otherwise use as literal
        if let Some(desc) = lookup(s) {
            return desc.to_string();
        }
        return s.to_string();
    }
    // @image-style frontmatter
    if let Some(s) = frontmatter {
        if let Some(desc) = lookup(s) {
            return desc.to_string();
        }
        return s.to_string();
    }
    // defaults.image_style config
    if let Some(name) = config_default_name {
        if let Some(desc) = lookup(name) {
            return desc.to_string();
        }
    }
    hardcoded.to_string()
}

// ── Auto-prompt ──────────────────────────────────────────────────────────────

async fn auto_prompt(presentation: &parser::Presentation, slide_number: usize) -> Result<String> {
    let slide = &presentation.slides[slide_number - 1];
    let raw = &slide.raw_source;

    let client = ailloy::Client::for_capability("chat")?;
    let response = client
        .chat(&[
            ailloy::Message::system(
                "You are a concise image prompt generator. Given slide content, \
                 generate a short, descriptive image prompt suitable for AI image generation. \
                 Respond with ONLY the prompt, no explanation.",
            ),
            ailloy::Message::user(format!(
                "Generate a concise image prompt for a presentation slide about:\n\n{raw}"
            )),
        ])
        .await?;

    Ok(response.content.trim().to_string())
}

// ── Filename generation ──────────────────────────────────────────────────────

async fn generate_filename(has_chat: bool, prompt_text: &str, ext: &str, dir: &Path) -> String {
    let base = if has_chat {
        chat_filename(prompt_text)
            .await
            .unwrap_or_else(|_| hex_filename())
    } else {
        hex_filename()
    };

    // Check for collisions
    let mut candidate = format!("{base}.{ext}");
    let mut counter = 2;
    while dir.join(&candidate).exists() {
        candidate = format!("{base}-{counter}.{ext}");
        counter += 1;
    }
    candidate
}

async fn chat_filename(prompt_text: &str) -> Result<String> {
    let client = ailloy::Client::for_capability("chat")?;
    let response = client
        .chat(&[
            ailloy::Message::system(
                "Generate a short kebab-case filename (no extension) for an image. \
                 Respond with ONLY the filename, 2-4 words, lowercase, hyphens between words. \
                 Example: golden-sunset",
            ),
            ailloy::Message::user(prompt_text),
        ])
        .await?;

    let name = response
        .content
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>();

    if name.is_empty() {
        Ok(hex_filename())
    } else {
        // Truncate to reasonable length
        Ok(name.chars().take(40).collect())
    }
}

fn hex_filename() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("image-{:08x}", (n & 0xFFFF_FFFF) as u32)
}

// ── Line replacement helpers ─────────────────────────────────────────────────

/// Replace `icon: generate-image, prompt: "..."` with `icon: <name>` in a diagram line.
fn replace_icon_marker(line: &str, icon_name: &str) -> String {
    // Strategy: replace `icon: generate-image` with `icon: <name>` and remove `prompt: "..."`
    let mut result = line.to_string();

    // Replace the icon value
    result = result.replace("icon: generate-image", &format!("icon: {icon_name}"));

    // Remove the prompt: "..." portion (with surrounding commas)
    if let Some(prompt_start) = result.find("prompt:") {
        // Find the extent of the prompt value including quotes
        let after = &result[prompt_start..];
        let colon_end = "prompt:".len();
        let after_colon = after[colon_end..].trim_start();
        let quote = after_colon.chars().next().unwrap_or(' ');
        if quote == '"' || quote == '\'' {
            let rest = &after_colon[1..];
            if let Some(end) = rest.find(quote) {
                let prompt_byte_end =
                    prompt_start + colon_end + (after_colon.len() - rest.len()) + end + 1;

                // Remove the prompt portion and any surrounding comma
                let before = &result[..prompt_start];
                let after = &result[prompt_byte_end..];

                // Clean up commas: ", prompt: ..." or "prompt: ..., "
                let before = before.trim_end_matches([',', ' ']);
                let after = after.trim_start_matches([',', ' ']);

                result = if before.is_empty() {
                    after.to_string()
                } else if after.is_empty() {
                    before.to_string()
                } else {
                    format!("{before}, {after}")
                };
            }
        }
    }

    result
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_image_gen_line() {
        assert_eq!(
            parse_image_gen_line("![a sunset over mountains](image-generation)"),
            Some("a sunset over mountains".to_string())
        );
        assert_eq!(
            parse_image_gen_line("![](image-generation)"),
            Some(String::new())
        );
        assert_eq!(parse_image_gen_line("![photo](photo.jpg)"), None);
        assert_eq!(
            parse_image_gen_line("![my prompt @fill](image-generation)"),
            Some("my prompt".to_string())
        );
    }

    #[test]
    fn test_extract_icon_prompt() {
        assert_eq!(
            extract_icon_prompt(
                "Gateway (icon: generate-image, prompt: \"An API gateway\", pos: 1,1)"
            ),
            Some("An API gateway".to_string())
        );
        assert_eq!(
            extract_icon_prompt("DB (icon: generate-image, prompt: 'A database icon')"),
            Some("A database icon".to_string())
        );
        assert_eq!(extract_icon_prompt("Server (icon: server, pos: 1,1)"), None);
    }

    #[test]
    fn test_replace_icon_marker() {
        let line = "- Gateway (icon: generate-image, prompt: \"An API gateway\", pos: 1,1)";
        let result = replace_icon_marker(line, "api-gateway");
        assert!(result.contains("icon: api-gateway"));
        assert!(!result.contains("prompt:"));
        assert!(!result.contains("generate-image"));
        assert!(result.contains("pos: 1,1"));
    }

    #[test]
    fn test_replace_icon_marker_no_prompt() {
        let line = "- Server (icon: generate-image, pos: 1,1)";
        let result = replace_icon_marker(line, "server-icon");
        assert_eq!(result, "- Server (icon: server-icon, pos: 1,1)");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("a very long string here", 10), "a very ...");
    }
}
