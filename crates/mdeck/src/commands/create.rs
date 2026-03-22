//! `mdeck ai create` — create a presentation from content using AI.
//!
//! Accepts text, markdown, PDF, or DOCX input and generates a complete
//! mdeck-format presentation with speaker notes, visualizations, and
//! image generation markers.

use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use futures::StreamExt;

use crate::cli::CreateArgs;
use crate::commands::ai;

const APP_NAME: &str = "mdeck";

/// The full mdeck format specification, embedded at compile time.
const MDECK_SPEC: &str = include_str!("../../doc/mdeck-spec.md");

// ── Entry point ─────────────────────────────────────────────────────────────

pub async fn run(args: CreateArgs, quiet: bool) -> Result<()> {
    if !ai::has_capability("chat") {
        anyhow::bail!(
            "Chat AI not configured. Run `{APP_NAME} ai config` to set up a chat provider."
        );
    }

    if !quiet {
        eprintln!("{}", "MDeck AI Presentation Creator".bold());
        eprintln!();
    }

    // Step 1: Resolve and extract input content
    let (source_label, content) = resolve_input(&args)?;
    let word_count = content.split_whitespace().count();

    if content.trim().is_empty() {
        anyhow::bail!("No content found in input. Please provide non-empty content.");
    }

    if !quiet {
        eprintln!(
            "  {} {} ({} words)",
            "Input:".bold(),
            source_label,
            word_count
        );
    }

    // Step 2: Interactive mode — gather additional context
    let user_prompt = if args.interactive {
        gather_interactive_context(&args, &source_label, word_count)?
    } else {
        args.prompt.clone()
    };

    // Step 3: Analyze content and create outline
    if !quiet {
        eprintln!();
        eprintln!("{}", "Analyzing content...".bold());
        eprintln!();
    }

    let client = ailloy::Client::for_capability("chat")?;
    let outline = run_analysis(&client, &content, user_prompt.as_deref(), quiet).await?;

    // Step 4: Interactive confirmation
    if args.interactive {
        eprintln!();
        eprint!("{} Generate this presentation? [Y/n] ", "?".green().bold());
        io::stderr().flush()?;
        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm)?;
        let confirm = confirm.trim().to_lowercase();
        if confirm == "n" || confirm == "no" {
            eprintln!("{} Cancelled.", "!".yellow().bold());
            return Ok(());
        }
    }

    // Step 5: Generate slide content
    if !quiet {
        eprintln!();
        eprintln!("{}", "Generating presentation...".bold());
        eprintln!();
    }

    let presentation_md = run_generation(&client, &outline, user_prompt.as_deref(), quiet).await?;

    // Step 6: Write output
    let (output_file, output_dir) = resolve_output(&args.output)?;
    std::fs::create_dir_all(&output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    // Check for existing file
    if output_file.exists() && !args.interactive {
        eprintln!(
            "{} Output file already exists: {}",
            "!".yellow().bold(),
            output_file.display()
        );
        eprintln!("  Overwriting...");
    }

    std::fs::write(&output_file, &presentation_md)
        .with_context(|| format!("Failed to write output: {}", output_file.display()))?;

    if !quiet {
        eprintln!();
        eprintln!(
            "{} Presentation created: {}",
            "✓".green().bold(),
            output_file.display()
        );
    }

    // Step 7: Handle visualization opportunities
    let opportunities = extract_opportunities(&outline);
    if !opportunities.is_empty() {
        let opp_file = output_dir.join("visualization-opportunities.md");
        write_opportunities(&opp_file, &opportunities)?;
        if !quiet {
            eprintln!(
                "  {} Found {} visualization {} not yet supported by mdeck.",
                "ℹ".blue().bold(),
                opportunities.len(),
                if opportunities.len() == 1 {
                    "opportunity"
                } else {
                    "opportunities"
                }
            );
            eprintln!("    See: {}", opp_file.display());
            eprintln!(
                "    Consider sharing this as an issue at: {}",
                "https://github.com/mklab-se/mdeck/issues/new".cyan()
            );
        }
    }

    // Step 8: Check for image generation markers
    let image_count = presentation_md.matches("(image-generation)").count();
    if image_count > 0 {
        if ai::has_capability("image") {
            if !quiet {
                eprintln!(
                    "  {} {} image{} marked for AI generation.",
                    "ℹ".blue().bold(),
                    image_count,
                    if image_count == 1 { "" } else { "s" }
                );
                eprintln!(
                    "    Run: {} to generate them.",
                    format!("mdeck ai generate {}", output_file.display()).cyan()
                );
            }
        } else if !quiet {
            eprintln!(
                "  {} {} image{} marked for generation, but no image provider configured.",
                "ℹ".blue().bold(),
                image_count,
                if image_count == 1 { "" } else { "s" }
            );
            eprintln!("    Run `{APP_NAME} ai config` to add an image provider, then:");
            eprintln!(
                "    {}",
                format!("mdeck ai generate {}", output_file.display()).cyan()
            );
        }
    }

    // Suggest launching the presentation
    if !quiet {
        eprintln!();
        eprintln!(
            "  Launch: {}",
            format!("mdeck {}", output_file.display()).cyan()
        );
    }

    Ok(())
}

