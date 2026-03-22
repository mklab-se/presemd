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

    // Step 1: Resolve input content
    let (source_label, content) = resolve_input(&args, quiet)?;
    let word_count = content.split_whitespace().count();

    if content.trim().is_empty() {
        anyhow::bail!("No content found in input. Please provide non-empty content.");
    }

    // Show input info for file/stdin sources, but not for text the user just typed
    if !quiet && source_label != "(text input)" {
        eprintln!(
            "  {} {} ({} words)",
            "Input:".bold(),
            source_label,
            word_count
        );
    }

    let client = ailloy::Client::for_capability("chat")?;

    // Step 2: Interactive mode — AI-driven conversation to shape the presentation
    let context = if args.interactive {
        run_interactive_chat(&client, &content, args.prompt.as_deref(), quiet).await?
    } else {
        // Non-interactive: use --prompt directly or a sensible default
        args.prompt
            .clone()
            .unwrap_or_else(|| "General audience. Focus on key takeaways.".to_string())
    };

    // Step 3: Determine output filename
    let output_file = if args.output == Path::new("presentation.md") && !quiet {
        let suggested = suggest_filename(&client, &context).await?;
        let (file, _) = resolve_output(Path::new(&suggested))?;
        file
    } else {
        let (file, _) = resolve_output(&args.output)?;
        file
    };
    let output_dir = output_file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();

    // Step 4: Confirmation — show what will be created and ask for approval
    if !quiet {
        eprintln!();
        eprintln!("{}", "  Ready to generate:".bold());
        eprintln!("    {} {}", "File:".bold(), output_file.display());
        eprintln!();
    }

    if args.interactive {
        eprint!("{} Proceed with generation? [Y/n] ", "?".green().bold());
        io::stderr().flush()?;
        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm)?;
        let confirm = confirm.trim().to_lowercase();
        if confirm == "n" || confirm == "no" {
            eprintln!("{} Cancelled.", "!".yellow().bold());
            return Ok(());
        }
    }

    // Step 5: Generate the presentation
    let (presentation_md, opportunities) =
        run_pipeline(&client, &content, &context, &args.style, quiet).await?;

    // Step 6: Write output
    std::fs::create_dir_all(&output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    std::fs::write(&output_file, &presentation_md)
        .with_context(|| format!("Failed to write output: {}", output_file.display()))?;

    if !quiet {
        eprintln!(
            "{} Presentation created: {}",
            "✓".green().bold(),
            output_file.display()
        );
    }

    // Step 7: Auto-generate images if image capability is available
    let image_count = presentation_md.matches("(image-generation)").count();
    if image_count > 0 {
        if ai::has_capability("image") {
            if !quiet {
                eprintln!();
                eprintln!(
                    "  {} Generating {} image{}...",
                    "ℹ".blue().bold(),
                    image_count,
                    if image_count == 1 { "" } else { "s" }
                );
            }
            // Run generate with quiet=true to suppress inline image display in terminal
            crate::commands::generate::run(output_file.clone(), true, args.style.clone(), true)
                .await?;
            if !quiet {
                eprintln!(
                    "  {} {} image{} generated.",
                    "✓".green().bold(),
                    image_count,
                    if image_count == 1 { "" } else { "s" }
                );
            }
        } else if !quiet {
            eprintln!(
                "  {} {} image{} marked but no image provider configured.",
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

    if !quiet {
        eprintln!();
        eprintln!(
            "  Launch: {}",
            format!("mdeck {}", output_file.display()).cyan()
        );
    }

    // Step 8: Visualization opportunities — shown last as a warning
    if !opportunities.is_empty() && !quiet {
        let opp_file = output_dir.join("visualization-opportunities.md");
        write_opportunities(&opp_file, &opportunities)?;
        eprintln!();
        eprintln!(
            "  {} This presentation could be even better.",
            "!".yellow().bold(),
        );
        eprintln!(
            "    MDeck identified {} visualization{} that would enhance the slides",
            opportunities.len(),
            if opportunities.len() == 1 { "" } else { "s" },
        );
        eprintln!(
            "    but {} not yet supported.",
            if opportunities.len() == 1 {
                "is"
            } else {
                "are"
            }
        );
        eprintln!();
        eprintln!(
            "    The file {} contains detailed feature request{}",
            opp_file.display().to_string().cyan(),
            if opportunities.len() == 1 { "" } else { "s" },
        );
        eprintln!(
            "    ready to be copied into a GitHub issue. By sharing {} you help",
            if opportunities.len() == 1 {
                "it,"
            } else {
                "them,"
            }
        );
        eprintln!("    yourself and the MDeck community.");
        eprintln!();
        eprintln!(
            "    {}",
            "https://github.com/mklab-se/mdeck/issues/new".cyan()
        );
    }

    Ok(())
}

// ── Input resolution ────────────────────────────────────────────────────────

/// Resolve the input source and extract text content.
/// Returns (source_label, extracted_text).
fn resolve_input(args: &CreateArgs, quiet: bool) -> Result<(String, String)> {
    if let Some(ref input) = args.input {
        let path = Path::new(input);
        if path.exists() && path.is_file() {
            let label = format!("{}", path.display());
            let content = extract_from_file(path)?;
            return Ok((label, content));
        }
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

    // Interactive mode — ask for input
    if args.interactive {
        if !quiet {
            eprintln!(
                "{} What should the presentation be about?",
                "?".green().bold()
            );
            eprintln!("  Enter a file path, or describe the topic in your own words.");
            eprintln!();
        }
        eprint!("{} ", ">".bold());
        io::stderr().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();
        if input.is_empty() {
            anyhow::bail!("No input provided.");
        }
        let path = Path::new(&input);
        if path.exists() && path.is_file() {
            let label = format!("{}", path.display());
            let content = extract_from_file(path)?;
            return Ok((label, content));
        }
        return Ok(("(text input)".to_string(), input));
    }

    // No input provided — show help
    use clap::CommandFactory;
    let mut cmd = crate::cli::Cli::command();
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
        _ => std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display())),
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

// ── Interactive AI chat ─────────────────────────────────────────────────────

const INTERACTIVE_SYSTEM_PROMPT: &str = "\
You are a presentation design consultant for mdeck, a markdown-based presentation tool. \
You're having a conversation with someone who wants to create a presentation. \
Your goal is to understand what they need so you can create the best possible presentation.

Through natural conversation, learn about:
- Who the audience is (technical level, relationship to the topic)
- What the goal of the presentation is (inform, persuade, teach, decide)
- What key messages they want the audience to take away
- The tone and style (formal, casual, technical, inspirational)
- How long the presentation should be (number of slides)
- Any specific content they want included or excluded

Be conversational and helpful — ask one or two questions at a time, not a list. \
Build on what they tell you. If they provided source content, reference it specifically.

When you feel you have enough information to create a great presentation, \
summarize what you've agreed on in 2-3 concise paragraphs and end with exactly this marker:

[READY]

The summary before [READY] should cover: topic, audience, goal, key messages, tone, and approximate length. \
This summary will be used to guide the presentation generation.

If the user says /start or wants to proceed before you're fully ready, \
write your best summary with what you know and include [READY].

Keep your responses concise — this is a terminal chat, not an essay.";

/// Run an interactive AI chat to gather presentation context.
async fn run_interactive_chat(
    client: &ailloy::Client,
    content: &str,
    initial_prompt: Option<&str>,
    _quiet: bool,
) -> Result<String> {
    eprintln!(
        "  {} Type {} to start generation, {} to exit.\n",
        "ℹ".blue().bold(),
        "/start".bold(),
        "/quit".bold()
    );

    let mut history: Vec<ailloy::Message> =
        vec![ailloy::Message::system(INTERACTIVE_SYSTEM_PROMPT)];

    // Build the opening message — pass the user's actual words to the AI
    let word_count = content.split_whitespace().count();
    let is_short_text = word_count < 200;

    let opening = match (initial_prompt, is_short_text) {
        (Some(prompt), true) => {
            // Short text input + explicit prompt: send both directly
            format!(
                "I want to create a presentation. Here's what I told you:\n\n\
                 \"{content}\"\n\n\
                 Additional context: {prompt}\n\n\
                 Acknowledge what I've already told you — don't ask me things I already \
                 answered. Then ask a focused follow-up question about something I \
                 haven't covered yet."
            )
        }
        (Some(prompt), false) => {
            // Long content + prompt: summarize content, include prompt
            format!(
                "I want to create a presentation. I have {word_count} words of source \
                 material to work from.\n\n\
                 My instructions: {prompt}\n\n\
                 Acknowledge my instructions and ask a focused follow-up question \
                 about something I haven't covered yet."
            )
        }
        (None, true) => {
            // Short text input, no prompt: the text IS the user's intent
            format!(
                "I want to create a presentation. Here's what I told you:\n\n\
                 \"{content}\"\n\n\
                 Acknowledge what I've already told you — don't ask me things I already \
                 answered. If I mentioned the audience, don't ask who the audience is. \
                 If I mentioned the goal, don't ask what the goal is. Instead, ask a \
                 focused follow-up question about something I haven't covered yet."
            )
        }
        (None, false) => {
            // Long content, no prompt: reference the content
            format!(
                "I want to create a presentation from {word_count} words of source \
                 material I've provided. Ask me a focused question about who the \
                 audience is and what I want to achieve with this presentation."
            )
        }
    };
    history.push(ailloy::Message::user(&opening));

    // Get initial AI response
    let response = stream_chat(client, &history).await?;
    history.push(ailloy::Message::assistant(&response));

    if let Some(summary) = extract_ready_summary(&response) {
        return Ok(build_full_context(content, &summary));
    }

    eprintln!();

    // Chat loop
    loop {
        let input = match read_user_input()? {
            Some(s) => s,
            None => continue,
        };

        match input.as_str() {
            "/quit" | "/exit" | "/q" => {
                anyhow::bail!("Cancelled.");
            }
            "/start" => {
                // Force the AI to summarize and produce [READY]
                history.push(ailloy::Message::user(
                    "I'm ready to generate. Please summarize what we've discussed and proceed.",
                ));
                let response = stream_chat(client, &history).await?;
                history.push(ailloy::Message::assistant(&response));
                eprintln!();

                let summary = extract_ready_summary(&response).unwrap_or(response);
                return Ok(build_full_context(content, &summary));
            }
            "/help" => {
                eprintln!("{}", "Commands:".bold());
                eprintln!("  {} — Start generating the presentation", "/start".bold());
                eprintln!("  {} — Exit without generating", "/quit".bold());
                eprintln!("  {} — Show this help", "/help".bold());
                continue;
            }
            s if s.starts_with('/') => {
                eprintln!(
                    "{} Unknown command. Type {} for help.",
                    "!".yellow().bold(),
                    "/help".bold()
                );
                continue;
            }
            _ => {}
        }

        history.push(ailloy::Message::user(&input));
        let response = stream_chat(client, &history).await?;
        history.push(ailloy::Message::assistant(&response));
        eprintln!();

        if let Some(summary) = extract_ready_summary(&response) {
            return Ok(build_full_context(content, &summary));
        }
    }
}

/// Extract the summary text before the [READY] marker.
fn extract_ready_summary(text: &str) -> Option<String> {
    let marker = "[READY]";
    let idx = text.find(marker)?;
    let summary = text[..idx].trim().to_string();
    if summary.is_empty() {
        None
    } else {
        Some(summary)
    }
}

/// Combine source content and chat summary into the full context for generation.
fn build_full_context(content: &str, summary: &str) -> String {
    format!(
        "PRESENTATION BRIEF:\n{summary}\n\n\
         SOURCE MATERIAL ({} words):\n{content}",
        content.split_whitespace().count()
    )
}

/// Stream a chat response, printing tokens to stderr. Returns assembled text.
/// The `[READY]` marker is suppressed from output but preserved in the returned string.
async fn stream_chat(client: &ailloy::Client, history: &[ailloy::Message]) -> Result<String> {
    let mut stream = client.chat_stream(history).await?;
    let mut assembled = String::new();
    // Buffer to detect and suppress [READY] marker from display
    let mut display_buf = String::new();
    const MARKER: &str = "[READY]";

    while let Some(event) = stream.next().await {
        match event? {
            ailloy::StreamEvent::Delta(text) => {
                assembled.push_str(&text);
                display_buf.push_str(&text);

                // Check if we might be in the middle of [READY]
                if MARKER.starts_with(&display_buf) {
                    // Could still be building toward [READY] — hold the buffer
                    continue;
                }

                if display_buf.contains(MARKER) {
                    // Found [READY] — print everything before it, discard the marker
                    let before = display_buf.split(MARKER).next().unwrap_or("");
                    if !before.is_empty() {
                        eprint!("{before}");
                    }
                    // Print anything after the marker (unlikely but handle it)
                    let after_idx = display_buf.find(MARKER).unwrap() + MARKER.len();
                    let after = &display_buf[after_idx..];
                    if !after.is_empty() {
                        eprint!("{after}");
                    }
                    display_buf.clear();
                } else {
                    // No marker possible — flush the buffer
                    eprint!("{display_buf}");
                    display_buf.clear();
                }
                io::stderr().flush()?;
            }
            ailloy::StreamEvent::Done(_) => {
                // Flush any remaining buffer (excluding [READY])
                if !display_buf.is_empty() && !display_buf.contains(MARKER) {
                    eprint!("{display_buf}");
                } else if display_buf.contains(MARKER) {
                    let before = display_buf.split(MARKER).next().unwrap_or("");
                    if !before.is_empty() {
                        eprint!("{before}");
                    }
                }
                eprintln!();
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
        Ok(0) => Ok(None),
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

// ── Spinner ─────────────────────────────────────────────────────────────────

/// A terminal spinner that animates on a background thread.
struct Spinner {
    handle: Option<std::thread::JoinHandle<()>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Spinner {
    /// Start a spinner with the given message. The spinner animates until `stop()` is called.
    fn start(message: String) -> Self {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_clone = stop.clone();
        let handle = std::thread::spawn(move || {
            const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0;
            while !stop_clone.load(std::sync::atomic::Ordering::Relaxed) {
                eprint!("\r  {} {}", FRAMES[i % FRAMES.len()], message);
                let _ = io::stderr().flush();
                i += 1;
                std::thread::sleep(std::time::Duration::from_millis(80));
            }
        });
        Self {
            handle: Some(handle),
            stop,
        }
    }

    /// Stop the spinner and replace its line with a completion message.
    fn stop_with(mut self, message: &str) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        // Clear the spinner line and print the completion message
        eprint!("\r\x1b[2K  {message}\n");
        let _ = io::stderr().flush();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

// ── AI pipeline ─────────────────────────────────────────────────────────────

/// Suggest a filename based on the presentation context.
async fn suggest_filename(client: &ailloy::Client, context: &str) -> Result<String> {
    let messages = vec![
        ailloy::Message::system(
            "Given a presentation description, suggest a short kebab-case filename (2-4 words, no extension). \
             Reply with ONLY the filename, nothing else. Example: git-flow-adoption",
        ),
        ailloy::Message::user(context),
    ];
    let response = client.chat(&messages).await?;
    let name = response
        .content
        .trim()
        .trim_matches('"')
        .trim_matches('`')
        .to_string();
    if name.is_empty() || name.len() > 60 {
        Ok("presentation.md".to_string())
    } else {
        Ok(format!("{name}.md"))
    }
}

/// Run the full generation pipeline: analyze → generate.
/// Returns (presentation_markdown, visualization_opportunities).
async fn run_pipeline(
    client: &ailloy::Client,
    content: &str,
    context: &str,
    style: &Option<String>,
    quiet: bool,
) -> Result<(String, Vec<VisualizationOpportunity>)> {
    // Step A: Analyze content and create outline
    let spinner = if !quiet {
        Some(Spinner::start("Analyzing content...".to_string()))
    } else {
        None
    };

    let outline = run_analysis(client, content, context).await?;

    if let Some(s) = spinner {
        s.stop_with(&format!("{} Content analyzed.", "✓".green().bold()));
    }

    // Extract opportunities from the outline
    let opportunities = extract_opportunities(&outline);

    // Count slides in outline for progress reporting
    let slide_count = outline
        .matches("\"title\"")
        .count()
        .saturating_sub(1)
        .max(1);

    // Step B: Generate slides
    let spinner = if !quiet {
        Some(Spinner::start(format!(
            "Generating ~{slide_count} slides..."
        )))
    } else {
        None
    };

    let presentation_md = run_generation(client, &outline, context, style).await?;

    if let Some(s) = spinner {
        s.stop_with(&format!("{} Presentation generated.", "✓".green().bold()));
    }

    Ok((presentation_md, opportunities))
}

const ANALYSIS_SYSTEM_PROMPT: &str = "\
You are a presentation architect for mdeck, a markdown-based presentation tool. \
Analyze source content and design a presentation outline.

RULES:
- Create a concise, engaging presentation — NOT a verbatim reproduction of the source.
- The source material is detailed reference that could be handed out AFTER the talk.
- The presentation should support a PRESENTER — keep slides focused and visual.
- Each slide covers ONE key point or a small group of closely related points.
- Never overload a slide with information. Less is more.
- ACTIVELY look for visualization opportunities. Many concepts are better shown \
  visually than described in bullet points. Think about: flows, processes, hierarchies, \
  comparisons, timelines, branching structures, data relationships, before/after states.
- When a visualization would be ideal but mdeck doesn't support it, you MUST add it \
  to the opportunities array with a detailed description. This is critical — these \
  opportunities help improve mdeck over time. Be specific about what the visualization \
  would show, how it would be structured, and why a static image is not a good substitute \
  (e.g., branch diagrams need precision that generated images cannot provide).
- For concepts that require PRECISION in their visual representation (e.g., Git branch \
  histories, flowcharts with exact paths, state machines), always flag them as opportunities \
  even if an image fallback is provided. A generated image approximates but cannot replace \
  a precise, data-driven visualization.

Respond in JSON:
```json
{
  \"title\": \"Presentation Title\",
  \"suggested_filename\": \"kebab-case-name\",
  \"slides\": [
    {
      \"title\": \"Slide Title\",
      \"key_points\": [\"point 1\", \"point 2\"],
      \"layout_hint\": \"bullet|code|quote|visualization|image|title|section|two-column\",
      \"visualization\": null,
      \"notes_hint\": \"What the presenter should convey and how\"
    }
  ],
  \"opportunities\": [
    {
      \"visualization_name\": \"General name for a REUSABLE visualization type (e.g. Branch Graph, Flow Diagram, State Machine — NOT Git Flow Branch Diagram). Think: what would this be called if it were a library component?\",
      \"description\": \"2-3 sentences: what this GENERAL visualization type shows, why it matters, and why bullet points or AI-generated images are not adequate substitutes. Describe the category of visualization, not just this specific use case.\",
      \"data_description\": \"Detailed description of the data model: what entities exist, their relationships, how they map to visual elements (nodes, edges, lanes, axes, etc.). Think generically — what data would ANY use of this visualization need?\",
      \"rendering_description\": \"How the visualization should look when rendered: layout direction, positioning, colors, labels, what gets drawn and where. Be specific enough that an implementer can build it.\",
      \"suggested_syntax\": \"Complete multi-line mdeck syntax example using the - item per line pattern consistent with mdeck's other visualizations. Show a realistic example with 3-5 data points. Each line should be a separate item, NOT a one-liner.\",
      \"ascii_mockup\": \"A multi-line ASCII art sketch showing what the rendered output would look like. Use actual newlines between lines, not escaped newlines.\"
    }
  ]
}
```

Supported mdeck visualizations (use these when appropriate — set visualization field to the tag name):
- barchart, linechart, piechart, donut, stackedbar, scatter (data charts)
- timeline, gantt (temporal)
- orgchart, architecture (structural)
- gitgraph (git branch diagrams — USE THIS for any branching strategy, Git Flow, \
  merge workflows, etc. Syntax: `- branch main`, `- branch develop from main`, \
  `- commit develop: \"msg\"`, `- merge feature -> develop: \"label\"`)
- kpi, progress, funnel (metrics)
- radar, venn (comparison)
- wordcloud (text analysis)

IMPORTANT: Always prefer a supported visualization over bullet points. For example, \
if the topic involves git branches, merges, or branching strategies, USE @gitgraph. \
If the topic involves timelines or processes over time, USE @timeline or @gantt. \
Only add to opportunities if NONE of the above types can represent the concept.

If a visualization would be useful but is NOT in the list above, add it to `opportunities`. \
DEDUPLICATE: if multiple slides would benefit from the same visualization type, create \
only ONE opportunity entry that covers all use cases — don't repeat the same visualization \
for every slide that needs it.

Do NOT set layout_hint to `image` as a fallback for precision visualizations — AI-generated \
images are unpredictable and often contain errors, making them unsuitable for diagrams, \
flowcharts, branch histories, or anything where accuracy matters. Only use `image` layout \
for decorative or mood-setting visuals that don't need to be precise.

8-20 slides for most content. Start with title slide, end with summary/conclusion.";

/// Run the content analysis step (silent — output captured, not printed).
async fn run_analysis(client: &ailloy::Client, content: &str, context: &str) -> Result<String> {
    let mut user_message = format!("PRESENTATION CONTEXT:\n{context}\n\nSOURCE CONTENT:\n");

    const MAX_CONTENT_CHARS: usize = 100_000;
    if content.len() > MAX_CONTENT_CHARS {
        user_message.push_str(&content[..MAX_CONTENT_CHARS]);
        user_message.push_str("\n\n[Content truncated.]");
    } else {
        user_message.push_str(content);
    }

    let messages = vec![
        ailloy::Message::system(ANALYSIS_SYSTEM_PROMPT),
        ailloy::Message::user(&user_message),
    ];

    // Silent — don't print the JSON to the user
    let mut stream = client.chat_stream(&messages).await?;
    let mut assembled = String::new();
    while let Some(event) = stream.next().await {
        match event? {
            ailloy::StreamEvent::Delta(text) => assembled.push_str(&text),
            ailloy::StreamEvent::Done(_) => {}
        }
    }

    Ok(assembled)
}

/// Run the slide generation step (silent — output captured, not printed).
async fn run_generation(
    client: &ailloy::Client,
    outline: &str,
    context: &str,
    style: &Option<String>,
) -> Result<String> {
    let image_style_hint = if let Some(s) = style {
        format!("\n- Use image style: \"{s}\" for all AI-generated images.")
    } else {
        String::new()
    };

    // No fallback images for missing visualizations — precision diagrams
    // must not use AI-generated images as they're unreliable
    let fallback_instructions = String::new();

    let system_prompt = format!(
        "You are a presentation content generator for mdeck. \
        Generate a complete presentation in mdeck markdown format.\n\n\
        MDECK FORMAT SPECIFICATION:\n{MDECK_SPEC}\n\n\
        CRITICAL RULES:\n\
        - Generate valid mdeck markdown.\n\
        - Start with YAML frontmatter (title, author, @theme, @transition).\n\
        - Use `---` to separate slides.\n\
        - Include DETAILED speaker notes after `???` on EVERY slide. Speaker notes must be \
          thorough enough for someone who has NEVER seen the source material to present \
          effectively. Each note should include:\n\
          • The core message of the slide (what the audience should understand)\n\
          • Detailed talking points (what to say, in what order)\n\
          • Suggested delivery approach (pause here, ask this question, emphasize this)\n\
          • Background context the presenter needs to answer audience questions\n\
          • Transition to the next slide\n\
        - Use progressive reveal (`+` markers) for bullet lists where it helps pacing.\n\
        - Use visualization code blocks where the outline specifies them.\n\
        - ONLY use `![descriptive prompt](image-generation)` for decorative or mood-setting \
          images that lighten up the presentation — NOT for diagrams, flowcharts, processes, \
          or anything that requires precision. AI-generated images are unpredictable and often \
          contain errors, so they must never be used where accuracy matters. If a slide needs \
          a precise visualization that mdeck doesn't support, use bullet points or text \
          instead — the presenter can draw on a whiteboard if needed.\n\
        - Do NOT add images just because you can. Only include them when they genuinely \
          enhance the presentation's atmosphere or help set the mood for a section.\n\
        - Keep slide text concise — the presentation supports the presenter.\n\
        - Use **bold** and *italic* for emphasis.\n\
        - Output ONLY the markdown content.{image_style_hint}{fallback_instructions}"
    );

    let user_message = format!(
        "Generate a complete mdeck presentation from this outline:\n\n{outline}\n\n\
         CONTEXT:\n{context}"
    );

    let messages = vec![
        ailloy::Message::system(&system_prompt),
        ailloy::Message::user(&user_message),
    ];

    // Silent generation — don't print raw markdown
    let mut stream = client.chat_stream(&messages).await?;
    let mut assembled = String::new();
    while let Some(event) = stream.next().await {
        match event? {
            ailloy::StreamEvent::Delta(text) => assembled.push_str(&text),
            ailloy::StreamEvent::Done(_) => {}
        }
    }

    let cleaned = strip_markdown_fences(&assembled);
    Ok(cleaned)
}

/// Strip markdown code fences if the AI wrapped the response.
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

    if output_str.ends_with('/') || output_str.ends_with('\\') || output.is_dir() {
        let dir = output.to_path_buf();
        let file = dir.join("presentation.md");
        Ok((file, dir))
    } else if output
        .extension()
        .is_some_and(|ext| ext == "md" || ext == "markdown")
    {
        let dir = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf();
        Ok((output.to_path_buf(), dir))
    } else {
        let dir = output.to_path_buf();
        let file = dir.join("presentation.md");
        Ok((file, dir))
    }
}

// ── Visualization opportunities ─────────────────────────────────────────────

/// A structured visualization opportunity extracted from the AI outline.
#[derive(Debug, Clone)]
struct VisualizationOpportunity {
    visualization_name: String,
    description: String,
    data_description: String,
    rendering_description: String,
    suggested_syntax: String,
    ascii_mockup: String,
}

/// Extract visualization opportunities from the AI outline JSON.
fn extract_opportunities(outline: &str) -> Vec<VisualizationOpportunity> {
    let mut opportunities = Vec::new();

    // Find the "opportunities" array in the JSON
    let Some(start) = outline.find("\"opportunities\"") else {
        return opportunities;
    };
    let Some(arr_start) = outline[start..].find('[') else {
        return opportunities;
    };
    let arr_content = &outline[start + arr_start..];

    // Find matching closing bracket
    let mut depth = 0;
    let mut end = 0;
    for (i, c) in arr_content.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = i + c.len_utf8();
                    break;
                }
            }
            _ => {}
        }
    }
    if end == 0 {
        return opportunities;
    }

    let arr_str = &arr_content[..end];

    // Parse individual opportunity objects
    let mut obj_depth = 0;
    let mut obj_start = None;
    for (i, c) in arr_str.char_indices() {
        match c {
            '{' => {
                if obj_depth == 0 {
                    obj_start = Some(i);
                }
                obj_depth += 1;
            }
            '}' => {
                obj_depth -= 1;
                if obj_depth == 0 {
                    if let Some(start) = obj_start {
                        let obj = &arr_str[start..i + c.len_utf8()];
                        if let Some(opp) = parse_opportunity(obj) {
                            opportunities.push(opp);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    opportunities
}

/// Parse a single opportunity JSON object.
fn parse_opportunity(json: &str) -> Option<VisualizationOpportunity> {
    fn extract_field(json: &str, field: &str) -> String {
        let pattern = format!("\"{field}\"");
        let Some(pos) = json.find(&pattern) else {
            return String::new();
        };
        let after = &json[pos + pattern.len()..];
        // Skip `: "`
        let Some(quote_start) = after.find('"') else {
            return String::new();
        };
        let value_start = &after[quote_start + 1..];
        let mut result = String::new();
        let mut escaped = false;
        for c in value_start.chars() {
            if escaped {
                match c {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    _ => result.push(c), // \", \\, etc.
                }
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                break;
            } else {
                result.push(c);
            }
        }
        result
    }

    let viz_name = extract_field(json, "visualization_name");
    let description = extract_field(json, "description");

    if description.is_empty() && viz_name.is_empty() {
        return None;
    }

    Some(VisualizationOpportunity {
        visualization_name: if viz_name.is_empty() {
            "Unknown".to_string()
        } else {
            viz_name
        },
        description,
        data_description: extract_field(json, "data_description"),
        rendering_description: extract_field(json, "rendering_description"),
        suggested_syntax: extract_field(json, "suggested_syntax"),
        ascii_mockup: extract_field(json, "ascii_mockup"),
    })
}

/// Write visualization opportunities to a file in GitHub-issue-ready format.
/// If the file already exists, appends only new opportunities (by name) to avoid duplicates.
fn write_opportunities(path: &Path, opportunities: &[VisualizationOpportunity]) -> Result<()> {
    let header = "# Visualization Opportunities for MDeck\n\n\
         Each section below is a self-contained feature request ready to be submitted \
         as a GitHub issue. Copy the section you're interested in and paste it at:\n\
         https://github.com/mklab-se/mdeck/issues/new\n\n";

    // Read existing file to find already-listed opportunities and the next number
    let (mut content, mut next_number) = if path.exists() {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        // Count existing entries to continue numbering
        let count = existing.matches("## ").count();
        (existing, count + 1)
    } else {
        (header.to_string(), 1)
    };

    // Collect existing visualization names (lowercase, no spaces) to deduplicate
    let existing_lower = content.to_lowercase();

    let mut added = 0;
    for opp in opportunities {
        let tag = opp.visualization_name.to_lowercase().replace(' ', "");
        // Skip if this visualization type is already in the file
        if existing_lower.contains(&format!("`@{tag}`")) {
            continue;
        }

        content.push_str(&format!(
            "---\n\n## {next_number}. Feature Request: `@{tag}` Visualization\n\n"
        ));

        content.push_str("### Summary\n\n");
        content.push_str(&format!("{}\n\n", opp.description));

        if !opp.data_description.is_empty() {
            content.push_str("### Data Model\n\n");
            content.push_str(&format!("{}\n\n", opp.data_description));
        }

        if !opp.rendering_description.is_empty() {
            content.push_str("### Rendering Specification\n\n");
            content.push_str(&format!("{}\n\n", opp.rendering_description));
        }

        if !opp.ascii_mockup.is_empty() {
            content.push_str("### Visual Mockup\n\n```\n");
            content.push_str(&opp.ascii_mockup);
            content.push_str("\n```\n\n");
        }

        if !opp.suggested_syntax.is_empty() {
            content.push_str("### Proposed Syntax\n\n````markdown\n");
            content.push_str(&format!("```@{tag}\n"));
            content.push_str(&opp.suggested_syntax);
            content.push_str("\n```\n````\n\n");
        }

        content.push_str("### Implementation Notes\n\n");
        content.push_str(
            "MDeck renders visualizations from fenced code blocks with `@` language tags \
             (e.g., `@barchart`, `@timeline`, `@architecture`). Each visualization type \
             is implemented as a Rust rendering function in `crates/mdeck/src/render/`. \
             The parser detects the `@` tag in `crates/mdeck/src/parser/blocks.rs` and \
             creates a corresponding `Block` variant. Progressive reveal is supported \
             via `+` and `*` list markers.\n\n",
        );

        next_number += 1;
        added += 1;
    }

    if added > 0 {
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write opportunities: {}", path.display()))?;
    }

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
                    "visualization_name": "Swimlane Diagram",
                    "description": "Shows cross-team workflow with parallel lanes",
                    "data_description": "Teams as horizontal lanes with tasks flowing between them",
                    "rendering_description": "Horizontal lanes with arrows between them",
                    "suggested_syntax": "- Marketing -> Engineering: handoff",
                    "ascii_mockup": "| Marketing | --> | Engineering | --> | QA |"
                }
            ]
        }"#;
        let opps = extract_opportunities(outline);
        assert_eq!(opps.len(), 1);
        assert_eq!(opps[0].visualization_name, "Swimlane Diagram");
        assert!(opps[0].description.contains("cross-team"));
        assert!(!opps[0].ascii_mockup.is_empty());
    }

    #[test]
    fn test_extract_ready_summary() {
        let text = "Great! Here's what we'll create:\n\nA presentation about Git Flow for developers.\n\n[READY]";
        let summary = extract_ready_summary(text).unwrap();
        assert!(summary.contains("Git Flow"));
        assert!(!summary.contains("[READY]"));
    }

    #[test]
    fn test_extract_ready_summary_none() {
        assert!(extract_ready_summary("Just chatting, no marker here.").is_none());
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
        let xml = r#"<w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body>"#;
        let text = extract_text_from_docx_xml(xml);
        assert!(text.contains("Cell"));
    }

    #[test]
    fn test_docx_xml_empty_document() {
        let xml = r#"<w:body></w:body>"#;
        let text = extract_text_from_docx_xml(xml);
        assert!(text.trim().is_empty());
    }

    #[test]
    fn test_docx_xml_self_closing_text_tag() {
        let xml = r#"<w:p><w:r><w:t/>Outside text</w:r></w:p>"#;
        let text = extract_text_from_docx_xml(xml);
        assert!(!text.contains("Outside"));
    }

    // ── Input resolution tests ──────────────────────────────────────────────

    #[test]
    fn test_resolve_input_literal_text() {
        let args = CreateArgs {
            input: Some("A presentation about Rust programming".to_string()),
            output: PathBuf::from("out.md"),
            prompt: None,
            interactive: false,
            style: None,
        };
        let (label, content) = resolve_input(&args, true).unwrap();
        assert_eq!(label, "(text input)");
        assert_eq!(content, "A presentation about Rust programming");
    }

    #[test]
    fn test_resolve_input_existing_file() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let cargo_toml = format!("{manifest_dir}/Cargo.toml");
        let args = CreateArgs {
            input: Some(cargo_toml),
            output: PathBuf::from("out.md"),
            prompt: None,
            interactive: false,
            style: None,
        };
        let (label, content) = resolve_input(&args, true).unwrap();
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
        let result = resolve_input(&args, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_input_interactive_with_input_provided() {
        let args = CreateArgs {
            input: Some("A talk about functional programming".to_string()),
            output: PathBuf::from("out.md"),
            prompt: None,
            interactive: true,
            style: None,
        };
        let (label, content) = resolve_input(&args, true).unwrap();
        assert_eq!(label, "(text input)");
        assert_eq!(content, "A talk about functional programming");
    }

    #[test]
    fn test_resolve_output_markdown_extension() {
        let (file, _dir) = resolve_output(Path::new("talk.markdown")).unwrap();
        assert_eq!(file, PathBuf::from("talk.markdown"));
    }

    #[test]
    fn test_strip_fences_generic_wrapper() {
        let input = "```\nfunction foo() {}\n```";
        let result = strip_markdown_fences(input);
        assert_eq!(result, "function foo() {}");
    }

    #[test]
    fn test_strip_fences_code_with_language() {
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

    #[test]
    fn test_extract_opportunities_multiple() {
        let outline = r#"{
            "opportunities": [
                {
                    "visualization_name": "Swimlane",
                    "description": "Cross-team flow"
                },
                {
                    "visualization_name": "Sankey",
                    "description": "Data flow volumes"
                }
            ]
        }"#;
        let opps = extract_opportunities(outline);
        assert_eq!(opps.len(), 2);
        assert_eq!(opps[0].visualization_name, "Swimlane");
        assert_eq!(opps[1].visualization_name, "Sankey");
    }

    #[test]
    fn test_extract_opportunities_no_opportunities_key() {
        let outline = r#"{"slides": [{"title": "Intro"}]}"#;
        assert!(extract_opportunities(outline).is_empty());
    }

    #[test]
    fn test_parse_opportunity_full() {
        let json = r#"{
            "visualization_name": "Swimlane Diagram",
            "description": "Shows parallel workflows",
            "data_description": "Teams and tasks",
            "rendering_description": "Horizontal lanes with arrows",
            "suggested_syntax": "- Marketing -> Engineering: handoff",
            "ascii_mockup": "| Marketing | --> | Engineering |"
        }"#;
        let opp = parse_opportunity(json).unwrap();
        assert_eq!(opp.visualization_name, "Swimlane Diagram");
        assert!(!opp.rendering_description.is_empty());
        assert!(!opp.ascii_mockup.is_empty());
    }

    #[test]
    fn test_build_full_context() {
        let content = "Some source text about Git.";
        let summary = "A presentation about Git Flow for developers.";
        let result = build_full_context(content, summary);
        assert!(result.contains("PRESENTATION BRIEF:"));
        assert!(result.contains("Git Flow"));
        assert!(result.contains("SOURCE MATERIAL"));
        assert!(result.contains("5 words"));
    }
}
