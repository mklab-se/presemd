use super::{Block, Directive, ImageDirectives, Inline, ListItem, ListMarker};

/// Extract @ directives from the beginning of a slide's raw text.
/// Returns (directives, remaining content).
pub fn extract_directives(raw: &str) -> (Vec<Directive>, String) {
    let mut directives = Vec::new();
    let mut remaining_lines = Vec::new();
    let mut past_directives = false;

    for line in raw.lines() {
        if !past_directives {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(directive) = parse_directive_line(trimmed) {
                directives.push(directive);
                continue;
            }
            past_directives = true;
        }
        remaining_lines.push(line);
    }

    (directives, remaining_lines.join("\n"))
}

fn parse_directive_line(line: &str) -> Option<Directive> {
    if !line.starts_with('@') {
        return None;
    }
    let after_at = &line[1..];
    let colon_pos = after_at.find(':')?;
    let name = after_at[..colon_pos].trim().to_string();
    let value = after_at[colon_pos + 1..].trim().to_string();

    // Validate: name should be word characters and hyphens
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }

    Some(Directive { name, value })
}

/// Parse a slide's content string into a Vec<Block>.
pub fn parse(content: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Skip blank lines
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // Column separator: +++
        if trimmed == "+++" {
            blocks.push(Block::ColumnSeparator);
            i += 1;
            continue;
        }

        // Horizontal rule: *** or ___
        if is_horizontal_rule(trimmed) {
            blocks.push(Block::HorizontalRule);
            i += 1;
            continue;
        }

        // Heading: # ...
        if let Some(heading) = parse_heading(trimmed) {
            blocks.push(heading);
            i += 1;
            continue;
        }

        // Fenced code block: ``` or ~~~
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let fence_char = if trimmed.starts_with("```") { '`' } else { '~' };
            let (block, end) = parse_code_block(&lines, i, fence_char);
            blocks.push(block);
            i = end;
            continue;
        }

        // Image: ![alt](path)
        if trimmed.starts_with("![") {
            if let Some(img) = parse_image(trimmed) {
                blocks.push(img);
                i += 1;
                continue;
            }
        }

        // Blockquote: > ...
        if trimmed.starts_with("> ") || trimmed == ">" {
            let (block, end) = parse_blockquote(&lines, i);
            blocks.push(block);
            i = end;
            continue;
        }

        // Table: | ... |
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let (block, end) = parse_table(&lines, i);
            if let Some(table) = block {
                blocks.push(table);
            }
            i = end;
            continue;
        }

        // Unordered list: - or + or *  (but not --- or ***)
        if is_list_start(trimmed) {
            let (block, end) = parse_list(&lines, i, false);
            blocks.push(block);
            i = end;
            continue;
        }

        // Ordered list: 1. ...
        if is_ordered_list_start(trimmed) {
            let (block, end) = parse_list(&lines, i, true);
            blocks.push(block);
            i = end;
            continue;
        }

        // Paragraph: collect consecutive non-blank, non-special lines
        let (block, end) = parse_paragraph(&lines, i);
        blocks.push(block);
        i = end;
    }

    blocks
}

fn is_horizontal_rule(line: &str) -> bool {
    if line.len() < 3 {
        return false;
    }
    let chars: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    if chars.len() < 3 {
        return false;
    }
    let first = chars[0];
    (first == '*' || first == '_') && chars.iter().all(|&c| c == first)
}

fn parse_heading(line: &str) -> Option<Block> {
    if !line.starts_with('#') {
        return None;
    }

    let mut level = 0u8;
    for ch in line.chars() {
        if ch == '#' {
            level += 1;
        } else {
            break;
        }
    }

    if level == 0 || level > 6 {
        return None;
    }

    let rest = &line[level as usize..];
    if !rest.starts_with(' ') && !rest.is_empty() {
        return None;
    }

    let text = rest.trim();
    let inlines = super::inline::parse(text);
    Some(Block::Heading { level, inlines })
}