// ── Input resolution ────────────────────────────────────────────────────────

/// Resolve the input source and extract text content.
/// Returns (source_label, extracted_text).
fn resolve_input(args: &CreateArgs) -> Result<(String, String)> {
    if let Some(ref input) = args.input {
        // Check if it's a file path
        let path = Path::new(input);
        if path.exists() && path.is_file() {
            let label = format!("{}", path.display());
            let content = extract_from_file(path)?;
            return Ok((label, content));
        }
        // Treat as literal text
        return Ok(("(text input)".to_string(), input.clone()));
    }

    // Try stdin if it's piped
    if !io::stdin().is_terminal() {
        let mut content = String::new();
        io::stdin()
            .read_to_string(&mut content)
            .context("Failed to read from stdin")?;
        if content.trim().is_empty() {
            anyhow::bail!("No content received from stdin.");
        }
        return Ok(("(stdin)".to_string(), content));
    }

    // No input provided — show help (same as --help)
    use clap::CommandFactory;
    let mut cmd = crate::cli::Cli::command();
    // Navigate to: mdeck → ai → create
    for sub in cmd.get_subcommands_mut() {
        if sub.get_name() == "ai" {
            for sub2 in sub.get_subcommands_mut() {
                if sub2.get_name() == "create" {
                    sub2.clone().name("mdeck ai create").print_help()?;
                    println!();
                    std::process::exit(0);
                }
            }
        }
    }
    anyhow::bail!("No input provided. Run `mdeck ai create --help` for usage.");
}

/// Extract text content from a file based on its extension.
fn extract_from_file(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "pdf" => extract_pdf(path),
        "docx" => extract_docx(path),
        _ => {
            // Assume text-based file (md, txt, etc.)
            std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read file: {}", path.display()))
        }
    }
}

/// Extract text from a PDF file.
fn extract_pdf(path: &Path) -> Result<String> {
    let bytes =
        std::fs::read(path).with_context(|| format!("Failed to read PDF: {}", path.display()))?;
    let text = pdf_extract::extract_text_from_mem(&bytes)
        .with_context(|| format!("Failed to extract text from PDF: {}", path.display()))?;

    if text.trim().len() < 50 {
        eprintln!(
            "  {} PDF text extraction yielded very little content ({} chars).",
            "!".yellow().bold(),
            text.trim().len()
        );
        eprintln!("    The PDF may contain images or scanned text that cannot be extracted.");
    }

    Ok(text)
}

/// Extract text from a DOCX file.
///
/// DOCX files are ZIP archives containing XML. We extract text from
/// `word/document.xml` by collecting content within `<w:t>` tags,
/// adding newlines at `<w:p>` boundaries (paragraphs).
fn extract_docx(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open DOCX: {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("Failed to read DOCX as ZIP: {}", path.display()))?;

    let mut doc_xml = String::new();
    {
        let mut doc_entry = archive
            .by_name("word/document.xml")
            .with_context(|| format!("No word/document.xml in DOCX: {}", path.display()))?;
        io::Read::read_to_string(&mut doc_entry, &mut doc_xml)?;
    }

    let text = extract_text_from_docx_xml(&doc_xml);

    if text.trim().len() < 50 {
        eprintln!(
            "  {} DOCX text extraction yielded very little content ({} chars).",
            "!".yellow().bold(),
            text.trim().len()
        );
    }

    Ok(text)
}

