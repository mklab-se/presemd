/// Split a document body (after frontmatter extraction) into raw slide strings.
///
/// Three mechanisms create slide breaks:
/// 1. `---` with blank lines on both sides
/// 2. Three or more consecutive blank lines (4+ newlines)
/// 3. A `# ` heading when the current slide already has content
pub fn split(body: &str) -> Vec<String> {
    // Phase 1: Replace explicit --- separators and blank-line gaps with a sentinel
    let sentinel = "\x00SLIDE_BREAK\x00";

    // Normalize line endings
    let body = body.replace("\r\n", "\n");

    // Split into lines first
    let lines: Vec<String> = body.split('\n').map(String::from).collect();

    // Process lines to detect separators
    let mut i = 0;
    let mut output_lines: Vec<String> = Vec::new();
    while i < lines.len() {
        let line = &lines[i];
        let trimmed = line.trim();

        // Check for --- separator with blank lines around it
        if is_dash_separator(trimmed) {
            // Check if previous line is blank and next line is blank
            let prev_blank = i == 0
                || output_lines
                    .last()
                    .is_some_and(|l: &String| l.trim().is_empty())
                || (!output_lines.is_empty() && output_lines.last().is_some_and(|l| l == sentinel));
            let next_blank =
                i + 1 >= lines.len() || lines.get(i + 1).is_some_and(|l| l.trim().is_empty());

            if prev_blank && next_blank {
                // Remove trailing blank line from output if present
                if output_lines.last().is_some_and(|l| l.trim().is_empty()) {
                    output_lines.pop();
                }
                output_lines.push(sentinel.to_string());
                // Skip next blank line
                if i + 1 < lines.len() && lines[i + 1].trim().is_empty() {
                    i += 1;
                }
                i += 1;
                continue;
            }
        }

        output_lines.push(line.clone());
        i += 1;
    }

    // Phase 2: Replace 3+ consecutive blank lines with sentinel
    let mut final_lines: Vec<String> = Vec::new();
    let mut blank_count = 0;
    for line in &output_lines {
        if line == sentinel {
            blank_count = 0;
            final_lines.push(line.clone());
            continue;
        }
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count < 3 {
                final_lines.push(line.clone());
            } else if blank_count == 3 {
                // Remove the 2 blank lines we already added
                final_lines.pop();
                final_lines.pop();
                final_lines.push(sentinel.to_string());
            }
            // else: more blank lines, skip them
        } else {
            blank_count = 0;
            final_lines.push(line.clone());
        }
    }

    // Rejoin into a string
    let result = final_lines.join("\n");

    // Phase 3: Split by sentinel
    let chunks: Vec<String> = result
        .split(sentinel)
        .map(|s| s.trim().to_string())
        .collect();

    // Phase 4: Apply heading inference within each chunk
    let mut slides: Vec<String> = Vec::new();
    for chunk in chunks {
        if chunk.is_empty() {
            continue;
        }
        split_by_heading_inference(&chunk, &mut slides);
    }

    slides
}

/// Split a chunk by H1 heading inference: when `# ` appears at the start of a line
/// and the current slide already has content, insert a break.
/// Lines inside fenced code blocks are never treated as headings.
fn split_by_heading_inference(chunk: &str, slides: &mut Vec<String>) {
    let mut current = String::new();
    let mut has_content = false;
    let mut in_code_fence = false;
    let mut fence_char: char = '`';
    let mut fence_len: usize = 0;

    for line in chunk.lines() {
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

        if !in_code_fence && line.starts_with("# ") && has_content {
            // This H1 starts a new slide.
            // Move any trailing directives from the old slide to the new one,
            // since `@layout: X` placed just before a `# Heading` belongs to
            // the heading's slide.
            let slide_text = current.trim().to_string();
            let (content_part, trailing_directives) = strip_trailing_directives(&slide_text);
            if !content_part.is_empty() {
                slides.push(content_part);
            }
            current = String::new();
            if !trailing_directives.is_empty() {
                current.push_str(&trailing_directives);
                current.push('\n');
            }
            has_content = false;
        }

        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);

        // Directives (@key: value) don't count as content for heading inference
        if !trimmed.is_empty() && !is_directive(trimmed) {
            has_content = true;
        }
    }

    let slide_text = current.trim().to_string();
    if !slide_text.is_empty() {
        slides.push(slide_text);
    }
}