fn parse_code_block(lines: &[&str], start: usize, fence_char: char) -> (Block, usize) {
    let opening = lines[start].trim();
    let fence_prefix: String = opening.chars().take_while(|&c| c == fence_char).collect();
    let fence_len = fence_prefix.len();

    // Parse language and highlight spec from opening line
    let after_fence = &opening[fence_len..];
    let (language, highlight_lines, viz_kind) = parse_code_info(after_fence.trim());

    let mut code_lines = Vec::new();
    let mut i = start + 1;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        // Check for closing fence
        let closing_count = trimmed.chars().take_while(|&c| c == fence_char).count();
        if closing_count >= fence_len
            && trimmed
                .chars()
                .skip(closing_count)
                .all(|c| c.is_whitespace())
        {
            i += 1;
            break;
        }
        code_lines.push(lines[i]);
        i += 1;
    }

    let code = code_lines.join("\n");

    let block = match viz_kind {
        VizKind::Diagram => Block::Diagram { content: code },
        VizKind::WordCloud => Block::WordCloud { content: code },
        VizKind::Timeline => Block::Timeline { content: code },
        VizKind::PieChart => Block::PieChart { content: code },
        VizKind::BarChart => Block::BarChart { content: code },
        VizKind::LineChart => Block::LineChart { content: code },
        VizKind::DonutChart => Block::DonutChart { content: code },
        VizKind::KpiCards => Block::KpiCards { content: code },
        VizKind::FunnelChart => Block::FunnelChart { content: code },
        VizKind::RadarChart => Block::RadarChart { content: code },
        VizKind::StackedBar => Block::StackedBar { content: code },
        VizKind::VennDiagram => Block::VennDiagram { content: code },
        VizKind::ProgressBars => Block::ProgressBars { content: code },
        VizKind::ScatterPlot => Block::ScatterPlot { content: code },
        VizKind::OrgChart => Block::OrgChart { content: code },
        VizKind::GanttChart => Block::GanttChart { content: code },
        VizKind::None => Block::CodeBlock {
            language,
            code,
            highlight_lines,
        },
    };
    (block, i)
}

/// Which visualization type a code block represents (if any).
#[derive(Debug, Clone, Copy, PartialEq)]
enum VizKind {
    None,
    Diagram,
    WordCloud,
    Timeline,
    PieChart,
    BarChart,
    LineChart,
    DonutChart,
    KpiCards,
    FunnelChart,
    RadarChart,
    StackedBar,
    VennDiagram,
    ProgressBars,
    ScatterPlot,
    OrgChart,
    GanttChart,
}

fn parse_code_info(info: &str) -> (Option<String>, Vec<usize>, VizKind) {
    if info.is_empty() {
        return (None, vec![], VizKind::None);
    }

    // Check for visualization language tags
    if info.starts_with("@architecture") {
        return (None, vec![], VizKind::Diagram);
    }
    if info.starts_with("@wordcloud") {
        return (None, vec![], VizKind::WordCloud);
    }
    if info.starts_with("@timeline") {
        return (None, vec![], VizKind::Timeline);
    }
    if info.starts_with("@piechart") {
        return (None, vec![], VizKind::PieChart);
    }
    if info.starts_with("@barchart") {
        return (None, vec![], VizKind::BarChart);
    }
    if info.starts_with("@linechart") {
        return (None, vec![], VizKind::LineChart);
    }
    if info.starts_with("@donut") {
        return (None, vec![], VizKind::DonutChart);
    }
    if info.starts_with("@kpi") {
        return (None, vec![], VizKind::KpiCards);
    }
    if info.starts_with("@funnel") {
        return (None, vec![], VizKind::FunnelChart);
    }
    if info.starts_with("@radar") {
        return (None, vec![], VizKind::RadarChart);
    }
    if info.starts_with("@stackedbar") {
        return (None, vec![], VizKind::StackedBar);
    }
    if info.starts_with("@venn") {
        return (None, vec![], VizKind::VennDiagram);
    }
    if info.starts_with("@progress") {
        return (None, vec![], VizKind::ProgressBars);
    }
    if info.starts_with("@scatter") {
        return (None, vec![], VizKind::ScatterPlot);
    }
    if info.starts_with("@orgchart") {
        return (None, vec![], VizKind::OrgChart);
    }
    if info.starts_with("@gantt") {
        return (None, vec![], VizKind::GanttChart);
    }

    // Parse language and optional highlight spec
    let (lang_part, highlight_part) = if let Some(brace_start) = info.find('{') {
        let lang = info[..brace_start].trim();
        let rest = &info[brace_start..];
        let highlight = if let Some(brace_end) = rest.find('}') {
            parse_highlight_spec(&rest[1..brace_end])
        } else {
            vec![]
        };
        (lang, highlight)
    } else {
        (info.split_whitespace().next().unwrap_or(""), vec![])
    };

    let language = if lang_part.is_empty() {
        None
    } else {
        Some(lang_part.to_string())
    };

    (language, highlight_part, VizKind::None)
}