/// Extract plain text from DOCX XML content.
///
/// Collects text within `<w:t>` tags, inserting newlines at `</w:p>` paragraph boundaries.
/// Ignores `<w:tbl>` to avoid confusion with `<w:t>`.
fn extract_text_from_docx_xml(xml: &str) -> String {
    let mut text = String::new();
    let mut in_text_tag = false;

    let mut chars = xml.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            for tc in chars.by_ref() {
                if tc == '>' {
                    break;
                }
                tag.push(tc);
            }

            let tag_trimmed = tag.trim();
            if tag_trimmed.starts_with("w:t") && !tag_trimmed.starts_with("w:tbl") {
                in_text_tag = !tag_trimmed.ends_with('/');
            } else if tag_trimmed == "/w:t" {
                in_text_tag = false;
            } else if tag_trimmed == "/w:p" {
                text.push('\n');
            }
        } else if in_text_tag {
            text.push(c);
        }
    }

    text
}

// ── Interactive mode ────────────────────────────────────────────────────────

/// Gather additional context from the user in interactive mode.
fn gather_interactive_context(
    args: &CreateArgs,
    source_label: &str,
    word_count: usize,
) -> Result<Option<String>> {
    eprintln!(
        "\n  {} {} ({} words)",
        "Source:".bold(),
        source_label,
        word_count
    );

    let mut context_parts: Vec<String> = Vec::new();

    // Use existing prompt if provided
    if let Some(ref prompt) = args.prompt {
        eprintln!("  {} {}", "Context:".bold(), prompt);
        context_parts.push(prompt.clone());
    }

    // Ask about audience
    eprintln!();
    eprint!(
        "{} Who is the target audience? (press Enter to skip) ",
        "?".green().bold()
    );
    io::stderr().flush()?;
    let mut audience = String::new();
    io::stdin().read_line(&mut audience)?;
    let audience = audience.trim();
    if !audience.is_empty() {
        context_parts.push(format!("Target audience: {audience}"));
    }

    // Ask about purpose
    eprint!(
        "{} What is the purpose of this presentation? (press Enter to skip) ",
        "?".green().bold()
    );
    io::stderr().flush()?;
    let mut purpose = String::new();
    io::stdin().read_line(&mut purpose)?;
    let purpose = purpose.trim();
    if !purpose.is_empty() {
        context_parts.push(format!("Purpose: {purpose}"));
    }

    // Ask about emphasis
    eprint!(
        "{} Any specific points to emphasize? (press Enter to skip) ",
        "?".green().bold()
    );
    io::stderr().flush()?;
    let mut emphasis = String::new();
    io::stdin().read_line(&mut emphasis)?;
    let emphasis = emphasis.trim();
    if !emphasis.is_empty() {
        context_parts.push(format!("Key emphasis: {emphasis}"));
    }

    // Summary
    if context_parts.is_empty() {
        eprintln!("  {} Using defaults (general audience).", "ℹ".blue().bold());
        Ok(None)
    } else {
        let combined = context_parts.join("\n");
        eprintln!();
        eprintln!("{}", "  Presentation context:".bold());
        for part in &context_parts {
            eprintln!("    • {part}");
        }
        Ok(Some(combined))
    }
}

// ── AI pipeline ─────────────────────────────────────────────────────────────

const ANALYSIS_SYSTEM_PROMPT: &str = "\
You are a presentation architect for mdeck, a markdown-based presentation tool. \
Your job is to analyze source content and design a presentation outline.

IMPORTANT RULES:
- Create a concise, engaging presentation — NOT a verbatim reproduction of the source.
- Think of the source material as detailed reference that could be handed out AFTER the talk.
- The presentation should support a PRESENTER — keep slides focused and visual.
- Each slide should cover ONE key point or a small group of closely related points.
- Never overload a slide with information. Less is more.
- Use progressive reveal (bullet points shown one at a time) where appropriate.
- Identify where visualizations (charts, diagrams, timelines, etc.) would enhance understanding.

