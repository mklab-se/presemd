use std::time::Instant;

use eframe::egui::{self, FontId, Pos2, Stroke};

use crate::theme::Theme;

use super::{VizReveal, assign_steps, parse_reveal_prefix, reveal_anim_progress};

// ─── Date Arithmetic ────────────────────────────────────────────────────────

/// Simple date representation (year, month 1-based, day 1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Date {
    year: i32,
    month: u32,
    day: u32,
}

impl Date {
    fn new(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }

    fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return None;
        }
        let year = parts[0].parse().ok()?;
        let month = parts[1].parse().ok()?;
        let day = parts[2].parse().ok()?;
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        Some(Self { year, month, day })
    }

    #[cfg(test)]
    fn format(&self) -> String {
        format!("{}-{:02}-{:02}", self.year, self.month, self.day)
    }

    fn format_short(self) -> String {
        static MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let m = self.month.saturating_sub(1).min(11) as usize;
        format!("{} {}", MONTHS[m], self.day)
    }

    fn format_month_year(self) -> String {
        static MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let m = self.month.saturating_sub(1).min(11) as usize;
        format!("{} {}", MONTHS[m], self.year)
    }

    /// Convert to a day number (days since an epoch). Used for arithmetic.
    fn to_days(self) -> i64 {
        // Algorithm from https://en.wikipedia.org/wiki/Julian_day
        let y = self.year as i64;
        let m = self.month as i64;
        let d = self.day as i64;
        let a = (14 - m) / 12;
        let yy = y + 4800 - a;
        let mm = m + 12 * a - 3;
        d + (153 * mm + 2) / 5 + 365 * yy + yy / 4 - yy / 100 + yy / 400 - 32045
    }

    fn from_days(jdn: i64) -> Self {
        // Inverse of to_days
        let a = jdn + 32044;
        let b = (4 * a + 3) / 146097;
        let c = a - (146097 * b) / 4;
        let d = (4 * c + 3) / 1461;
        let e = c - (1461 * d) / 4;
        let m = (5 * e + 2) / 153;
        let day = (e - (153 * m + 2) / 5 + 1) as u32;
        let month = (m + 3 - 12 * (m / 10)) as u32;
        let year = (100 * b + d - 4800 + m / 10) as i32;
        Self { year, month, day }
    }

    fn add_days(self, n: i64) -> Self {
        Self::from_days(self.to_days() + n)
    }

    fn add_workdays(self, n: i64) -> Self {
        let mut current = self.to_days();
        let mut remaining = n;
        let dir: i64 = if n >= 0 { 1 } else { -1 };
        let mut abs_remaining = remaining.unsigned_abs();
        while abs_remaining > 0 {
            current += dir;
            let d = Self::from_days(current);
            if d.weekday() < 5 {
                // Mon-Fri
                abs_remaining -= 1;
            }
        }
        remaining = 0; // consumed
        let _ = remaining;
        Self::from_days(current)
    }

    /// 0=Mon, 1=Tue, ..., 6=Sun
    fn weekday(self) -> u32 {
        let jdn = self.to_days();
        ((jdn % 7) as u32 + 7) % 7 // Adjusted so Monday = 0
    }

    fn days_between(self, other: &Date) -> i64 {
        other.to_days() - self.to_days()
    }
}

// ─── Duration Parsing ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum Duration {
    Days(i64),
    WorkDays(i64),
}

fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if let Some(n) = s.strip_suffix("wd") {
        return n.trim().parse().ok().map(Duration::WorkDays);
    }
    if let Some(n) = s.strip_suffix('d') {
        return n.trim().parse().ok().map(Duration::Days);
    }
    if let Some(n) = s.strip_suffix('w') {
        return n.trim().parse::<i64>().ok().map(|w| Duration::Days(w * 7));
    }
    if let Some(n) = s.strip_suffix('m') {
        return n.trim().parse::<i64>().ok().map(|m| Duration::Days(m * 30));
    }
    None
}

