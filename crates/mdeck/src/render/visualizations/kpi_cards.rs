use std::time::Instant;

use eframe::egui::{self, Color32, FontId, Pos2};

use crate::theme::Theme;

use super::{VizReveal, assign_steps, parse_reveal_prefix, reveal_anim_progress};

// ─── Parsing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct KpiEntry {
    label: String,
    value: String,
    trend: Option<String>,
    reveal: VizReveal,
}

fn parse_kpi_cards(content: &str) -> Vec<KpiEntry> {
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

        // Parse "Label: Value (trend: +12%)" or "Label: Value"
        if let Some(colon_pos) = text.find(": ") {
            let label = text[..colon_pos].trim().to_string();
            let rest = text[colon_pos + 2..].trim();

            // Check for trend in parentheses
            let (value, trend) = if let Some(paren_start) = rest.find("(trend:") {
                let value = rest[..paren_start].trim().to_string();
                let trend_part = &rest[paren_start..];
                let trend_text = trend_part
                    .trim_start_matches("(trend:")
                    .trim_end_matches(')')
                    .trim()
                    .to_string();
                (value, Some(trend_text))
            } else {
                (rest.to_string(), None)
            };

            entries.push(KpiEntry {
                label,
                value,
                trend,
                reveal,
            });
        }
    }

    entries
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_kpi_cards(
    ui: &egui::Ui,
    content: &str,
    theme: &Theme,
    pos: Pos2,
    max_width: f32,
    max_height: f32,
    opacity: f32,
    reveal_step: usize,
    reveal_timestamp: Option<Instant>,
    scale: f32,
) -> f32 {
    let entries = parse_kpi_cards(content);
    if entries.is_empty() {
        return 0.0;
    }

    let height = if max_height > 0.0 {
        max_height
    } else {
        300.0 * scale
    };

    let reveals: Vec<VizReveal> = entries.iter().map(|e| e.reveal).collect();
    let steps = assign_steps(&reveals);
    let painter = ui.painter();

    let n = entries.len();
    let card_gap = 24.0 * scale;
    let total_gaps = (n as f32 - 1.0).max(0.0) * card_gap;
    let card_width = ((max_width - total_gaps) / n as f32).min(320.0 * scale);
    let card_height = height * 0.6;
    let total_width = n as f32 * card_width + total_gaps;
    let start_x = pos.x + (max_width - total_width) / 2.0;
    let card_y = pos.y + (height - card_height) / 2.0;

    let value_font = FontId::proportional(theme.body_size * 2.0 * scale);
    let label_font = FontId::proportional(theme.body_size * 0.7 * scale);
    let trend_font = FontId::proportional(theme.body_size * 0.6 * scale);

    let mut needs_repaint = false;

    for (i, entry) in entries.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        let card_x = start_x + i as f32 * (card_width + card_gap);
        let item_opacity = opacity * anim;

        // Card background
        let bg_color = Theme::with_opacity(theme.foreground, item_opacity * 0.05);
        let card_rect = egui::Rect::from_min_size(
            Pos2::new(card_x, card_y),
            egui::vec2(card_width, card_height),
        );
        painter.rect_filled(card_rect, 12.0 * scale, bg_color);

        // Value text (centered, large)
        let text_color = Theme::with_opacity(theme.foreground, item_opacity);
        let value_galley =
            painter.layout_no_wrap(entry.value.clone(), value_font.clone(), text_color);
        let value_x = card_x + (card_width - value_galley.rect.width()) / 2.0;
        let value_y = card_y + card_height * 0.2;
        painter.galley(
            Pos2::new(value_x, value_y),
            value_galley.clone(),
            text_color,
        );

        // Label text (centered, below value)
        let label_color = Theme::with_opacity(theme.foreground, item_opacity * 0.7);
        let label_galley =
            painter.layout_no_wrap(entry.label.clone(), label_font.clone(), label_color);
        let label_x = card_x + (card_width - label_galley.rect.width()) / 2.0;
        let label_y = value_y + value_galley.rect.height() + 8.0 * scale;
        painter.galley(
            Pos2::new(label_x, label_y),
            label_galley.clone(),
            label_color,
        );

        // Trend indicator with arrow (centered, below label)
        if let Some(ref trend) = entry.trend {
            let is_positive = trend.starts_with('+');
            let is_negative = trend.starts_with('-');
            let trend_color = if is_positive {
                Theme::with_opacity(Color32::from_rgb(34, 197, 94), item_opacity)
            } else if is_negative {
                Theme::with_opacity(Color32::from_rgb(239, 68, 68), item_opacity)
            } else {
                Theme::with_opacity(theme.foreground, item_opacity * 0.6)
            };

            // Draw trend text
            let trend_galley =
                painter.layout_no_wrap(trend.clone(), trend_font.clone(), trend_color);
            let trend_y = label_y + label_galley.rect.height() + 8.0 * scale;

            // Arrow size scales with the text height (so larger trends get larger arrows)
            let arrow_h = trend_galley.rect.height() * 0.7;
            let arrow_w = arrow_h * 0.8;
            let arrow_gap = 6.0 * scale;

            let total_w = if is_positive || is_negative {
                arrow_w + arrow_gap + trend_galley.rect.width()
            } else {
                trend_galley.rect.width()
            };
            let start_x = card_x + (card_width - total_w) / 2.0;

            if is_positive || is_negative {
                // Draw arrow as a filled triangle
                let arrow_cx = start_x + arrow_w / 2.0;
                let arrow_cy = trend_y + trend_galley.rect.height() / 2.0;
                let points = if is_positive {
                    // Up arrow
                    vec![
                        Pos2::new(arrow_cx, arrow_cy - arrow_h / 2.0),
                        Pos2::new(arrow_cx - arrow_w / 2.0, arrow_cy + arrow_h / 2.0),
                        Pos2::new(arrow_cx + arrow_w / 2.0, arrow_cy + arrow_h / 2.0),
                    ]
                } else {
                    // Down arrow
                    vec![
                        Pos2::new(arrow_cx - arrow_w / 2.0, arrow_cy - arrow_h / 2.0),
                        Pos2::new(arrow_cx + arrow_w / 2.0, arrow_cy - arrow_h / 2.0),
                        Pos2::new(arrow_cx, arrow_cy + arrow_h / 2.0),
                    ]
                };
                painter.add(egui::Shape::convex_polygon(
                    points,
                    trend_color,
                    egui::Stroke::NONE,
                ));

                // Text after arrow
                painter.galley(
                    Pos2::new(start_x + arrow_w + arrow_gap, trend_y),
                    trend_galley,
                    trend_color,
                );
            } else {
                painter.galley(Pos2::new(start_x, trend_y), trend_galley, trend_color);
            }
        }
    }

    if needs_repaint {
        ui.ctx().request_repaint();
    }

    height
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kpi_basic() {
        let content = "- Revenue: $4.2M";
        let entries = parse_kpi_cards(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Revenue");
        assert_eq!(entries[0].value, "$4.2M");
        assert!(entries[0].trend.is_none());
    }

    #[test]
    fn test_parse_kpi_with_trend() {
        let content = "- Revenue: $4.2M (trend: +12%)";
        let entries = parse_kpi_cards(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Revenue");
        assert_eq!(entries[0].value, "$4.2M");
        assert_eq!(entries[0].trend, Some("+12%".to_string()));
    }

    #[test]
    fn test_parse_kpi_multiple() {
        let content = "- Revenue: $4.2M (trend: +12%)\n- Users: 1.2M (trend: +8%)\n+ Churn: 3.2% (trend: -0.5%)";
        let entries = parse_kpi_cards(content);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].reveal, VizReveal::Static);
        assert_eq!(entries[2].reveal, VizReveal::NextStep);
        assert_eq!(entries[2].trend, Some("-0.5%".to_string()));
    }

    #[test]
    fn test_parse_kpi_skips_invalid() {
        let content = "- Valid: $100\n- no_colon\n# comment\n- Also: $200";
        let entries = parse_kpi_cards(content);
        assert_eq!(entries.len(), 2);
    }
}