Respond with a structured outline in this exact JSON format:
```json
{
  \"title\": \"Presentation Title\",
  \"slides\": [
    {
      \"title\": \"Slide Title\",
      \"key_points\": [\"point 1\", \"point 2\"],
      \"layout_hint\": \"bullet|code|quote|visualization|image|title|section|two-column\",
      \"visualization\": null,
      \"notes_hint\": \"Brief description of what the presenter should say\"
    }
  ],
  \"opportunities\": [
    {
      \"slide_title\": \"Which slide\",
      \"description\": \"What visualization would help\",
      \"suggested_format\": \"How it could be implemented\"
    }
  ]
}
```

For the `visualization` field, use one of these mdeck-supported types when appropriate:
- barchart, linechart, piechart, donut, stackedbar, scatter (data charts)
- timeline, gantt (temporal)
- orgchart, architecture (structural)
- kpi, progress, funnel (metrics)
- radar, venn (comparison)
- wordcloud (text analysis)

If a visualization would be useful but is NOT in the list above, add it to `opportunities` instead.

Keep the outline to 8-20 slides for most content. Start with a title slide and end with a summary/conclusion.";

/// Run the content analysis step.
async fn run_analysis(
    client: &ailloy::Client,
    content: &str,
    user_prompt: Option<&str>,
    quiet: bool,
) -> Result<String> {
    let mut user_message = String::new();

    if let Some(prompt) = user_prompt {
        user_message.push_str(&format!("PRESENTATION CONTEXT:\n{prompt}\n\n"));
    }

    user_message.push_str("SOURCE CONTENT:\n");

    // Truncate very long content with a warning
    const MAX_CONTENT_CHARS: usize = 100_000;
    if content.len() > MAX_CONTENT_CHARS {
        if !quiet {
            eprintln!(
                "  {} Content is very large ({} chars). Truncating to {} chars.",
                "!".yellow().bold(),
                content.len(),
                MAX_CONTENT_CHARS
            );
        }
        user_message.push_str(&content[..MAX_CONTENT_CHARS]);
        user_message.push_str("\n\n[Content truncated — focus on the content provided above.]");
    } else {
        user_message.push_str(content);
    }

    let messages = vec![
        ailloy::Message::system(ANALYSIS_SYSTEM_PROMPT),
        ailloy::Message::user(&user_message),
    ];

    let response = stream_response(client, &messages, quiet).await?;
    Ok(response)
}

/// Run the slide generation step.
async fn run_generation(
    client: &ailloy::Client,
    outline: &str,
    user_prompt: Option<&str>,
    quiet: bool,
) -> Result<String> {
    let system_prompt = format!(
        "You are a presentation content generator for mdeck. \
        Given a slide outline, generate a complete presentation in mdeck markdown format.\n\n\
        MDECK FORMAT SPECIFICATION:\n{MDECK_SPEC}\n\n\
        CRITICAL RULES:\n\
        - Generate valid mdeck markdown that can be directly rendered.\n\
        - Start with YAML frontmatter (title, author, @theme, @transition).\n\
        - Use `---` to separate slides.\n\
        - Include speaker notes after `???` on EVERY slide. Notes should explain:\n\
          • The intention and purpose of the slide\n\
          • Key talking points for the presenter\n\
          • Suggested delivery approach (questions to ask, pauses, emphasis)\n\
        - Use progressive reveal (`+` markers) for bullet lists where it helps pacing.\n\
        - Use visualization code blocks (```@barchart, ```@timeline, etc.) where the outline specifies.\n\
        - Mark images with `![descriptive alt text](image-generation)` for later AI generation.\n\
        - Keep text concise — this is a presentation, not a document.\n\
        - Use appropriate heading levels (# for slide titles).\n\
        - Use **bold** and *italic* for emphasis in slide content.\n\
        - Output ONLY the markdown content. No explanations or commentary outside the markdown."
    );

    let mut user_message =
        String::from("Generate a complete mdeck presentation from this outline:\n\n");
    user_message.push_str(outline);

    if let Some(prompt) = user_prompt {
        user_message.push_str(&format!("\n\nADDITIONAL CONTEXT:\n{prompt}"));
    }

    let messages = vec![
        ailloy::Message::system(&system_prompt),
        ailloy::Message::user(&user_message),
    ];

    let response = stream_response(client, &messages, quiet).await?;

    // Strip markdown code fences if the AI wrapped the output
    let cleaned = strip_markdown_fences(&response);
    Ok(cleaned)
}