fn apply_duration(start: &Date, dur: Duration) -> Date {
    match dur {
        Duration::Days(n) => start.add_days(n),
        Duration::WorkDays(n) => start.add_workdays(n),
    }
}

// ─── Parsing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct GanttTask {
    name: String,
    start: Option<Date>,
    end: Option<Date>,
    duration: Option<Duration>,
    /// (task_name, delay_duration)
    dependencies: Vec<(String, Option<Duration>)>,
    reveal: VizReveal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LabelMode {
    Side,
    Inside,
}

#[derive(Debug, Clone)]
struct GanttData {
    tasks: Vec<GanttTask>,
    title: Option<String>,
    labels: LabelMode,
}

fn parse_gantt(content: &str) -> GanttData {
    let mut tasks = Vec::new();
    let mut title = None;
    let mut labels = LabelMode::Side;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse directives
        if trimmed.starts_with('#') {
            if let Some(rest) = trimmed
                .strip_prefix("# title:")
                .or_else(|| trimmed.strip_prefix("#title:"))
            {
                title = Some(rest.trim().to_string());
            } else if let Some(rest) = trimmed
                .strip_prefix("# labels:")
                .or_else(|| trimmed.strip_prefix("#labels:"))
            {
                if rest.trim().eq_ignore_ascii_case("inside") {
                    labels = LabelMode::Inside;
                }
            }
            continue;
        }

        let (text, reveal) = parse_reveal_prefix(trimmed);
        if text.is_empty() {
            continue;
        }

        // Parse "Task Name: spec1, spec2, ..."
        if let Some(colon_pos) = text.find(": ") {
            let name = text[..colon_pos].trim().to_string();
            let specs_str = &text[colon_pos + 2..];

            let mut start = None;
            let mut end = None;
            let mut duration = None;
            let mut dependencies = Vec::new();

            for spec in split_specs(specs_str) {
                let spec = spec.trim();
                if spec.is_empty() {
                    continue;
                }

                // "after TaskName" or "after TaskName + 3d"
                if let Some(rest) = spec.strip_prefix("after ") {
                    let (dep_name, delay) = parse_dependency(rest);
                    dependencies.push((dep_name, delay));
                }
                // Try as date
                else if let Some(d) = Date::parse(spec) {
                    if start.is_none() {
                        start = Some(d);
                    } else {
                        end = Some(d);
                    }
                }
                // Try as duration
                else if let Some(d) = parse_duration(spec) {
                    duration = Some(d);
                }
            }

            tasks.push(GanttTask {
                name,
                start,
                end,
                duration,
                dependencies,
                reveal,
            });
        }
    }

    GanttData {
        tasks,
        title,
        labels,
    }
}

