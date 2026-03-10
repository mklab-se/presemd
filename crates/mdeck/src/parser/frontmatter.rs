use super::PresentationMeta;
use std::collections::HashMap;

pub fn extract(content: &str) -> (PresentationMeta, String) {
    let trimmed = content.trim_start_matches('\u{feff}'); // strip BOM

    if !trimmed.starts_with("---\n") && !trimmed.starts_with("---\r\n") {
        return (PresentationMeta::default(), trimmed.to_string());
    }

    // Skip the opening "---\n" or "---\r\n"
    let after_opening = if let Some(rest) = trimmed.strip_prefix("---\r\n") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("---\n") {
        rest
    } else {
        return (PresentationMeta::default(), trimmed.to_string());
    };

    // Find closing ---
    let closing = find_closing_delimiter(after_opening);
    let Some(end_pos) = closing else {
        return (PresentationMeta::default(), trimmed.to_string());
    };

    let yaml_str = &after_opening[..end_pos];
    let rest_start = end_pos
        + after_opening[end_pos..]
            .find('\n')
            .unwrap_or(after_opening.len() - end_pos)
        + 1;
    let body = if rest_start < after_opening.len() {
        &after_opening[rest_start..]
    } else {
        ""
    };

    let meta = parse_frontmatter(yaml_str);
    (meta, body.to_string())
}

fn find_closing_delimiter(s: &str) -> Option<usize> {
    for (i, line) in s.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed == "---" && i > 0 {
            // Calculate byte offset
            let mut offset = 0;
            for (j, l) in s.lines().enumerate() {
                if j == i {
                    return Some(offset);
                }
                offset += l.len() + 1; // +1 for newline
            }
        }
    }
    None
}

fn parse_frontmatter(yaml_str: &str) -> PresentationMeta {
    // Try to parse as YAML HashMap
    let map: HashMap<String, serde_yaml::Value> = match serde_yaml::from_str(yaml_str) {
        Ok(m) => m,
        Err(_) => return parse_frontmatter_manual(yaml_str),
    };

    PresentationMeta {
        title: get_string(&map, "title"),
        author: get_string(&map, "author"),
        date: map.get("date").map(|v| match v {
            serde_yaml::Value::String(s) => s.clone(),
            other => format!("{other:?}"),
        }),
        theme: get_string(&map, "@theme"),
        transition: get_string(&map, "@transition"),
        aspect: get_string(&map, "@aspect"),
        code_theme: get_string(&map, "@code-theme"),
        footer: get_string(&map, "@footer"),
        image_style: get_string(&map, "@image-style"),
        icon_style: get_string(&map, "@icon-style"),
    }
}

fn get_string(map: &HashMap<String, serde_yaml::Value>, key: &str) -> Option<String> {
    map.get(key).and_then(|v| match v {
        serde_yaml::Value::String(s) => Some(s.clone()),
        _ => None,
    })
}

/// Fallback: parse key: value lines manually
fn parse_frontmatter_manual(yaml_str: &str) -> PresentationMeta {
    let mut meta = PresentationMeta::default();
    for line in yaml_str.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "title" => meta.title = Some(value.to_string()),
                "author" => meta.author = Some(value.to_string()),
                "date" => meta.date = Some(value.to_string()),
                "@theme" => meta.theme = Some(value.to_string()),
                "@transition" => meta.transition = Some(value.to_string()),
                "@aspect" => meta.aspect = Some(value.to_string()),
                "@code-theme" => meta.code_theme = Some(value.to_string()),
                "@footer" => meta.footer = Some(value.to_string()),
                "@image-style" => meta.image_style = Some(value.to_string()),
                "@icon-style" => meta.icon_style = Some(value.to_string()),
                _ => {}
            }
        }
    }
    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_frontmatter() {
        let content = "---\ntitle: \"Hello\"\nauthor: \"Test\"\n@theme: dark\n---\n\n# Slide";
        let (meta, body) = extract(content);
        assert_eq!(meta.title.as_deref(), Some("Hello"));
        assert_eq!(meta.author.as_deref(), Some("Test"));
        assert_eq!(meta.theme.as_deref(), Some("dark"));
        assert!(body.contains("# Slide"));
    }

    #[test]
    fn test_no_frontmatter() {
        let content = "# Just a slide\n\nSome content";
        let (meta, body) = extract(content);
        assert!(meta.title.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn test_frontmatter_with_all_fields() {
        let content = "---\ntitle: \"Test\"\nauthor: \"Author\"\ndate: 2026-02-28\n@theme: light\n@transition: fade\n@aspect: 16:9\n@footer: \"footer text\"\n---\nBody";
        let (meta, body) = extract(content);
        assert_eq!(meta.title.as_deref(), Some("Test"));
        assert_eq!(meta.theme.as_deref(), Some("light"));
        assert_eq!(meta.transition.as_deref(), Some("fade"));
        assert_eq!(meta.aspect.as_deref(), Some("16:9"));
        assert_eq!(meta.footer.as_deref(), Some("footer text"));
        assert_eq!(body.trim(), "Body");
    }

    #[test]
    fn test_frontmatter_image_style() {
        let content = "---\ntitle: \"Test\"\n@image-style: Pixar\n@icon-style: minimal\n---\nBody";
        let (meta, body) = extract(content);
        assert_eq!(meta.title.as_deref(), Some("Test"));
        assert_eq!(meta.image_style.as_deref(), Some("Pixar"));
        assert_eq!(meta.icon_style.as_deref(), Some("minimal"));
        assert_eq!(body.trim(), "Body");
    }

    #[test]
    fn test_frontmatter_date_not_string() {
        let content = "---\ntitle: \"Test\"\ndate: 2026-02-28\n---\nBody";
        let (meta, _body) = extract(content);
        assert!(meta.date.is_some());
    }
}
