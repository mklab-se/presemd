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
}

#[derive(Debug, Clone)]
pub struct Slide {
    #[allow(dead_code)]
    pub directives: Vec<Directive>,
    pub blocks: Vec<Block>,
    pub layout: Layout,
    /// The original raw markdown source text for this slide.
    pub raw_source: String,
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
    TwoColumn,
    Content,
}

pub fn parse(content: &str, _base_path: &Path) -> Presentation {
    let (meta, body) = frontmatter::extract(content);
    let raw_slides = splitter::split(&body);
    let slides: Vec<Slide> = raw_slides
        .into_iter()
        .filter(|raw| !raw.trim().is_empty())
        .map(|raw| {
            let raw_source = raw.clone();
            let (directives, content) = blocks::extract_directives(&raw);
            let blocks = blocks::parse(&content);
            let layout = classify_layout(&directives, &blocks);
            Slide {
                directives,
                blocks,
                layout,
                raw_source,
            }
        })
        .collect();
    Presentation { meta, slides }
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
                "diagram" => Layout::Diagram,
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
            Block::Table { .. } => tables += 1,
            Block::ColumnSeparator => column_separators += 1,
            Block::HorizontalRule => {}
        }
    }

    let total = blocks.len();

    // 1. Diagram
    if diagrams > 0 {
        return Layout::Diagram;
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

    // 7. Quote slide
    if quotes > 0 && lists == 0 && code_blocks == 0 && images == 0 && tables == 0 {
        return Layout::Quote;
    }

    // 8. Code slide
    if code_blocks > 0 && lists == 0 && images == 0 && quotes == 0 && tables == 0 {
        return Layout::Code;
    }

    // 9. Bullet slide: heading + list
    if !headings.is_empty() && lists > 0 && code_blocks == 0 && images == 0 && quotes == 0 {
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
        let content = include_str!("../../../../sample-presentations/poker-night.md");
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
        let content = include_str!("../../../../sample-presentations/saloon-workshop.md");
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
}