/// Split specs by comma, but respect "after Task + 3d" as a single spec.
fn split_specs(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut after_mode = false;

    for part in s.split(", ") {
        if part.starts_with("after ") {
            if !current.is_empty() {
                result.push(current.clone());
                current.clear();
            }
            after_mode = true;
            current = part.to_string();
        } else if after_mode && part.starts_with('+') {
            // This is the delay part of "after Task + 3d"
            current.push_str(", ");
            current.push_str(part);
            after_mode = false;
        } else {
            after_mode = false;
            if !current.is_empty() {
                result.push(current.clone());
                current.clear();
            }
            current = part.to_string();
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn parse_dependency(rest: &str) -> (String, Option<Duration>) {
    // "TaskName + 3d" or just "TaskName"
    if let Some(plus_pos) = rest.find(" + ") {
        let dep_name = rest[..plus_pos].trim().to_string();
        let delay_str = rest[plus_pos + 3..].trim();
        let delay = parse_duration(delay_str);
        (dep_name, delay)
    } else {
        (rest.trim().to_string(), None)
    }
}

// ─── Resolution ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ResolvedTask {
    name: String,
    start: Date,
    end: Date,
    reveal: VizReveal,
}

fn resolve_tasks(data: &GanttData) -> Vec<ResolvedTask> {
    let mut resolved: Vec<ResolvedTask> = Vec::new();

    for task in &data.tasks {
        let dep_end = task
            .dependencies
            .iter()
            .filter_map(|(dep_name, delay)| {
                resolved.iter().find(|r| r.name == *dep_name).map(|r| {
                    if let Some(d) = delay {
                        apply_duration(&r.end, *d)
                    } else {
                        r.end
                    }
                })
            })
            .max();

        let (start, end) = match (task.start, task.end, task.duration, dep_end) {
            // Start + End explicit
            (Some(s), Some(e), _, _) => (s, e),
            // Start + Duration
            (Some(s), None, Some(dur), _) => (s, apply_duration(&s, dur)),
            // Duration + End
            (None, Some(e), Some(dur), _) => {
                let dur_days = match dur {
                    Duration::Days(n) => n,
                    Duration::WorkDays(n) => n, // approximate
                };
                (e.add_days(-dur_days), e)
            }
            // Dependency + Duration
            (None, None, Some(dur), Some(dep_e)) => (dep_e, apply_duration(&dep_e, dur)),
            // Dependency only (1 day default)
            (None, None, None, Some(dep_e)) => (dep_e, dep_e.add_days(1)),
            // Start only (default 1 day)
            (Some(s), None, None, _) => (s, s.add_days(1)),
            // No info at all — skip
            // Fallback: use dependency end as start, or skip
            _ => continue,
        };

        resolved.push(ResolvedTask {
            name: task.name.clone(),
            start,
            end,
            reveal: task.reveal,
        });
    }

    resolved
}

// ─── Timeline Scale ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum TimeScale {
    Days,
    Weeks,
    Months,
}

struct TimeGrid {
    scale: TimeScale,
    labels: Vec<(f32, String)>, // (fraction 0..1, label)
}

fn compute_time_grid(min_date: &Date, max_date: &Date, total_days: i64) -> TimeGrid {
    let scale = if total_days <= 21 {
        TimeScale::Days
    } else if total_days <= 120 {
        TimeScale::Weeks
    } else {
        TimeScale::Months
    };

    let mut labels = Vec::new();

    match scale {
        TimeScale::Days => {
            let mut d = *min_date;
            while d <= *max_date {
                let frac = min_date.days_between(&d) as f32 / total_days as f32;
                labels.push((frac, d.format_short()));
                d = d.add_days(1);
            }
        }
        TimeScale::Weeks => {
            // Start from first Monday on or after min_date
            let mut d = *min_date;
            let wd = d.weekday();
            if wd > 0 {
                d = d.add_days((7 - wd as i64) % 7);
            }
            while d <= *max_date {
                let frac = min_date.days_between(&d) as f32 / total_days as f32;
                labels.push((frac, d.format_short()));
                d = d.add_days(7);
            }
        }
        TimeScale::Months => {
            // First day of each month
            let mut y = min_date.year;
            let mut m = min_date.month;
            loop {
                let d = Date::new(y, m, 1);
                if d > *max_date {
                    break;
                }
                if d >= *min_date {
                    let frac = min_date.days_between(&d) as f32 / total_days as f32;
                    labels.push((frac, d.format_month_year()));
                }
                m += 1;
                if m > 12 {
                    m = 1;
                    y += 1;
                }
            }
        }
    }

    TimeGrid { scale, labels }
}

// ─── Renderer ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_gantt_chart(
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
    let data = parse_gantt(content);
    let resolved = resolve_tasks(&data);
    if resolved.is_empty() {
        return 0.0;
    }

    let height = if max_height > 0.0 {
        max_height
    } else {
        500.0 * scale
    };

    let reveals: Vec<VizReveal> = resolved.iter().map(|t| t.reveal).collect();
    let steps = assign_steps(&reveals);
    let palette = theme.edge_palette();
    let painter = ui.painter();

    // Date range
    let min_date = resolved.iter().map(|t| t.start).min().unwrap();
    let max_date = resolved.iter().map(|t| t.end).max().unwrap();
    let total_days = min_date.days_between(&max_date).max(1);

    // Add padding to date range (1 unit on each side)
    let pad_days = (total_days as f32 * 0.03).max(1.0) as i64;
    let display_min = min_date.add_days(-pad_days);
    let display_max = max_date.add_days(pad_days);
    let display_total = display_min.days_between(&display_max).max(1);

    let time_grid = compute_time_grid(&display_min, &display_max, display_total);

    // Layout dimensions
    let padding = 40.0 * scale;
    let label_area_width = if data.labels == LabelMode::Inside {
        padding * 0.5 // Minimal left margin when labels are inside bars
    } else {
        max_width * 0.22 // Left area for task names
    };
    let timeline_label_height = 35.0 * scale; // Bottom area for date labels
    let header_height = if data.title.is_some() {
        40.0 * scale
    } else {
        10.0 * scale
    };

    let chart_left = pos.x + label_area_width;
    let chart_width = max_width - label_area_width - padding;
    let chart_top = pos.y + header_height;
    let chart_height = height - header_height - timeline_label_height - padding * 0.5;
    let chart_bottom = chart_top + chart_height;

    // Visible tasks (respecting reveal)
    let visible_count = resolved
        .iter()
        .enumerate()
        .filter(|(i, _)| steps.get(*i).copied().unwrap_or(0) <= reveal_step)
        .count();
    if visible_count == 0 {
        return height;
    }

    let mut needs_repaint = false;

    // Title
    if let Some(ref title_text) = data.title {
        let title_font = FontId::proportional(theme.body_size * 0.75 * scale);
        let title_color = Theme::with_opacity(theme.foreground, opacity * 0.9);
        let galley = painter.layout_no_wrap(title_text.clone(), title_font, title_color);
        let tx = pos.x + (max_width - galley.rect.width()) / 2.0;
        painter.galley(Pos2::new(tx, pos.y + 4.0 * scale), galley, title_color);
    }

    // Grid lines and date labels
    let grid_color = Theme::with_opacity(theme.foreground, opacity * 0.06);
    let label_font = FontId::proportional(theme.body_size * 0.50 * scale);
    let label_color = Theme::with_opacity(theme.foreground, opacity * 0.45);

    for (frac, label_text) in &time_grid.labels {
        let x = chart_left + frac * chart_width;

        // Vertical grid line
        painter.line_segment(
            [Pos2::new(x, chart_top), Pos2::new(x, chart_bottom)],
            Stroke::new(0.5 * scale, grid_color),
        );

        // Date label
        let galley = painter.layout_no_wrap(label_text.clone(), label_font.clone(), label_color);
        let lx = x - galley.rect.width() / 2.0;
        painter.galley(
            Pos2::new(lx, chart_bottom + 6.0 * scale),
            galley,
            label_color,
        );
    }

    // Weekend shading (only in day-level scale)
    if matches!(time_grid.scale, TimeScale::Days) {
        let weekend_color = Theme::with_opacity(theme.foreground, opacity * 0.04);
        let mut d = display_min;
        while d <= display_max {
            let wd = d.weekday();
            if wd >= 5 {
                // Saturday or Sunday
                let frac = display_min.days_between(&d) as f32 / display_total as f32;
                let next_frac =
                    display_min.days_between(&d.add_days(1)) as f32 / display_total as f32;
                let x0 = chart_left + frac * chart_width;
                let x1 = chart_left + next_frac * chart_width;
                let weekend_rect =
                    egui::Rect::from_min_max(Pos2::new(x0, chart_top), Pos2::new(x1, chart_bottom));
                painter.rect_filled(weekend_rect, 0.0, weekend_color);
            }
            d = d.add_days(1);
        }
    }

    // Axis line at bottom
    let axis_color = Theme::with_opacity(theme.foreground, opacity * 0.15);
    painter.line_segment(
        [
            Pos2::new(chart_left, chart_bottom),
            Pos2::new(chart_left + chart_width, chart_bottom),
        ],
        Stroke::new(1.0 * scale, axis_color),
    );

    // Task bars
    let total_tasks = resolved.len();
    let row_height = (chart_height / total_tasks as f32).min(50.0 * scale);
    let bar_height = (row_height * 0.55).min(32.0 * scale).max(12.0 * scale);
    let bar_corner = 3.0 * scale;

    let task_name_font = FontId::proportional(theme.body_size * 0.55 * scale);
    let bar_label_font = FontId::proportional(theme.body_size * 0.50 * scale);

    // Center tasks vertically if they don't fill the chart
    let total_task_height = total_tasks as f32 * row_height;
    let y_offset = if total_task_height < chart_height {
        (chart_height - total_task_height) / 2.0
    } else {
        0.0
    };

    for (i, task) in resolved.iter().enumerate() {
        let step = steps.get(i).copied().unwrap_or(0);
        if step > reveal_step {
            continue;
        }

        let (anim, repaint) = reveal_anim_progress(step, reveal_step, reveal_timestamp);
        if repaint {
            needs_repaint = true;
        }

        let row_y = chart_top + y_offset + i as f32 * row_height;
        let bar_y = row_y + (row_height - bar_height) / 2.0;

        // Alternating row background
        if i % 2 == 0 {
            let row_bg = Theme::with_opacity(theme.foreground, opacity * 0.02);
            let row_rect = egui::Rect::from_min_size(
                Pos2::new(pos.x, row_y),
                egui::vec2(max_width, row_height),
            );
            painter.rect_filled(row_rect, 0.0, row_bg);
        }

        // Task name on the left (side mode only)
        if data.labels == LabelMode::Side {
            let name_color = Theme::with_opacity(theme.foreground, opacity * 0.8 * anim);
            let galley = painter.layout(
                task.name.clone(),
                task_name_font.clone(),
                name_color,
                label_area_width - 16.0 * scale,
            );
            let name_y = row_y + (row_height - galley.rect.height()) / 2.0;
            painter.galley(Pos2::new(pos.x + 8.0 * scale, name_y), galley, name_color);
        }

        // Bar position
        let start_frac = display_min.days_between(&task.start) as f32 / display_total as f32;
        let end_frac = display_min.days_between(&task.end) as f32 / display_total as f32;

        let bar_x = chart_left + start_frac * chart_width;
        let bar_w = ((end_frac - start_frac) * chart_width * anim).max(3.0 * scale);

        let color_idx = i % palette.len();
        let bar_color = Theme::with_opacity(palette[color_idx], opacity * 0.75 * anim);

        let bar_rect =
            egui::Rect::from_min_size(Pos2::new(bar_x, bar_y), egui::vec2(bar_w, bar_height));
        painter.rect_filled(bar_rect, bar_corner, bar_color);

        // Subtle border for definition
        let border_color = Theme::with_opacity(palette[color_idx], opacity * 0.3 * anim);
        painter.rect_stroke(
            bar_rect,
            bar_corner,
            Stroke::new(0.5 * scale, border_color),
            egui::StrokeKind::Outside,
        );

        // Bar label: task name inside (inside mode) or duration (side mode)
        if anim > 0.7 {
            let label_opacity = ((anim - 0.7) / 0.3).min(1.0);

            if data.labels == LabelMode::Inside {
                // Task name inside bar, with duration suffix
                let days = task.start.days_between(&task.end);
                let dur_suffix = format_duration_label(days, &time_grid.scale);
                let inside_text = format!("{}  {}", task.name, dur_suffix);
                let text_color =
                    Theme::with_opacity(theme.foreground, opacity * 0.9 * label_opacity);
                let name_galley =
                    painter.layout_no_wrap(inside_text, bar_label_font.clone(), text_color);

                if name_galley.rect.width() + 12.0 * scale < bar_w {
                    // Fits inside — left-aligned with padding
                    let dx = bar_x + 6.0 * scale;
                    let dy = bar_y + (bar_height - name_galley.rect.height()) / 2.0;
                    painter.galley(Pos2::new(dx, dy), name_galley, text_color);
                } else {
                    // Try name only (no duration)
                    let name_only_galley = painter.layout_no_wrap(
                        task.name.clone(),
                        bar_label_font.clone(),
                        text_color,
                    );
                    if name_only_galley.rect.width() + 12.0 * scale < bar_w {
                        let dx = bar_x + 6.0 * scale;
                        let dy = bar_y + (bar_height - name_only_galley.rect.height()) / 2.0;
                        painter.galley(Pos2::new(dx, dy), name_only_galley, text_color);
                    } else {
                        // Doesn't fit — place to the right of bar
                        let dx = bar_x + bar_w + 6.0 * scale;
                        let dy = bar_y + (bar_height - name_only_galley.rect.height()) / 2.0;
                        if dx + name_only_galley.rect.width() < chart_left + chart_width {
                            painter.galley(Pos2::new(dx, dy), name_only_galley, text_color);
                        }
                    }
                }
            } else {
                // Side mode: show duration inside/beside bar
                let days = task.start.days_between(&task.end);
                let dur_text = format_duration_label(days, &time_grid.scale);
                let dur_color =
                    Theme::with_opacity(theme.foreground, opacity * 0.7 * label_opacity);
                let dur_galley =
                    painter.layout_no_wrap(dur_text, bar_label_font.clone(), dur_color);

                if dur_galley.rect.width() + 8.0 * scale < bar_w {
                    // Inside bar, centered
                    let dx = bar_x + (bar_w - dur_galley.rect.width()) / 2.0;
                    let dy = bar_y + (bar_height - dur_galley.rect.height()) / 2.0;
                    painter.galley(Pos2::new(dx, dy), dur_galley, dur_color);
                } else if bar_x + bar_w + dur_galley.rect.width() + 10.0 * scale
                    < chart_left + chart_width
                {
                    // Right of bar
                    let dx = bar_x + bar_w + 6.0 * scale;
                    let dy = bar_y + (bar_height - dur_galley.rect.height()) / 2.0;
                    painter.galley(Pos2::new(dx, dy), dur_galley, dur_color);
                }
            }
        }

        // Dependency arrows
        for (dep_name, _) in data.tasks.get(i).map_or(&[][..], |t| &t.dependencies) {
            if let Some((di, dep_task)) = resolved
                .iter()
                .enumerate()
                .find(|(_, r)| r.name == *dep_name)
            {
                let dep_step = steps.get(di).copied().unwrap_or(0);
                if dep_step > reveal_step {
                    continue;
                }

                let dep_end_frac =
                    display_min.days_between(&dep_task.end) as f32 / display_total as f32;
                let dep_row_y = chart_top + y_offset + di as f32 * row_height;
                let dep_bar_center_y = dep_row_y + row_height / 2.0;

                let arrow_start_x = chart_left + dep_end_frac * chart_width;
                let arrow_end_x = bar_x;
                let arrow_end_y = bar_y + bar_height / 2.0;

                let arrow_color = Theme::with_opacity(theme.foreground, opacity * 0.25 * anim);
                let arrow_stroke = Stroke::new(1.5 * scale, arrow_color);

                // Draw L-shaped connector
                let mid_x = (arrow_start_x + arrow_end_x) / 2.0;
                painter.line_segment(
                    [
                        Pos2::new(arrow_start_x, dep_bar_center_y),
                        Pos2::new(mid_x, dep_bar_center_y),
                    ],
                    arrow_stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(mid_x, dep_bar_center_y),
                        Pos2::new(mid_x, arrow_end_y),
                    ],
                    arrow_stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(mid_x, arrow_end_y),
                        Pos2::new(arrow_end_x, arrow_end_y),
                    ],
                    arrow_stroke,
                );

                // Arrowhead
                let arrow_size = 4.0 * scale;
                painter.line_segment(
                    [
                        Pos2::new(arrow_end_x - arrow_size, arrow_end_y - arrow_size),
                        Pos2::new(arrow_end_x, arrow_end_y),
                    ],
                    arrow_stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(arrow_end_x - arrow_size, arrow_end_y + arrow_size),
                        Pos2::new(arrow_end_x, arrow_end_y),
                    ],
                    arrow_stroke,
                );
            }
        }
    }

    // Horizontal separator line between label area and chart
    let sep_color = Theme::with_opacity(theme.foreground, opacity * 0.08);
    painter.line_segment(
        [
            Pos2::new(chart_left - 4.0 * scale, chart_top),
            Pos2::new(chart_left - 4.0 * scale, chart_bottom),
        ],
        Stroke::new(0.5 * scale, sep_color),
    );

    if needs_repaint {
        ui.ctx().request_repaint();
    }

    height
}