/// Stream a chat response, optionally printing tokens to stderr.
async fn stream_response(
    client: &ailloy::Client,
    messages: &[ailloy::Message],
    quiet: bool,
) -> Result<String> {
    let mut stream = client.chat_stream(messages).await?;
    let mut assembled = String::new();

    while let Some(event) = stream.next().await {
        match event? {
            ailloy::StreamEvent::Delta(text) => {
                assembled.push_str(&text);
                if !quiet {
                    eprint!("{text}");
                    io::stderr().flush()?;
                }
            }
            ailloy::StreamEvent::Done(_) => {
                if !quiet {
                    eprintln!();
                }
            }
        }
    }

    Ok(assembled)
}

/// Strip markdown code fences if the AI wrapped the response in ```markdown ... ```.
fn strip_markdown_fences(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```markdown") {
        if let Some(content) = rest.strip_suffix("```") {
            return content.trim().to_string();
        }
    }
    if let Some(rest) = trimmed.strip_prefix("```md") {
        if let Some(content) = rest.strip_suffix("```") {
            return content.trim().to_string();
        }
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some(content) = rest.strip_suffix("```") {
            // Only strip if the first line after ``` is empty or looks like frontmatter
            let first_line = content.lines().next().unwrap_or("");
            if first_line.trim().is_empty() || first_line.trim() == "---" {
                return content.trim().to_string();
            }
        }
    }
    trimmed.to_string()
}

// ── Output resolution ───────────────────────────────────────────────────────

/// Resolve the output path into (markdown_file, output_directory).
fn resolve_output(output: &Path) -> Result<(PathBuf, PathBuf)> {
    let output_str = output.to_string_lossy();

    // If it ends with / or is an existing directory, put presentation.md inside
    if output_str.ends_with('/') || output_str.ends_with('\\') || output.is_dir() {
        let dir = output.to_path_buf();
        let file = dir.join("presentation.md");
        Ok((file, dir))
    } else if output
        .extension()
        .is_some_and(|ext| ext == "md" || ext == "markdown")
    {
        // It's a .md file
        let dir = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf();
        Ok((output.to_path_buf(), dir))
    } else {
        // Treat as directory
        let dir = output.to_path_buf();
        let file = dir.join("presentation.md");
        Ok((file, dir))
    }
}

// ── Visualization opportunities ─────────────────────────────────────────────

