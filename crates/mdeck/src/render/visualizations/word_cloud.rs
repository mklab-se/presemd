use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use eframe::egui::{self, Color32, FontId, Pos2};

use crate::theme::Theme;

use super::{VizReveal, assign_steps, parse_reveal_prefix};

// ─── Cache ──────────────────────────────────────────────────────────────────

/// Cached word positions so layout is stable across frames.
static LAYOUT_CACHE: LazyLock<Mutex<std::collections::HashMap<u64, Vec<WordLayout>>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

pub fn clear_cache() {
    LAYOUT_CACHE.lock().unwrap().clear();
}

#[derive(Debug, Clone)]
struct WordLayout {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    font_size: f32,
}

// ─── Parsing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct WordEntry {
    text: String,
    size: f32,
    reveal: VizReveal,
}

fn parse_word_cloud(content: &str) -> Vec<WordEntry> {
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (text, reveal) = parse_reveal_prefix(trimmed);
        if text.is_empty() {
            continue;
        }

        // Parse "Word (size: N)" or "Word"
        let (label, size) = if let Some(paren_start) = text.find('(') {
            let before = text[..paren_start].trim();
            let meta = &text[paren_start..];
            let size = parse_size_meta(meta).unwrap_or(20.0);
            (before.to_string(), size)
        } else {
            (text.to_string(), 20.0)
        };

        if !label.is_empty() {
            entries.push(WordEntry {
                text: label,
                size,
                reveal,
            });
        }
    }
    entries
}

fn parse_size_meta(meta: &str) -> Option<f32> {
    let inner = meta.trim_start_matches('(').trim_end_matches(')');
    for part in inner.split(',') {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("size:") {
            if let Ok(s) = val.trim().parse::<f32>() {
                return Some(s);
            }
        }
    }
    None
}

fn cache_key(content: &str, width_bits: u32, height_bits: u32) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    width_bits.hash(&mut hasher);
    height_bits.hash(&mut hasher);
    hasher.finish()
}

// ─── Layout algorithm ───────────────────────────────────────────────────────

/// Compute font sizes that fill the available area densely.
/// The largest word's size value maps to a font that takes up a significant
/// portion of the area; other words scale proportionally.
fn compute_font_sizes(
    entries: &[WordEntry],
    area_width: f32,
    area_height: f32,
    scale: f32,
) -> Vec<f32> {
    if entries.is_empty() {
        return vec![];
    }

    let max_size = entries
        .iter()
        .map(|e| e.size)
        .fold(0.0f32, f32::max)
        .max(1.0);
    let min_size = entries
        .iter()
        .map(|e| e.size)
        .fold(f32::MAX, f32::min)
        .max(1.0);

    // The largest word should have a font size that's roughly 1/4 to 1/3 of
    // the area height. Smaller words scale down proportionally but with a
    // minimum floor so they remain legible.
    let max_font = (area_height * 0.28).min(area_width * 0.12);
    let min_font = max_font * 0.15; // smallest word is ~15% of largest

    entries
        .iter()
        .map(|e| {
            if (max_size - min_size).abs() < 0.001 {
                max_font * scale
            } else {
                let t = (e.size - min_size) / (max_size - min_size);
                // Use power curve so medium words are still fairly large
                let t_curved = t.powf(0.6);
                (min_font + t_curved * (max_font - min_font)) * scale
            }
        })
        .collect()
}

struct PlaceCtx {
    cx: f32,
    cy: f32,
    area_width: f32,
    area_height: f32,
    scale: f32,
}

/// Try to place a word using spiral search. Returns None if no valid position found.
fn spiral_place(
    ctx: &PlaceCtx,
    w: f32,
    h: f32,
    placed: &[WordLayout],
    pad: f32,
) -> Option<(f32, f32)> {
    let mut t = 0.0f32;
    let t_step = 0.08; // finer steps for better packing
    let growth = 1.0 * ctx.scale;
    // Horizontal stretch ratio to fill widescreen areas
    let aspect = (ctx.area_width / ctx.area_height).max(1.0);

    for _ in 0..10000 {
        let angle = t * 2.5;
        let r = t * growth;
        let x = ctx.cx + r * angle.cos() * aspect - w / 2.0;
        let y = ctx.cy + r * angle.sin() - h / 2.0;

        if x >= 0.0 && y >= 0.0 && x + w <= ctx.area_width && y + h <= ctx.area_height {
            let overlaps = placed.iter().any(|p| {
                x < p.x + p.width + pad
                    && x + w + pad > p.x
                    && y < p.y + p.height + pad
                    && y + h + pad > p.y
            });
            if !overlaps {
                return Some((x, y));
            }
        }
        t += t_step;
    }
    None
}