fn parse_highlight_spec(spec: &str) -> Vec<usize> {
    let mut lines = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            if let (Ok(s), Ok(e)) = (start.trim().parse::<usize>(), end.trim().parse::<usize>()) {
                for line in s..=e {
                    lines.push(line);
                }
            }
        } else if let Ok(n) = part.parse::<usize>() {
            lines.push(n);
        }
    }
    lines
}

fn parse_image(line: &str) -> Option<Block> {
    // ![alt](path)
    if !line.starts_with("![") {
        return None;
    }

    let close_bracket = line.find("](")?;
    let alt_full = &line[2..close_bracket];

    let paren_start = close_bracket + 2;
    let paren_end = line[paren_start..].find(')')? + paren_start;
    let path = line[paren_start..paren_end].to_string();

    // Extract directives from alt text
    let (alt, directives) = parse_image_alt(alt_full);

    Some(Block::Image {
        alt,
        path,
        directives,
    })
}

fn parse_image_alt(alt_full: &str) -> (String, ImageDirectives) {
    let mut directives = ImageDirectives::default();
    let mut alt_parts = Vec::new();

    for word in alt_full.split_whitespace() {
        if let Some(directive) = word.strip_prefix('@') {
            if directive == "fill" {
                directives.fill = true;
            } else if directive == "fit" {
                directives.fit = true;
            } else if directive == "left" {
                directives.align = Some("left".to_string());
            } else if directive == "right" {
                directives.align = Some("right".to_string());
            } else if directive == "center" {
                directives.align = Some("center".to_string());
            } else if let Some(val) = directive.strip_prefix("width:") {
                directives.width = Some(val.to_string());
            } else if let Some(val) = directive.strip_prefix("height:") {
                directives.height = Some(val.to_string());
            }
        } else {
            alt_parts.push(word);
        }
    }

    (alt_parts.join(" "), directives)
}

fn parse_blockquote(lines: &[&str], start: usize) -> (Block, usize) {
    let mut quote_text = String::new();
    let mut i = start;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        if let Some(rest) = trimmed.strip_prefix("> ") {
            if !quote_text.is_empty() {
                quote_text.push(' ');
            }
            quote_text.push_str(rest);
            i += 1;
        } else if trimmed == ">" {
            if !quote_text.is_empty() {
                quote_text.push(' ');
            }
            i += 1;
        } else {
            break;
        }
    }

    let inlines = super::inline::parse(&quote_text);
    (Block::BlockQuote { inlines }, i)
}

fn parse_table(lines: &[&str], start: usize) -> (Option<Block>, usize) {
    let mut table_lines: Vec<&str> = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with('|') {
            table_lines.push(trimmed);
            i += 1;
        } else if trimmed.is_empty() {
            i += 1;
            break;
        } else {
            break;
        }
    }

    if table_lines.len() < 2 {
        return (None, i);
    }

    // First line = headers
    let headers = parse_table_row(table_lines[0]);

    // Second line = separator (skip)
    // Remaining lines = data rows
    let rows: Vec<Vec<Vec<Inline>>> = table_lines
        .iter()
        .skip(2)
        .map(|line| parse_table_row(line))
        .collect();

    (Some(Block::Table { headers, rows }), i)
}

fn parse_table_row(line: &str) -> Vec<Vec<Inline>> {
    let trimmed = line.trim().trim_matches('|');
    trimmed
        .split('|')
        .map(|cell| super::inline::parse(cell.trim()))
        .collect()
}

fn is_list_start(line: &str) -> bool {
    if line.len() < 2 {
        return false;
    }
    let first = line.chars().next().unwrap();
    let second = line.chars().nth(1).unwrap();
    (first == '-' || first == '+' || first == '*') && second == ' '
}

fn is_ordered_list_start(line: &str) -> bool {
    let Some(dot_pos) = line.find(". ") else {
        return false;
    };
    line[..dot_pos].trim().chars().all(|c| c.is_ascii_digit()) && dot_pos > 0
}