/// Extract [OPPORTUNITY: ...] markers from the outline.
fn extract_opportunities(outline: &str) -> Vec<String> {
    let mut opportunities = Vec::new();

    // Look for the "opportunities" array in the JSON
    if let Some(start) = outline.find("\"opportunities\"") {
        if let Some(arr_start) = outline[start..].find('[') {
            let arr_content = &outline[start + arr_start..];
            // Find matching closing bracket
            let mut depth = 0;
            let mut end = 0;
            for (i, c) in arr_content.chars().enumerate() {
                match c {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if end > 0 {
                let arr_str = &arr_content[..end];
                // Simple extraction of description fields
                for line in arr_str.lines() {
                    let trimmed = line.trim().trim_matches(['"', ',']);
                    if let Some(desc) = trimmed.strip_prefix("description") {
                        let desc = desc.trim_start_matches(['"', ':', ' ']);
                        let desc = desc.trim_end_matches(['"', ',']);
                        if !desc.is_empty() {
                            opportunities.push(desc.to_string());
                        }
                    }
                }
            }
        }
    }

    opportunities
}

/// Write visualization opportunities to a file in GitHub-issue-ready format.
fn write_opportunities(path: &Path, opportunities: &[String]) -> Result<()> {
    let mut content = String::from(
        "# Visualization Opportunities\n\n\
         The following visualizations were identified as potentially useful for this presentation\n\
         but are not currently supported by mdeck.\n\n\
         If you find any of these valuable, consider opening a GitHub issue to request support.\n\n",
    );

    for (i, opp) in opportunities.iter().enumerate() {
        content.push_str(&format!("## {}. {}\n\n", i + 1, opp));
        content.push_str(
            "**Requested by:** `mdeck ai create` (auto-detected during presentation generation)\n\n",
        );
        content.push_str("---\n\n");
    }

    content.push_str(
        "To request a new visualization type: https://github.com/mklab-se/mdeck/issues/new\n",
    );

    std::fs::write(path, content)
        .with_context(|| format!("Failed to write opportunities: {}", path.display()))?;

    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_output_md_file() {
        let (file, dir) = resolve_output(Path::new("slides.md")).unwrap();
        assert_eq!(file, PathBuf::from("slides.md"));
        assert_eq!(dir, PathBuf::from("."));
    }

    #[test]
    fn test_resolve_output_directory_slash() {
        let (file, dir) = resolve_output(Path::new("output/")).unwrap();
        assert_eq!(file, PathBuf::from("output/presentation.md"));
        assert_eq!(dir, PathBuf::from("output"));
    }

    #[test]
    fn test_resolve_output_no_extension() {
        let (file, dir) = resolve_output(Path::new("my-presentation")).unwrap();
        assert_eq!(file, PathBuf::from("my-presentation/presentation.md"));
        assert_eq!(dir, PathBuf::from("my-presentation"));
    }

    #[test]
    fn test_resolve_output_nested_path() {
        let (file, dir) = resolve_output(Path::new("dir/subdir/pres.md")).unwrap();
        assert_eq!(file, PathBuf::from("dir/subdir/pres.md"));
        assert_eq!(dir, PathBuf::from("dir/subdir"));
    }

    #[test]
    fn test_strip_markdown_fences_wrapped() {
        let input = "```markdown\n---\ntitle: Test\n---\n# Slide\n```";
        let result = strip_markdown_fences(input);
        assert!(result.starts_with("---"));
        assert!(!result.contains("```"));
    }

    #[test]
    fn test_strip_markdown_fences_unwrapped() {
        let input = "---\ntitle: Test\n---\n# Slide";
        let result = strip_markdown_fences(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_strip_markdown_fences_md() {
        let input = "```md\n---\ntitle: Test\n---\n```";
        let result = strip_markdown_fences(input);
        assert!(result.starts_with("---"));
    }

    #[test]
    fn test_extract_opportunities_empty() {
        let outline = r#"{"slides": [], "opportunities": []}"#;
        assert!(extract_opportunities(outline).is_empty());
    }

    #[test]
    fn test_extract_opportunities_found() {
        let outline = r#"{
            "opportunities": [
                {
                    "slide_title": "Data Flow",
                    "description": "Swimlane diagram showing cross-team workflow",
                    "suggested_format": "Horizontal lanes with arrows"
                }
            ]
        }"#;
        let opps = extract_opportunities(outline);
        assert_eq!(opps.len(), 1);
        assert!(opps[0].contains("Swimlane"));
    }

    // ── DOCX XML parsing tests ──────────────────────────────────────────────

    #[test]
    fn test_docx_xml_basic_paragraph() {
        let xml = r#"<w:body><w:p><w:r><w:t>Hello world</w:t></w:r></w:p></w:body>"#;
        let text = extract_text_from_docx_xml(xml);
        assert_eq!(text.trim(), "Hello world");
    }

    #[test]
    fn test_docx_xml_multiple_paragraphs() {
        let xml = r#"<w:body><w:p><w:r><w:t>First</w:t></w:r></w:p><w:p><w:r><w:t>Second</w:t></w:r></w:p></w:body>"#;
        let text = extract_text_from_docx_xml(xml);
        let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines, vec!["First", "Second"]);
    }

    #[test]
    fn test_docx_xml_multiple_runs() {
        let xml = r#"<w:p><w:r><w:t>Hello </w:t></w:r><w:r><w:t>world</w:t></w:r></w:p>"#;
        let text = extract_text_from_docx_xml(xml);
        assert_eq!(text.trim(), "Hello world");
    }

    #[test]
    fn test_docx_xml_text_with_attributes() {
        // <w:t xml:space="preserve"> is common in real DOCX files
        let xml = r#"<w:p><w:r><w:t xml:space="preserve">Preserved text</w:t></w:r></w:p>"#;
        let text = extract_text_from_docx_xml(xml);
        assert_eq!(text.trim(), "Preserved text");
    }

    #[test]
    fn test_docx_xml_ignores_non_text_tags() {
        let xml = r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Bold text</w:t></w:r></w:p>"#;
        let text = extract_text_from_docx_xml(xml);
        assert_eq!(text.trim(), "Bold text");
    }

    #[test]
    fn test_docx_xml_table_not_confused_with_text() {
        // <w:tbl> should not be confused with <w:t>
        let xml = r#"<w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body>"#;
        let text = extract_text_from_docx_xml(xml);
        assert!(
            text.contains("Cell"),
            "Text inside table cells should still be extracted"
        );
    }

    #[test]
    fn test_docx_xml_empty_document() {
        let xml = r#"<w:body></w:body>"#;
        let text = extract_text_from_docx_xml(xml);
        assert!(text.trim().is_empty());
    }

    #[test]
    fn test_docx_xml_self_closing_text_tag() {
        // Self-closing <w:t/> should not start text capture
        let xml = r#"<w:p><w:r><w:t/>Outside text</w:r></w:p>"#;
        let text = extract_text_from_docx_xml(xml);
        assert!(
            !text.contains("Outside"),
            "Self-closing <w:t/> should not capture text outside the tag"
        );
    }

    // ── File extension routing tests ────────────────────────────────────────

    #[test]
    fn test_resolve_input_literal_text() {
        let args = CreateArgs {
            input: Some("A presentation about Rust programming".to_string()),
            output: PathBuf::from("out.md"),
            prompt: None,
            interactive: false,
            style: None,
        };
        let (label, content) = resolve_input(&args).unwrap();
        assert_eq!(label, "(text input)");
        assert_eq!(content, "A presentation about Rust programming");
    }

    #[test]
    fn test_resolve_input_existing_file() {
        // Use Cargo.toml as a known file that always exists relative to the crate root
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let cargo_toml = format!("{manifest_dir}/Cargo.toml");
        let args = CreateArgs {
            input: Some(cargo_toml.clone()),
            output: PathBuf::from("out.md"),
            prompt: None,
            interactive: false,
            style: None,
        };
        let (label, content) = resolve_input(&args).unwrap();
        assert!(label.contains("Cargo.toml"));
        assert!(content.contains("mdeck"));
    }

    #[test]
    fn test_resolve_input_no_input_no_stdin() {
        let args = CreateArgs {
            input: None,
            output: PathBuf::from("out.md"),
            prompt: None,
            interactive: false,
            style: None,
        };
        // This should error when stdin is a terminal
        let result = resolve_input(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_output_markdown_extension() {
        let (file, _dir) = resolve_output(Path::new("talk.markdown")).unwrap();
        assert_eq!(file, PathBuf::from("talk.markdown"));
    }

    // ── Strip markdown fences edge cases ────────────────────────────────────

    #[test]
    fn test_strip_fences_generic_wrapper() {
        // Generic ``` wrapping (common AI output) gets stripped when first line is empty
        let input = "```\nfunction foo() {}\n```";
        let result = strip_markdown_fences(input);
        assert_eq!(result, "function foo() {}");
    }

    #[test]
    fn test_strip_fences_code_with_language() {
        // ``` with a language tag that isn't md/markdown should NOT be stripped
        let input = "```rust\nfn main() {}\n```";
        let result = strip_markdown_fences(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_strip_fences_with_whitespace() {
        let input = "  ```markdown\n---\ntitle: Test\n---\n```  ";
        let result = strip_markdown_fences(input);
        assert!(result.starts_with("---"));
    }

    // ── Multiple opportunities extraction ───────────────────────────────────

    #[test]
    fn test_extract_opportunities_multiple() {
        let outline = r#"{
            "opportunities": [
                {
                    "description": "Swimlane diagram"
                },
                {
                    "description": "Sankey chart"
                }
            ]
        }"#;
        let opps = extract_opportunities(outline);
        assert_eq!(opps.len(), 2);
        assert!(opps[0].contains("Swimlane"));
        assert!(opps[1].contains("Sankey"));
    }

    #[test]
    fn test_extract_opportunities_no_opportunities_key() {
        let outline = r#"{"slides": [{"title": "Intro"}]}"#;
        assert!(extract_opportunities(outline).is_empty());
    }
}