/// Dense spiral placement: place largest words first at center, pack tightly.
/// If a word can't be placed at its target size, progressively shrink it.
fn compute_layout(
    ui: &egui::Ui,
    entries: &[WordEntry],
    area_width: f32,
    area_height: f32,
    scale: f32,
) -> Vec<WordLayout> {
    let font_sizes = compute_font_sizes(entries, area_width, area_height, scale);

    // Sort by font size descending (place largest first)
    let mut sorted_indices: Vec<usize> = (0..entries.len()).collect();
    sorted_indices.sort_by(|a, b| {
        font_sizes[*b]
            .partial_cmp(&font_sizes[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut placed: Vec<WordLayout> = Vec::new();
    let mut result = vec![
        WordLayout {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            font_size: 0.0,
        };
        entries.len()
    ];

    let ctx = PlaceCtx {
        cx: area_width / 2.0,
        cy: area_height / 2.0,
        area_width,
        area_height,
        scale,
    };

    for &orig_idx in &sorted_indices {
        let entry = &entries[orig_idx];
        let mut fs = font_sizes[orig_idx];
        let pad = (1.0 * scale).max(fs * 0.01);

        // Try placing at full size, then shrink up to 3 times if needed
        let mut layout_found = None;
        for shrink in 0..4 {
            let try_fs = fs * (1.0 - shrink as f32 * 0.15);
            let font_id = FontId::proportional(try_fs);
            let galley = ui
                .painter()
                .layout_no_wrap(entry.text.clone(), font_id, Color32::WHITE);
            let w = galley.rect.width();
            let h = galley.rect.height();

            if let Some((x, y)) = spiral_place(&ctx, w, h, &placed, pad) {
                fs = try_fs;
                layout_found = Some(WordLayout {
                    x,
                    y,
                    width: w,
                    height: h,
                    font_size: fs,
                });
                break;
            }
        }

        // Last resort: place at smallest tried size, skip if truly can't fit
        let layout = layout_found.unwrap_or_else(|| {
            let small_fs = fs * 0.55;
            let font_id = FontId::proportional(small_fs);
            let galley = ui
                .painter()
                .layout_no_wrap(entry.text.clone(), font_id, Color32::WHITE);
            let w = galley.rect.width();
            let h = galley.rect.height();
            if let Some((x, y)) = spiral_place(&ctx, w, h, &placed, 0.0) {
                WordLayout {
                    x,
                    y,
                    width: w,
                    height: h,
                    font_size: small_fs,
                }
            } else {
                // Absolute fallback: tiny and at center (shouldn't happen normally)
                WordLayout {
                    x: (ctx.cx - w / 2.0).clamp(0.0, (area_width - w).max(0.0)),
                    y: (ctx.cy - h / 2.0).clamp(0.0, (area_height - h).max(0.0)),
                    width: w,
                    height: h,
                    font_size: small_fs,
                }
            }
        });

        placed.push(layout.clone());
        result[orig_idx] = layout;
    }

    result
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_word_cloud(
    ui: &egui::Ui,
    content: &str,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    max_height: f32,
    opacity: f32,
    reveal_step: usize,
    scale: f32,
) -> f32 {
    let entries = parse_word_cloud(content);
    if entries.is_empty() {
        return 0.0;
    }

    let height = if max_height > 0.0 {
        max_height
    } else {
        500.0 * scale
    };

    // Get or compute layout
    let key = cache_key(content, max_width as u32, height as u32);
    let layouts = {
        let mut cache = LAYOUT_CACHE.lock().unwrap();
        if let Some(cached) = cache.get(&key) {
            cached.clone()
        } else {
            let layout = compute_layout(ui, &entries, max_width, height, scale);
            cache.insert(key, layout.clone());
            layout
        }
    };

    // Compute reveal steps
    let reveals: Vec<VizReveal> = entries.iter().map(|e| e.reveal).collect();
    let steps = assign_steps(&reveals);
    let palette = theme.edge_palette();

    let painter = ui.painter();

    for (i, entry) in entries.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        if let Some(wl) = layouts.get(i) {
            let color_idx = i % palette.len();
            let color = Theme::with_opacity(palette[color_idx], opacity);
            let font_id = FontId::proportional(wl.font_size);

            let galley = painter.layout_no_wrap(entry.text.clone(), font_id, color);
            let text_pos = Pos2::new(pos.x + wl.x, pos.y + wl.y);
            painter.galley(text_pos, galley, color);
        }
    }

    height
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_word_cloud_basic() {
        let content = "- Data Science (size: 40)\n- AI (size: 50)";
        let entries = parse_word_cloud(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "Data Science");
        assert_eq!(entries[0].size, 40.0);
        assert_eq!(entries[1].text, "AI");
        assert_eq!(entries[1].size, 50.0);
    }

    #[test]
    fn test_parse_word_cloud_no_size() {
        let content = "- Hello World";
        let entries = parse_word_cloud(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, "Hello World");
        assert_eq!(entries[0].size, 20.0);
    }

    #[test]
    fn test_parse_word_cloud_reveal_markers() {
        let content = "- Static\n+ Step1\n* WithPrev";
        let entries = parse_word_cloud(content);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].reveal, VizReveal::Static);
        assert_eq!(entries[1].reveal, VizReveal::NextStep);
        assert_eq!(entries[2].reveal, VizReveal::WithPrev);
    }

    #[test]
    fn test_parse_word_cloud_skips_comments() {
        let content = "# comment\n- Word (size: 30)";
        let entries = parse_word_cloud(content);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_parse_size_meta() {
        assert_eq!(parse_size_meta("(size: 40)"), Some(40.0));
        assert_eq!(parse_size_meta("(size: 12.5)"), Some(12.5));
        assert_eq!(parse_size_meta("(invalid)"), None);
    }

    #[test]
    fn test_compute_font_sizes_scales_proportionally() {
        let entries = vec![
            WordEntry {
                text: "Big".to_string(),
                size: 50.0,
                reveal: VizReveal::Static,
            },
            WordEntry {
                text: "Small".to_string(),
                size: 10.0,
                reveal: VizReveal::Static,
            },
        ];
        let sizes = compute_font_sizes(&entries, 1800.0, 900.0, 1.0);
        assert!(
            sizes[0] > sizes[1],
            "Bigger size value should produce bigger font"
        );
        assert!(
            sizes[0] > 100.0,
            "Largest font should be substantial: {}",
            sizes[0]
        );
        assert!(
            sizes[1] > 20.0,
            "Smallest font should be legible: {}",
            sizes[1]
        );
    }
}