fn format_duration_label(days: i64, scale: &TimeScale) -> String {
    match scale {
        TimeScale::Days => {
            if days == 1 {
                "1 day".to_string()
            } else {
                format!("{days} days")
            }
        }
        TimeScale::Weeks => {
            if days < 7 {
                format!("{days}d")
            } else if days % 7 == 0 {
                let w = days / 7;
                if w == 1 {
                    "1 wk".to_string()
                } else {
                    format!("{w} wks")
                }
            } else {
                format!("{days}d")
            }
        }
        TimeScale::Months => {
            if days < 30 {
                format!("{days}d")
            } else {
                let months = days / 30;
                if months == 1 {
                    "~1 mo".to_string()
                } else {
                    format!("~{months} mo")
                }
            }
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_parse() {
        let d = Date::parse("2024-01-15").unwrap();
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 15);
    }

    #[test]
    fn test_date_invalid() {
        assert!(Date::parse("not-a-date").is_none());
        assert!(Date::parse("2024-13-01").is_none());
        assert!(Date::parse("2024-01-32").is_none());
    }

    #[test]
    fn test_date_arithmetic() {
        let d = Date::new(2024, 1, 15);
        let d2 = d.add_days(10);
        assert_eq!(d2.format(), "2024-01-25");

        let d3 = d.add_days(20);
        assert_eq!(d3.format(), "2024-02-04");
    }

    #[test]
    fn test_date_days_between() {
        let d1 = Date::new(2024, 1, 1);
        let d2 = Date::new(2024, 1, 31);
        assert_eq!(d1.days_between(&d2), 30);
    }

    #[test]
    fn test_date_roundtrip() {
        let d = Date::new(2024, 6, 15);
        let days = d.to_days();
        let d2 = Date::from_days(days);
        assert_eq!(d, d2);
    }

    #[test]
    fn test_date_weekday() {
        // 2024-01-15 is a Monday
        let d = Date::new(2024, 1, 15);
        assert_eq!(d.weekday(), 0); // Monday
    }

    #[test]
    fn test_date_workdays() {
        // From Monday, add 5 working days = next Monday
        let d = Date::new(2024, 1, 15); // Monday
        let d2 = d.add_workdays(5);
        assert_eq!(d2.format(), "2024-01-22"); // Next Monday
    }

    #[test]
    fn test_parse_duration() {
        assert!(matches!(parse_duration("10d"), Some(Duration::Days(10))));
        assert!(matches!(parse_duration("5wd"), Some(Duration::WorkDays(5))));
        assert!(matches!(parse_duration("2w"), Some(Duration::Days(14))));
        assert!(matches!(parse_duration("3m"), Some(Duration::Days(90))));
        assert!(parse_duration("foo").is_none());
    }

    #[test]
    fn test_parse_gantt_basic() {
        let content = "- Research: 2024-01-15, 10d\n- Design: 5d, after Research\n- Build: 2024-02-01, 2024-03-01";
        let data = parse_gantt(content);
        assert_eq!(data.tasks.len(), 3);
        assert_eq!(data.tasks[0].name, "Research");
        assert!(data.tasks[0].start.is_some());
        assert!(data.tasks[0].duration.is_some());
        assert_eq!(data.tasks[1].name, "Design");
        assert_eq!(data.tasks[1].dependencies.len(), 1);
        assert_eq!(data.tasks[1].dependencies[0].0, "Research");
        assert_eq!(data.tasks[2].name, "Build");
        assert!(data.tasks[2].start.is_some());
        assert!(data.tasks[2].end.is_some());
    }

    #[test]
    fn test_parse_gantt_with_delay() {
        let content = "- A: 2024-01-01, 5d\n- B: 3d, after A + 2d";
        let data = parse_gantt(content);
        assert_eq!(data.tasks[1].dependencies[0].0, "A");
        assert!(data.tasks[1].dependencies[0].1.is_some());
    }

    #[test]
    fn test_parse_gantt_reveal_markers() {
        let content = "- A: 2024-01-01, 5d\n+ B: 3d, after A\n* C: 2d, after A";
        let data = parse_gantt(content);
        assert_eq!(data.tasks[0].reveal, VizReveal::Static);
        assert_eq!(data.tasks[1].reveal, VizReveal::NextStep);
        assert_eq!(data.tasks[2].reveal, VizReveal::WithPrev);
    }

    #[test]
    fn test_resolve_tasks_basic() {
        let content = "- Research: 2024-01-15, 10d\n- Design: 5d, after Research";
        let data = parse_gantt(content);
        let resolved = resolve_tasks(&data);
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].start.format(), "2024-01-15");
        assert_eq!(resolved[0].end.format(), "2024-01-25");
        assert_eq!(resolved[1].start.format(), "2024-01-25");
        assert_eq!(resolved[1].end.format(), "2024-01-30");
    }

    #[test]
    fn test_resolve_tasks_with_delay() {
        let content = "- A: 2024-01-01, 5d\n- B: 3d, after A + 2d";
        let data = parse_gantt(content);
        let resolved = resolve_tasks(&data);
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].end.format(), "2024-01-06");
        // B starts 2 days after A ends
        assert_eq!(resolved[1].start.format(), "2024-01-08");
        assert_eq!(resolved[1].end.format(), "2024-01-11");
    }

    #[test]
    fn test_resolve_tasks_parallel() {
        let content = "- Planning: 2024-01-01, 5d\n- Frontend: 10d, after Planning\n- Backend: 10d, after Planning";
        let data = parse_gantt(content);
        let resolved = resolve_tasks(&data);
        assert_eq!(resolved.len(), 3);
        // Frontend and Backend should start on same date
        assert_eq!(resolved[1].start, resolved[2].start);
    }

    #[test]
    fn test_format_short() {
        let d = Date::new(2024, 3, 15);
        assert_eq!(d.format_short(), "Mar 15");
    }

    #[test]
    fn test_format_month_year() {
        let d = Date::new(2024, 3, 1);
        assert_eq!(d.format_month_year(), "Mar 2024");
    }

    #[test]
    fn test_time_grid_days() {
        let min = Date::new(2024, 1, 1);
        let max = Date::new(2024, 1, 14);
        let grid = compute_time_grid(&min, &max, 13);
        assert!(matches!(grid.scale, TimeScale::Days));
        assert!(!grid.labels.is_empty());
    }

    #[test]
    fn test_time_grid_weeks() {
        let min = Date::new(2024, 1, 1);
        let max = Date::new(2024, 3, 1);
        let grid = compute_time_grid(&min, &max, 60);
        assert!(matches!(grid.scale, TimeScale::Weeks));
    }

    #[test]
    fn test_time_grid_months() {
        let min = Date::new(2024, 1, 1);
        let max = Date::new(2024, 12, 31);
        let grid = compute_time_grid(&min, &max, 365);
        assert!(matches!(grid.scale, TimeScale::Months));
    }

    #[test]
    fn test_split_specs() {
        let specs = split_specs("5d, after Research");
        assert_eq!(specs, vec!["5d", "after Research"]);

        let specs = split_specs("after A + 3d, 10d");
        assert_eq!(specs, vec!["after A + 3d", "10d"]);
    }

    #[test]
    fn test_workday_duration() {
        let content = "- Task: 2024-01-15, 5wd"; // Monday + 5 workdays
        let data = parse_gantt(content);
        let resolved = resolve_tasks(&data);
        assert_eq!(resolved[0].start.format(), "2024-01-15");
        assert_eq!(resolved[0].end.format(), "2024-01-22"); // Next Monday
    }
}