fn parse_list(lines: &[&str], start: usize, ordered: bool) -> (Block, usize) {
    let mut items: Vec<ListItem> = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            // Check if next non-blank line continues the list
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if j < lines.len() {
                let next = lines[j].trim();
                let indent = line_indent(lines[j]);
                if (indent > 0 && (is_list_start(next) || is_ordered_list_start(next)))
                    || is_list_start(next)
                    || is_ordered_list_start(next)
                {
                    i = j;
                    continue;
                }
            }
            break;
        }

        let indent = line_indent(line);

        if indent == 0 {
            // Top-level item
            if ordered {
                if let Some((text, marker)) = extract_ordered_item(trimmed) {
                    items.push(ListItem {
                        marker,
                        inlines: super::inline::parse(text),
                        children: Vec::new(),
                    });
                    i += 1;
                    // Collect nested items
                    let (children, new_i) = collect_children(lines, i, 0);
                    if let Some(last) = items.last_mut() {
                        last.children = children;
                    }
                    i = new_i;
                    continue;
                }
                break;
            } else if let Some((text, marker)) = extract_unordered_item(trimmed) {
                items.push(ListItem {
                    marker,
                    inlines: super::inline::parse(text),
                    children: Vec::new(),
                });
                i += 1;
                // Collect nested items
                let (children, new_i) = collect_children(lines, i, 0);
                if let Some(last) = items.last_mut() {
                    last.children = children;
                }
                i = new_i;
                continue;
            } else {
                break;
            }
        } else {
            // This is a nested item that belongs to the last top-level item
            // This case is handled by collect_children, so we shouldn't get here
            // in normal flow. But just in case, treat it as continuation.
            if let Some((text, marker)) = extract_any_list_item(trimmed) {
                if let Some(last) = items.last_mut() {
                    last.children.push(ListItem {
                        marker,
                        inlines: super::inline::parse(text),
                        children: Vec::new(),
                    });
                }
                i += 1;
                continue;
            }
            break;
        }
    }

    (Block::List { ordered, items }, i)
}

fn collect_children(lines: &[&str], start: usize, parent_indent: usize) -> (Vec<ListItem>, usize) {
    let mut children = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        let indent = line_indent(line);
        if indent <= parent_indent {
            break;
        }

        if let Some((text, marker)) = extract_any_list_item(trimmed) {
            children.push(ListItem {
                marker,
                inlines: super::inline::parse(text),
                children: Vec::new(),
            });
            i += 1;

            // Recursively collect deeper children
            let (sub_children, new_i) = collect_children(lines, i, indent);
            if let Some(last) = children.last_mut() {
                last.children = sub_children;
            }
            i = new_i;
        } else {
            break;
        }
    }

    (children, i)
}

fn extract_unordered_item(line: &str) -> Option<(&str, ListMarker)> {
    if line.len() < 2 {
        return None;
    }
    let first = line.chars().next()?;
    let second = line.chars().nth(1)?;
    if second != ' ' {
        return None;
    }
    let marker = match first {
        '-' => ListMarker::Static,
        '+' => ListMarker::NextStep,
        '*' => ListMarker::WithPrev,
        _ => return None,
    };
    Some((&line[2..], marker))
}

fn extract_ordered_item(line: &str) -> Option<(&str, ListMarker)> {
    let dot_pos = line.find(". ")?;
    if dot_pos == 0 || !line[..dot_pos].chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((&line[dot_pos + 2..], ListMarker::Ordered))
}

fn extract_any_list_item(line: &str) -> Option<(&str, ListMarker)> {
    extract_unordered_item(line).or_else(|| extract_ordered_item(line))
}

