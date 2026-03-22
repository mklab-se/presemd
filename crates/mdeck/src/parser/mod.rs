pub mod blocks;
pub mod frontmatter;
pub mod inline;
pub mod splitter;

use std::path::Path;

#[derive(Debug, Clone)]
pub struct Presentation {
    pub meta: PresentationMeta,
    pub slides: Vec<Slide>,
}

#[derive(Debug, Clone, Default)]
pub struct PresentationMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub theme: Option<String>,
    pub transition: Option<String>,
    pub aspect: Option<String>,
    pub code_theme: Option<String>,
    pub footer: Option<String>,
    pub image_style: Option<String>,
    pub icon_style: Option<String>,
    pub slide_level: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct Slide {
    #[allow(dead_code)]
    pub directives: Vec<Directive>,
    pub blocks: Vec<Block>,
    pub layout: Layout,
    /// The original raw markdown source text for this slide.
    pub raw_source: String,
    /// Speaker notes for this slide (content after `???` separator).
    #[allow(dead_code)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Directive {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum Block {
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Paragraph {
        inlines: Vec<Inline>,
    },
    List {
        ordered: bool,
        items: Vec<ListItem>,
    },
    Image {
        alt: String,
        path: String,
        directives: ImageDirectives,
    },
    CodeBlock {
        language: Option<String>,
        code: String,
        highlight_lines: Vec<usize>,
    },
    BlockQuote {
        inlines: Vec<Inline>,
    },
    Table {
        headers: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    HorizontalRule,
    Diagram {
        content: String,
    },
    WordCloud {
        content: String,
    },
    Timeline {
        content: String,
    },
    PieChart {
        content: String,
    },
    BarChart {
        content: String,
    },
    LineChart {
        content: String,
    },
    DonutChart {
        content: String,
    },
    KpiCards {
        content: String,
    },
    FunnelChart {
        content: String,
    },
    RadarChart {
        content: String,
    },
    StackedBar {
        content: String,
    },
    VennDiagram {
        content: String,
    },
    ProgressBars {
        content: String,
    },
    ScatterPlot {
        content: String,
    },
    OrgChart {
        content: String,
    },
    GanttChart {
        content: String,
    },
    GitGraph {
        content: String,
    },
    ColumnSeparator,
}

#[derive(Debug, Clone, Default)]
pub struct ImageDirectives {
    pub width: Option<String>,
    pub height: Option<String>,
    pub fill: bool,
    pub fit: bool,
    pub align: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Code(String),
    Link {
        text: Vec<Inline>,
        #[allow(dead_code)]
        url: String,
    },
}

#[derive(Debug, Clone)]
pub struct ListItem {
    pub marker: ListMarker,
    pub inlines: Vec<Inline>,
    pub children: Vec<ListItem>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ListMarker {
    Static,
    NextStep,
    WithPrev,
    Ordered,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layout {
    Title,
    Section,
    Image,
    Gallery,
    Quote,
    Code,
    Bullet,
    Diagram,
    Visualization,
    TwoColumn,
    Content,
}

pub fn parse(content: &str, _base_path: &Path) -> Presentation {
    let (meta, body) = frontmatter::extract(content);
    let raw_slides = splitter::split(&body, meta.slide_level);
    let slides: Vec<Slide> = raw_slides
        .into_iter()
        .filter(|raw| !raw.trim().is_empty())
        .map(|raw| {
            let raw_source = raw.clone();
            let (content_part, notes) = extract_notes(&raw);
            let (directives, content) = blocks::extract_directives(&content_part);
            let blocks = blocks::parse(&content);
            let layout = classify_layout(&directives, &blocks);
            Slide {
                directives,
                blocks,
                layout,
                raw_source,
                notes,
            }
        })
        .collect();
    Presentation { meta, slides }
}

/// Extract speaker notes from a raw slide string.
///
/// Notes are separated from slide content by a `???` line (three or more `?` characters).
/// The `???` separator is ignored inside fenced code blocks.
/// Returns `(content, Some(notes))` if a notes separator was found, or `(original, None)`.
fn extract_notes(raw: &str) -> (String, Option<String>) {
    let mut in_code_fence = false;
    let mut fence_char: char = '`';
    let mut fence_len: usize = 0;

    let lines: Vec<&str> = raw.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Track fenced code blocks
        if in_code_fence {
            let closing_count = trimmed.chars().take_while(|&c| c == fence_char).count();
            if closing_count >= fence_len
                && trimmed
                    .chars()
                    .skip(closing_count)
                    .all(|c| c.is_whitespace())
            {
                in_code_fence = false;
            }
        } else if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_fence = true;
            fence_char = trimmed.chars().next().unwrap();
            fence_len = trimmed.chars().take_while(|&c| c == fence_char).count();
        }

        // Check for notes separator (??? with 3+ question marks, outside code blocks)
        if !in_code_fence && trimmed.len() >= 3 && trimmed.chars().all(|c| c == '?') {
            let content = lines[..i].join("\n");
            let notes_text = if i + 1 < lines.len() {
                lines[i + 1..].join("\n").trim().to_string()
            } else {
                String::new()
            };
            let notes = if notes_text.is_empty() {
                None
            } else {
                Some(notes_text)
            };
            return (content, notes);
        }
    }

    (raw.to_string(), None)
}

fn classify_layout(directives: &[Directive], blocks: &[Block]) -> Layout {
    // Check for explicit @layout directive
    for d in directives {
        if d.name == "layout" {
            return match d.value.as_str() {
                "title" => Layout::Title,
                "section" => Layout::Section,
                "image" => Layout::Image,
                "gallery" => Layout::Gallery,
                "quote" => Layout::Quote,
                "code" => Layout::Code,
                "bullets" | "bullet" => Layout::Bullet,
                "diagram" | "architecture" => Layout::Diagram,
                "visualization" => Layout::Visualization,
                "two-column" => Layout::TwoColumn,
                _ => Layout::Content,
            };
        }
    }

    // Count element types
    let mut headings: Vec<u8> = Vec::new();
    let mut paragraphs = 0;
    let mut short_paragraphs = 0;
    let mut lists = 0;
    let mut images = 0;
    let mut code_blocks = 0;
    let mut quotes = 0;
    let mut diagrams = 0;
    let mut visualizations = 0;
    let mut tables = 0;
    let mut column_separators = 0;

    for block in blocks {
        match block {
            Block::Heading { level, .. } => headings.push(*level),
            Block::Paragraph { inlines } => {
                paragraphs += 1;
                let text_len: usize = inlines.iter().map(inline_text_len).sum();
                if text_len < 120 {
                    short_paragraphs += 1;
                }
            }
            Block::List { .. } => lists += 1,
            Block::Image { .. } => images += 1,
            Block::CodeBlock { .. } => code_blocks += 1,
            Block::BlockQuote { .. } => quotes += 1,
            Block::Diagram { .. } => diagrams += 1,
            Block::WordCloud { .. }
            | Block::Timeline { .. }
            | Block::PieChart { .. }
            | Block::BarChart { .. }
            | Block::LineChart { .. }
            | Block::DonutChart { .. }
            | Block::KpiCards { .. }
            | Block::FunnelChart { .. }
            | Block::RadarChart { .. }
            | Block::StackedBar { .. }
            | Block::VennDiagram { .. }
            | Block::ProgressBars { .. }
            | Block::ScatterPlot { .. }
            | Block::OrgChart { .. }
            | Block::GanttChart { .. }
            | Block::GitGraph { .. } => visualizations += 1,
            Block::Table { .. } => tables += 1,
            Block::ColumnSeparator => column_separators += 1,
            Block::HorizontalRule => {}
        }
    }

    let total = blocks.len();

    // 1. Diagram / Visualization
    if diagrams > 0 {
        return Layout::Diagram;
    }
    if visualizations > 0 {
        return Layout::Visualization;
    }

    // 2. Two-column (has column separator)
    if column_separators > 0 {
        return Layout::TwoColumn;
    }

    // 3. Title: H1 + optional (H2 or short P), nothing else
    if headings.len() == 1 && headings[0] == 1 {
        let non_heading = total - 1;
        if non_heading == 0 {
            // Just H1 — could be section or title
            // If it's a lone H1, it's a section divider
            return Layout::Section;
        }
        if non_heading == 1 && (short_paragraphs == 1 || headings.len() == 1) {
            // Check if the other element is H2 or short paragraph
            for block in blocks {
                match block {
                    Block::Heading { level: 2, .. } => return Layout::Title,
                    Block::Paragraph { inlines } => {
                        let text_len: usize = inlines.iter().map(inline_text_len).sum();
                        if text_len < 120 {
                            return Layout::Title;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // 4. Section divider: single heading, nothing else
    if headings.len() == 1
        && paragraphs == 0
        && lists == 0
        && images == 0
        && code_blocks == 0
        && quotes == 0
        && tables == 0
    {
        return Layout::Section;
    }

    // 5. Image slide: single image, optional heading, optional short caption
    if images == 1 && lists == 0 && code_blocks == 0 && quotes == 0 && tables == 0 {
        let other = total - images - headings.len();
        if other <= 1 {
            return Layout::Image;
        }
    }

    // 6. Gallery: 2+ images
    if images >= 2
        && lists == 0
        && code_blocks == 0
        && quotes == 0
        && paragraphs == 0
        && tables == 0
    {
        return Layout::Gallery;
    }

    // 7. Quote slide (allow one image for side-panel rendering)
    if quotes > 0 && lists == 0 && code_blocks == 0 && images <= 1 && tables == 0 {
        return Layout::Quote;
    }

    // 8. Code slide (allow one image for side-panel rendering)
    if code_blocks > 0 && lists == 0 && images <= 1 && quotes == 0 && tables == 0 {
        return Layout::Code;
    }

    // 9. Bullet slide: heading + list (allow one image for side-panel rendering)
    if !headings.is_empty() && lists > 0 && code_blocks == 0 && images <= 1 && quotes == 0 {
        return Layout::Bullet;
    }

    Layout::Content
}

/// Count the maximum number of reveal steps in a slide's blocks.
/// Each `+` (NextStep) marker in any list or diagram counts as one step.
pub fn compute_max_steps(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .map(|b| match b {
            Block::List { items, .. } => count_next_steps(items),
            Block::Diagram { content } => crate::render::diagram::count_diagram_steps(content),
            Block::WordCloud { content }
            | Block::Timeline { content }
            | Block::PieChart { content }
            | Block::BarChart { content }
            | Block::LineChart { content }
            | Block::DonutChart { content }
            | Block::KpiCards { content }
            | Block::FunnelChart { content }
            | Block::RadarChart { content }
            | Block::StackedBar { content }
            | Block::VennDiagram { content }
            | Block::ProgressBars { content }
            | Block::ScatterPlot { content }
            | Block::OrgChart { content }
            | Block::GanttChart { content }
            | Block::GitGraph { content } => {
                crate::render::visualizations::count_viz_steps(content)
            }
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

fn count_next_steps(items: &[ListItem]) -> usize {
    let mut count = 0;
    for item in items {
        if item.marker == ListMarker::NextStep {
            count += 1;
        }
        count += count_next_steps(&item.children);
    }
    count
}

/// Extract plain text from inline elements.
#[allow(dead_code)]
pub fn inlines_to_text(inlines: &[Inline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) => text.push_str(s),
            Inline::Bold(children) | Inline::Italic(children) | Inline::Strikethrough(children) => {
                text.push_str(&inlines_to_text(children));
            }
            Inline::Code(s) => text.push_str(s),
            Inline::Link { text: t, .. } => text.push_str(&inlines_to_text(t)),
        }
    }
    text
}

fn inline_text_len(inline: &Inline) -> usize {
    match inline {
        Inline::Text(s) => s.len(),
        Inline::Bold(children) | Inline::Italic(children) | Inline::Strikethrough(children) => {
            children.iter().map(inline_text_len).sum()
        }
        Inline::Code(s) => s.len(),
        Inline::Link { text, .. } => text.iter().map(inline_text_len).sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_poker_night_parses() {
        let content = include_str!("../../../../samples/poker-night.md");
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.meta.theme.as_deref(), Some("dark"));
        assert_eq!(pres.meta.transition.as_deref(), Some("slide"));
        assert!(
            pres.slides.len() >= 14,
            "Expected at least 14 slides, got {}",
            pres.slides.len()
        );
        assert!(matches!(pres.slides[0].layout, Layout::Title));
    }

    #[test]
    fn test_saloon_workshop_parses() {
        let content = include_str!("../../../../samples/saloon-workshop.md");
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.meta.theme.as_deref(), Some("light"));
        assert_eq!(pres.meta.transition.as_deref(), Some("fade"));
        assert_eq!(
            pres.meta.footer.as_deref(),
            Some("mdeck sample presentation")
        );
        assert!(
            pres.slides.len() >= 16,
            "Expected at least 16 slides, got {}",
            pres.slides.len()
        );
        assert!(matches!(pres.slides[0].layout, Layout::Title));
    }

    #[test]
    fn test_title_slide_layout() {
        let content = "# Hello World\n\nA subtitle here";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(matches!(pres.slides[0].layout, Layout::Title));
    }

    #[test]
    fn test_section_layout() {
        let content = "## Part One";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(matches!(pres.slides[0].layout, Layout::Section));
    }

    #[test]
    fn test_bullet_layout() {
        let content = "# Key Points\n\n- First\n- Second\n- Third";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(matches!(pres.slides[0].layout, Layout::Bullet));
    }

    #[test]
    fn test_quote_layout() {
        let content = "> Something wise\n\n-- Author";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(matches!(pres.slides[0].layout, Layout::Quote));
    }

    #[test]
    fn test_code_layout() {
        let content = "# Example\n\n```rust\nfn main() {}\n```";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(matches!(pres.slides[0].layout, Layout::Code));
    }

    #[test]
    fn test_two_column_layout() {
        let content = "@layout: two-column\n\n# Compare\n\nLeft side\n\n+++\n\nRight side";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(matches!(pres.slides[0].layout, Layout::TwoColumn));
    }

    #[test]
    fn test_image_layout() {
        let content = "![Photo @fill](photo.jpg)\n\nA caption";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(matches!(pres.slides[0].layout, Layout::Image));
    }

    #[test]
    fn test_multiple_slides() {
        let content = "# Slide One\n\nContent\n\n\n\n# Slide Two\n\nMore content";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 2);
    }

    #[test]
    fn test_slide_separator_dashes() {
        let content = "# Slide One\n\n---\n\n# Slide Two";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 2);
    }

    #[test]
    fn test_heading_inference() {
        let content = "# First\n\nSome content\n\n# Second\n\nMore content";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 2);
    }

    // --- Speaker Notes Tests ---

    #[test]
    fn test_notes_basic() {
        let content = "# My Slide\n\n- Point one\n- Point two\n\n???\n\nThis is the speaker note.";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert_eq!(
            pres.slides[0].notes.as_deref(),
            Some("This is the speaker note.")
        );
        // Blocks should NOT contain notes content
        assert!(matches!(pres.slides[0].layout, Layout::Bullet));
    }

    #[test]
    fn test_notes_none() {
        let content = "# Simple Slide\n\nJust content, no notes.";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(pres.slides[0].notes.is_none());
    }

    #[test]
    fn test_notes_multiline_with_formatting() {
        let content =
            "# Slide\n\n???\n\nFirst line of notes.\n\nSecond paragraph with **bold** text.";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert_eq!(
            pres.slides[0].notes.as_deref(),
            Some("First line of notes.\n\nSecond paragraph with **bold** text.")
        );
    }

    #[test]
    fn test_notes_inside_code_block_ignored() {
        let content = "# Slide\n\n```\n???\nsome code\n```\n\nParagraph after code.";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(
            pres.slides[0].notes.is_none(),
            "??? inside code block should not be treated as notes separator"
        );
    }

    #[test]
    fn test_notes_four_question_marks() {
        let content = "# Slide\n\n????\n\nNotes with four question marks.";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert_eq!(
            pres.slides[0].notes.as_deref(),
            Some("Notes with four question marks.")
        );
    }

    #[test]
    fn test_notes_separator_as_first_line() {
        let content = "???\n\nOnly notes, no visible content.";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert_eq!(
            pres.slides[0].notes.as_deref(),
            Some("Only notes, no visible content.")
        );
        assert!(pres.slides[0].blocks.is_empty());
    }

    #[test]
    fn test_notes_empty_after_separator() {
        let content = "# Slide\n\n???";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        // Empty notes after separator → None
        assert!(pres.slides[0].notes.is_none());
    }

    #[test]
    fn test_notes_across_multiple_slides() {
        let content =
            "# Slide 1\n\n???\n\nNotes for slide 1\n\n---\n\n# Slide 2\n\n???\n\nNotes for slide 2";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 2);
        assert_eq!(pres.slides[0].notes.as_deref(), Some("Notes for slide 1"));
        assert_eq!(pres.slides[1].notes.as_deref(), Some("Notes for slide 2"));
    }

    #[test]
    fn test_notes_layout_unaffected() {
        // Notes content should not affect layout classification
        let content =
            "# Title\n\nSubtitle\n\n???\n\n- This list in notes should not make it a Bullet layout";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(
            matches!(pres.slides[0].layout, Layout::Title),
            "Layout should be Title, got {:?}",
            pres.slides[0].layout
        );
    }

    #[test]
    fn test_notes_existing_samples_regression() {
        // Existing presentations should have no notes (no ??? separators)
        let content = include_str!("../../../../samples/poker-night.md");
        let pres = parse(content, Path::new("."));
        for (i, slide) in pres.slides.iter().enumerate() {
            assert!(
                slide.notes.is_none(),
                "Slide {} in poker-night.md should have no notes",
                i
            );
        }
    }

    #[test]
    fn test_notes_inside_tilde_code_block_ignored() {
        let content = "# Slide\n\n~~~\n???\nsome code\n~~~\n\nParagraph after code.";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(
            pres.slides[0].notes.is_none(),
            "??? inside tilde code block should not be treated as notes separator"
        );
    }

    #[test]
    fn test_notes_only_first_separator_counts() {
        let content = "# Slide\n\n???\n\nFirst notes section\n\n???\n\nSecond section";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        // Only the first ??? should split — everything after it is notes
        let notes = pres.slides[0].notes.as_deref().unwrap();
        assert!(notes.contains("First notes section"));
        assert!(notes.contains("???"));
        assert!(notes.contains("Second section"));
    }

    #[test]
    fn test_notes_separator_with_whitespace() {
        let content = "# Slide\n\n   ???   \n\nNotes with whitespace separator.";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert_eq!(
            pres.slides[0].notes.as_deref(),
            Some("Notes with whitespace separator.")
        );
    }

    #[test]
    fn test_notes_separator_with_mixed_content_not_separator() {
        // "??? some text" should NOT be a notes separator — it has non-? chars
        let content = "# Slide\n\n??? some text here\n\nMore content.";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(
            pres.slides[0].notes.is_none(),
            "??? followed by text should not be a notes separator"
        );
    }

    #[test]
    fn test_notes_two_question_marks_not_separator() {
        let content = "# Slide\n\n??\n\nNot notes.";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(
            pres.slides[0].notes.is_none(),
            "?? (only 2 marks) should not trigger notes separator"
        );
    }

    #[test]
    fn test_notes_with_directives() {
        // Directives should still work when notes are present
        let content = "@layout: code\n# Code Example\n\n```rust\nfn main() {}\n```\n\n???\n\nExplain that this is a minimal Rust program.";
        let pres = parse(content, Path::new("."));
        assert_eq!(pres.slides.len(), 1);
        assert!(
            matches!(pres.slides[0].layout, Layout::Code),
            "Layout should be Code when @layout directive is present, got {:?}",
            pres.slides[0].layout
        );
        assert_eq!(
            pres.slides[0].notes.as_deref(),
            Some("Explain that this is a minimal Rust program.")
        );
    }

    #[test]
    fn test_notes_sample_presentation_parses() {
        let content = include_str!("../../../../samples/features/notes.md");
        let pres = parse(content, Path::new("."));
        assert!(
            pres.slides.len() >= 5,
            "Expected at least 5 slides in notes sample, got {}",
            pres.slides.len()
        );
        // Every slide in the notes sample should have notes
        for (i, slide) in pres.slides.iter().enumerate() {
            assert!(
                slide.notes.is_some(),
                "Slide {} in notes.md should have notes",
                i
            );
        }
    }
}