/// Split trailing directive lines (and blank lines before them) from a slide's raw text.
/// Returns `(content, directives)` where `directives` contains only `@key: value` lines.
fn strip_trailing_directives(text: &str) -> (String, String) {
    let lines: Vec<&str> = text.lines().collect();

    // Walk backwards from the end, collecting contiguous directive / blank lines
    let mut split_at = lines.len();
    for i in (0..lines.len()).rev() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty() || is_directive(trimmed) {
            split_at = i;
        } else {
            break;
        }
    }

    if split_at == lines.len() {
        // Nothing to strip
        return (text.to_string(), String::new());
    }

    let content = lines[..split_at].join("\n").trim().to_string();
    let directives: String = lines[split_at..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .copied()
        .collect::<Vec<&str>>()
        .join("\n");

    (content, directives)
}

fn is_dash_separator(line: &str) -> bool {
    line.len() >= 3 && line.chars().all(|c| c == '-')
}

fn is_directive(line: &str) -> bool {
    line.starts_with('@')
        && line.contains(':')
        && line[1..line.find(':').unwrap()]
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blank_line_split() {
        let body = "Slide one\n\n\n\nSlide two";
        let slides = split(body);
        assert_eq!(slides.len(), 2);
        assert_eq!(slides[0], "Slide one");
        assert_eq!(slides[1], "Slide two");
    }

    #[test]
    fn test_dash_separator() {
        let body = "Slide one\n\n---\n\nSlide two";
        let slides = split(body);
        assert_eq!(slides.len(), 2);
        assert_eq!(slides[0], "Slide one");
        assert_eq!(slides[1], "Slide two");
    }

    #[test]
    fn test_heading_inference() {
        let body = "# First\n\nContent\n\n# Second\n\nMore content";
        let slides = split(body);
        assert_eq!(slides.len(), 2);
        assert!(slides[0].starts_with("# First"));
        assert!(slides[1].starts_with("# Second"));
    }

    #[test]
    fn test_h2_no_split() {
        let body = "# Title\n\n## Subtitle\n\nContent";
        let slides = split(body);
        assert_eq!(slides.len(), 1);
    }

    #[test]
    fn test_heading_inference_first_heading() {
        // First heading shouldn't split (no prior content)
        let body = "# Only Heading\n\nContent here";
        let slides = split(body);
        assert_eq!(slides.len(), 1);
    }

    #[test]
    fn test_combined_separators() {
        let body = "Slide one\n\n\n\n---\n\n\n\nSlide two";
        let slides = split(body);
        // Should produce 2 slides, not 3 (overlapping separators = single break)
        assert_eq!(slides.len(), 2);
    }

    #[test]
    fn test_directive_before_heading_moves_to_next_slide() {
        let body = "# Title\n\nSubtitle\n\n@layout: two-column\n# Second Slide\n\nContent";
        let slides = split(body);
        assert_eq!(slides.len(), 2, "Expected 2 slides, got {}", slides.len());
        // Directive should NOT be on the first slide
        assert!(
            !slides[0].contains("@layout"),
            "First slide should not contain @layout directive: {}",
            slides[0]
        );
        // Directive should be on the second slide (before the heading)
        assert!(
            slides[1].contains("@layout: two-column"),
            "Second slide should start with @layout directive: {}",
            slides[1]
        );
    }

    #[test]
    fn test_heading_in_code_block_no_split() {
        let body = "# Title\n\n```python\n# this is a comment\nprint('hi')\n```";
        let slides = split(body);
        assert_eq!(
            slides.len(),
            1,
            "Hash comment in code block should not split"
        );
    }

    #[test]
    fn test_poker_night_slide_count() {
        let content = include_str!("../../../../sample-presentations/poker-night.md");
        // Strip frontmatter
        let (_, body) = super::super::frontmatter::extract(content);
        let slides = split(&body);
        assert!(
            slides.len() >= 14,
            "Expected at least 14 slides, got {}",
            slides.len()
        );
    }
}