fn line_indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn parse_paragraph(lines: &[&str], start: usize) -> (Block, usize) {
    let mut text = String::new();
    let mut i = start;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Stop at blank lines or special block starts
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("```")
            || trimmed.starts_with("~~~")
            || trimmed.starts_with("![")
            || trimmed.starts_with("> ")
            || trimmed == ">"
            || trimmed == "+++"
            || is_horizontal_rule(trimmed)
            || (trimmed.starts_with('|') && trimmed.ends_with('|'))
            || is_list_start(trimmed)
            || is_ordered_list_start(trimmed)
        {
            break;
        }

        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(trimmed);
        i += 1;
    }

    let inlines = super::inline::parse(&text);
    (Block::Paragraph { inlines }, i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_directives() {
        let raw = "@layout: two-column\n@theme: dark\n\n# Title\n\nContent";
        let (dirs, content) = extract_directives(raw);
        assert_eq!(dirs.len(), 2);
        assert_eq!(dirs[0].name, "layout");
        assert_eq!(dirs[0].value, "two-column");
        assert_eq!(dirs[1].name, "theme");
        assert_eq!(dirs[1].value, "dark");
        assert!(content.contains("# Title"));
    }

    #[test]
    fn test_parse_heading() {
        let blocks = parse("# Title");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], Block::Heading { level: 1, .. }));
    }

    #[test]
    fn test_parse_code_block() {
        let blocks = parse("```rust\nfn main() {}\n```");
        assert_eq!(blocks.len(), 1);
        if let Block::CodeBlock { language, code, .. } = &blocks[0] {
            assert_eq!(language.as_deref(), Some("rust"));
            assert_eq!(code, "fn main() {}");
        } else {
            panic!("Expected CodeBlock");
        }
    }

    #[test]
    fn test_parse_diagram_block() {
        let blocks = parse("```@architecture\n- A -> B: hello\n```");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], Block::Diagram { .. }));
    }

    #[test]
    fn test_parse_image() {
        let blocks = parse("![Photo @fill](photo.jpg)");
        assert_eq!(blocks.len(), 1);
        if let Block::Image {
            alt,
            path,
            directives,
        } = &blocks[0]
        {
            assert_eq!(alt, "Photo");
            assert_eq!(path, "photo.jpg");
            assert!(directives.fill);
        } else {
            panic!("Expected Image");
        }
    }

    #[test]
    fn test_parse_image_width() {
        let blocks = parse("![Diagram @width:80%](diagram.png)");
        assert_eq!(blocks.len(), 1);
        if let Block::Image { directives, .. } = &blocks[0] {
            assert_eq!(directives.width.as_deref(), Some("80%"));
        } else {
            panic!("Expected Image");
        }
    }

    #[test]
    fn test_parse_blockquote() {
        let blocks = parse("> This is a quote\n> with multiple lines");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], Block::BlockQuote { .. }));
    }

    #[test]
    fn test_parse_table() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let blocks = parse(input);
        assert_eq!(blocks.len(), 1);
        if let Block::Table { headers, rows } = &blocks[0] {
            assert_eq!(headers.len(), 2);
            assert_eq!(rows.len(), 1);
        } else {
            panic!("Expected Table");
        }
    }

    #[test]
    fn test_parse_unordered_list() {
        let blocks = parse("- First\n- Second\n- Third");
        assert_eq!(blocks.len(), 1);
        if let Block::List { ordered, items } = &blocks[0] {
            assert!(!ordered);
            assert_eq!(items.len(), 3);
        } else {
            panic!("Expected List");
        }
    }

    #[test]
    fn test_parse_list_markers() {
        let blocks = parse("- Static\n+ Next\n* WithPrev");
        assert_eq!(blocks.len(), 1);
        if let Block::List { items, .. } = &blocks[0] {
            assert_eq!(items[0].marker, ListMarker::Static);
            assert_eq!(items[1].marker, ListMarker::NextStep);
            assert_eq!(items[2].marker, ListMarker::WithPrev);
        } else {
            panic!("Expected List");
        }
    }

    #[test]
    fn test_parse_horizontal_rule() {
        let blocks = parse("Some text\n\n***\n\nMore text");
        assert!(blocks.iter().any(|b| matches!(b, Block::HorizontalRule)));
    }

    #[test]
    fn test_parse_column_separator() {
        let blocks = parse("Left content\n\n+++\n\nRight content");
        assert!(blocks.iter().any(|b| matches!(b, Block::ColumnSeparator)));
    }

    #[test]
    fn test_highlight_spec() {
        let result = parse_highlight_spec("3,5-7");
        assert_eq!(result, vec![3, 5, 6, 7]);
    }

    #[test]
    fn test_nested_list() {
        let blocks = parse("- Parent\n  - Child\n    - Grandchild");
        assert_eq!(blocks.len(), 1);
        if let Block::List { items, .. } = &blocks[0] {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].children.len(), 1);
            assert_eq!(items[0].children[0].children.len(), 1);
        } else {
            panic!("Expected List");
        }
    }
}
