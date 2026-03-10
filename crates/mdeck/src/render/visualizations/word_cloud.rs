use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use eframe::egui::{self, Color32, FontId, Pos2};
use eframe::epaint::TextShape;

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
    /// Visual bounding box position and size (after rotation)
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    font_size: f32,
    /// Whether this word is rotated 90° counter-clockwise
    rotated: bool,
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

/// Deterministic "random" check: should this word be rotated?
/// Uses a hash of the word text + index so it's stable across frames.
/// Small, short words rotate more often — they slot into vertical gaps
/// between large horizontal words. Long words stay horizontal since
/// they'd create tall columns if rotated.
fn should_rotate(text: &str, index: usize, rank: usize, total: usize) -> bool {
    // Never rotate if too few words (need density for cloud shape)
    if total < 12 {
        return false;
    }
    // Only the smallest quarter of words can rotate — they're small enough
    // to slot into vertical gaps without creating columns at the edges.
    if rank < (total * 3) / 4 {
        return false;
    }

    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    index.hash(&mut hasher);
    let hash = hasher.finish();

    // Among the smallest quarter: short words rotate ~45%, long words ~15%
    let length_penalty = (text.len() as f32 / 12.0).min(1.0);
    let threshold = (45.0 - length_penalty * 30.0) as u64;
    hash % 100 < threshold
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

    // Scale max font based on word count: more words → smaller fonts.
    // Sized to create a dense cloud shape within an elliptical boundary.
    let n = entries.len() as f32;
    // Scale fonts to fill the elliptical cloud area densely.
    let count_factor = (8.0 / n.max(1.0)).sqrt().clamp(0.28, 0.80);
    // Max font sized so the biggest word is prominent but not overwhelming.
    // At 1920x1080 with ~75 words: roughly 8-10% of area height.
    let max_font = (area_height * 0.28 * count_factor).min(area_width * 0.14 * count_factor);
    let min_font = max_font * 0.14; // smallest word is ~14% of largest

    entries
        .iter()
        .map(|e| {
            if (max_size - min_size).abs() < 0.001 {
                max_font * scale
            } else {
                let t = (e.size - min_size) / (max_size - min_size);
                // Moderate convex curve: big words are clearly bigger, but not
                // so extreme that they dwarf everything else.
                let t_curved = t.powf(1.5);
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
    /// Ellipse semi-axes for cloud shape constraint
    ellipse_a: f32,
    ellipse_b: f32,
    scale: f32,
}

/// Check whether a rectangle fits inside the cloud ellipse.
/// We check that the rectangle's center is within a shrunk ellipse
/// (shrunk by half the rect dimensions) so the whole rect stays inside.
fn rect_inside_ellipse(x: f32, y: f32, w: f32, h: f32, ctx: &PlaceCtx) -> bool {
    let center_x = x + w / 2.0;
    let center_y = y + h / 2.0;
    // Shrink ellipse by half the word dimensions so edges stay inside
    let a = (ctx.ellipse_a - w / 2.0).max(1.0);
    let b = (ctx.ellipse_b - h / 2.0).max(1.0);
    let dx = center_x - ctx.cx;
    let dy = center_y - ctx.cy;
    (dx * dx) / (a * a) + (dy * dy) / (b * b) <= 1.0
}

/// Try to place a word using spiral search. Returns None if no valid position found.
/// `word_size` is the font size of the word being placed, used to adapt spiral granularity.
fn spiral_place(
    ctx: &PlaceCtx,
    w: f32,
    h: f32,
    placed: &[WordLayout],
    pad: f32,
    word_size: f32,
) -> Option<(f32, f32)> {
    let mut t = 0.0f32;
    // Smaller words need finer spiral steps to find gaps between larger words
    let size_ratio = (word_size / (ctx.area_height * 0.15)).min(1.0);
    let t_step = 0.015 + size_ratio * 0.035; // finer steps for denser packing
    // Slow growth rate keeps words close to center (cloud-like)
    let base_growth = (ctx.area_width + ctx.area_height) * 0.00025;
    let growth = (base_growth * (0.3 + size_ratio * 0.7)) * ctx.scale;
    // Horizontal stretch to match ellipse shape
    let aspect = (ctx.ellipse_a / ctx.ellipse_b).max(1.0);

    let max_iters = if size_ratio < 0.3 { 40000 } else { 25000 };

    for _ in 0..max_iters {
        let angle = t * 2.5;
        let r = t * growth;
        let x = ctx.cx + r * angle.cos() * aspect - w / 2.0;
        let y = ctx.cy + r * angle.sin() - h / 2.0;

        // Check elliptical boundary (cloud shape) instead of rectangular
        if rect_inside_ellipse(x, y, w, h, ctx) {
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

/// Try to place a word, testing both its preferred rotation and the alternative.
/// Returns the placed WordLayout or None if it truly can't fit.
fn try_place_word(
    ui: &egui::Ui,
    ctx: &PlaceCtx,
    entry: &WordEntry,
    base_fs: f32,
    prefer_rotated: bool,
    placed: &[WordLayout],
) -> Option<WordLayout> {
    // Try preferred rotation first, then the alternative
    for &try_rotated in &[prefer_rotated, !prefer_rotated] {
        // Try progressively smaller sizes
        for shrink in 0..6 {
            let try_fs = base_fs * (1.0 - shrink as f32 * 0.10);
            if try_fs < 2.0 * ctx.scale {
                break; // don't go below minimum legible size
            }
            let font_id = FontId::proportional(try_fs);
            let galley = ui
                .painter()
                .layout_no_wrap(entry.text.clone(), font_id, Color32::WHITE);
            let orig_w = galley.rect.width();
            let orig_h = galley.rect.height();

            let (vis_w, vis_h) = if try_rotated {
                (orig_h, orig_w)
            } else {
                (orig_w, orig_h)
            };

            // Padding between words creates gaps that small vertical words
            // can slot into. Scale with font size so big words get bigger gaps.
            let pad = (try_fs * 0.15).max(2.0 * ctx.scale);
            if let Some((x, y)) = spiral_place(ctx, vis_w, vis_h, placed, pad, try_fs) {
                return Some(WordLayout {
                    x,
                    y,
                    width: vis_w,
                    height: vis_h,
                    font_size: try_fs,
                    rotated: try_rotated,
                });
            }
        }
    }
    None
}

/// Dense spiral placement: place largest words first at center, pack tightly.
/// Words that can't fit are dropped (not overlaid on top of others).
/// Some words are rotated 90° CCW for a classic word cloud look.
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
    // Initialize with zero-size layouts (unplaced words won't be drawn)
    let mut result = vec![
        WordLayout {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            font_size: 0.0,
            rotated: false,
        };
        entries.len()
    ];

    // Ellipse creates a cloud shape floating in the center with margins.
    // Words that don't fit get dropped, giving clean cloud edges.
    let ellipse_a = area_width * 0.44; // horizontal semi-axis (~88% of width)
    let ellipse_b = area_height * 0.44; // vertical semi-axis (~88% of height)

    let ctx = PlaceCtx {
        cx: area_width / 2.0,
        cy: area_height / 2.0,
        area_width,
        area_height,
        ellipse_a,
        ellipse_b,
        scale,
    };

    let total = entries.len();
    for (rank, &orig_idx) in sorted_indices.iter().enumerate() {
        let entry = &entries[orig_idx];
        let fs = font_sizes[orig_idx];
        let prefer_rotated = should_rotate(&entry.text, orig_idx, rank, total);

        if let Some(layout) = try_place_word(ui, &ctx, entry, fs, prefer_rotated, &placed) {
            placed.push(layout.clone());
            result[orig_idx] = layout;
        }
        // Words that can't fit are simply not placed (font_size stays 0)
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
            // Skip words that couldn't be placed (font_size 0)
            if wl.font_size < 1.0 {
                continue;
            }
            let color_idx = i % palette.len();
            let color = Theme::with_opacity(palette[color_idx], opacity);
            let font_id = FontId::proportional(wl.font_size);

            let galley = painter.layout_no_wrap(entry.text.clone(), font_id, color);

            if wl.rotated {
                // Rotate -90° (CCW). Pivot is at pos (top-left of unrotated text).
                // For visual bbox at (vx, vy) with visual size (orig_h, orig_w):
                //   anchor pos.x = vx + visual_width (= vx + orig_h)
                //   anchor pos.y = vy
                let anchor_pos = Pos2::new(pos.x + wl.x + wl.width, pos.y + wl.y);
                let text_shape = TextShape::new(anchor_pos, galley, color)
                    .with_angle(-std::f32::consts::FRAC_PI_2)
                    .with_opacity_factor(opacity);
                painter.add(text_shape);
            } else {
                let text_pos = Pos2::new(pos.x + wl.x, pos.y + wl.y);
                painter.galley(text_pos, galley, color);
            }
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
    fn test_should_rotate_never_for_top_words() {
        // Top 75% of words by rank should never rotate
        for rank in 0..37 {
            assert!(!should_rotate("Big", rank, rank, 50));
        }
    }

    #[test]
    fn test_should_rotate_never_for_few_words() {
        // Fewer than 12 words → no rotation
        for i in 0..11 {
            assert!(!should_rotate("Word", i, i, 11));
        }
    }

    #[test]
    fn test_should_rotate_deterministic() {
        // Same input should always give the same result
        let r1 = should_rotate("Test", 5, 5, 20);
        let r2 = should_rotate("Test", 5, 5, 20);
        assert_eq!(r1, r2);
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
