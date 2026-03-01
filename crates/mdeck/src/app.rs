use eframe::egui;
use std::path::PathBuf;
use std::time::Instant;

use crate::config::Config;
use crate::parser::{self, Presentation};
use crate::render;
use crate::render::image_cache::ImageCache;
use crate::render::transition::{
    ActiveTransition, TransitionDirection, TransitionKind, ease_in_out,
};
use crate::theme::Theme;

const OVERVIEW_TRANSITION_DURATION: f32 = 0.4;
const DRAW_FADE_DURATION: f32 = 8.0;
const DRAG_THRESHOLD: f32 = 5.0;

/// A freehand pen stroke (left-drag)
struct PenStroke {
    points: Vec<egui::Pos2>,
    start: Instant,
    slide_index: usize,
}

/// An arrow annotation (right-drag)
struct ArrowAnnotation {
    from: egui::Pos2,
    to: egui::Pos2,
    start: Instant,
    slide_index: usize,
}

/// Tracks an in-progress mouse interaction
enum ActiveDraw {
    None,
    /// Left button held: collecting points, might still be a click
    PenPending {
        origin: egui::Pos2,
        points: Vec<egui::Pos2>,
    },
    /// Left button held: drag threshold exceeded, definitely drawing
    PenDrawing {
        points: Vec<egui::Pos2>,
    },
    /// Right button held: collecting start/end, might still be a click
    ArrowPending {
        origin: egui::Pos2,
        current: egui::Pos2,
    },
    /// Right button held: drag threshold exceeded, definitely an arrow
    ArrowDrawing {
        from: egui::Pos2,
        current: egui::Pos2,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AppMode {
    Presentation,
    Grid { selected: usize },
    OverviewTransition { selected: usize, entering: bool },
}

struct PresentationApp {
    presentation: Presentation,
    #[allow(dead_code)]
    file_path: PathBuf,
    current_slide: usize,
    mode: AppMode,
    theme: Theme,
    default_transition: TransitionKind,
    transition: Option<ActiveTransition>,
    image_cache: ImageCache,
    show_hud: bool,
    show_raw_markdown: bool,
    toast: Option<Toast>,
    last_ctrl_c: Option<Instant>,
    last_esc: Option<Instant>,
    reveal_steps: Vec<usize>,
    max_steps: Vec<usize>,
    /// Timestamp of when each slide's reveal_step was last incremented (for animation)
    reveal_timestamps: Vec<Option<Instant>>,
    scroll_offsets: Vec<f32>,
    scroll_targets: Vec<f32>,
    frame_count: u32,
    fps: f32,
    fps_update: Instant,
    overview_transition_start: Option<Instant>,
    pen_strokes: Vec<PenStroke>,
    arrows: Vec<ArrowAnnotation>,
    active_draw: ActiveDraw,
    /// Cached slide rect from last frame, used for mouse coordinate conversion
    last_slide_rect: egui::Rect,
    /// Which grid cell the mouse is hovering over
    hover_slide: Option<usize>,
    /// Whether to show hover effect (false when keyboard took over)
    use_hover: bool,
    /// Last known hover position, used to detect actual mouse movement
    last_hover_pos: Option<egui::Pos2>,
    /// Current animated scroll position in grid
    grid_scroll_offset: f32,
    /// Target scroll position in grid
    grid_scroll_target: f32,
}

struct Toast {
    message: String,
    start: Instant,
}

impl Toast {
    fn new(message: String) -> Self {
        Self {
            message,
            start: Instant::now(),
        }
    }

    fn opacity(&self) -> f32 {
        let elapsed = self.start.elapsed().as_secs_f32();
        let duration = 1.5;
        let fade_start = 1.0;
        if elapsed < fade_start {
            1.0
        } else if elapsed < duration {
            1.0 - (elapsed - fade_start) / (duration - fade_start)
        } else {
            0.0
        }
    }

    fn is_expired(&self) -> bool {
        self.start.elapsed().as_secs_f32() >= 1.5
    }
}

impl PresentationApp {
    fn new(file: PathBuf, presentation: Presentation, windowed: bool) -> Self {
        let _ = windowed; // used at window creation time

        let theme_name = presentation.meta.theme.as_deref().unwrap_or("light");
        let theme = Theme::from_name(theme_name);

        let transition_name = presentation.meta.transition.as_deref().unwrap_or("slide");
        let default_transition = TransitionKind::from_name(transition_name);

        let base_path = file
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let image_cache = ImageCache::new(base_path);

        let max_steps: Vec<usize> = presentation
            .slides
            .iter()
            .map(|s| parser::compute_max_steps(&s.blocks))
            .collect();
        let slide_count = presentation.slides.len();
        let reveal_steps = vec![0; slide_count];
        let reveal_timestamps = vec![None; slide_count];
        let scroll_offsets = vec![0.0; slide_count];
        let scroll_targets = vec![0.0; slide_count];

        let now = Instant::now();
        Self {
            presentation,
            file_path: file,
            current_slide: 0,
            mode: AppMode::Presentation,
            theme,
            default_transition,
            transition: None,
            image_cache,
            show_hud: false,
            show_raw_markdown: false,
            toast: None,
            last_ctrl_c: None,
            last_esc: None,
            reveal_steps,
            max_steps,
            reveal_timestamps,
            scroll_offsets,
            scroll_targets,
            frame_count: 0,
            fps: 0.0,
            fps_update: now,
            overview_transition_start: None,
            pen_strokes: Vec::new(),
            arrows: Vec::new(),
            active_draw: ActiveDraw::None,
            last_slide_rect: egui::Rect::ZERO,
            hover_slide: None,
            use_hover: false,
            last_hover_pos: None,
            grid_scroll_offset: 0.0,
            grid_scroll_target: 0.0,
        }
    }

    fn slide_count(&self) -> usize {
        self.presentation.slides.len()
    }

    fn display_title(&self) -> String {
        self.presentation.meta.title.clone().unwrap_or_else(|| {
            self.file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
    }

    fn navigate_forward(&mut self) {
        if self.transition.is_some() {
            return;
        }

        let idx = self.current_slide;

        // If we have reveal steps remaining, reveal next item
        if self.reveal_steps[idx] < self.max_steps[idx] {
            self.reveal_steps[idx] += 1;
            self.reveal_timestamps[idx] = Some(Instant::now());
            return;
        }

        // Otherwise advance to the next slide
        if idx >= self.slide_count().saturating_sub(1) {
            return;
        }

        self.scroll_offsets[idx] = 0.0;
        self.scroll_targets[idx] = 0.0;
        self.transition = Some(ActiveTransition::new(
            idx,
            idx + 1,
            self.default_transition,
            TransitionDirection::Forward,
        ));
    }

    fn navigate_backward(&mut self) {
        if self.transition.is_some() {
            return;
        }

        let idx = self.current_slide;

        // If we've revealed items, un-reveal
        if self.reveal_steps[idx] > 0 {
            self.reveal_steps[idx] -= 1;
            return;
        }

        // Otherwise go to previous slide (fully revealed)
        if idx == 0 {
            return;
        }

        self.scroll_offsets[idx] = 0.0;
        self.scroll_targets[idx] = 0.0;
        let prev = idx - 1;
        // Show previous slide fully revealed
        self.reveal_steps[prev] = self.max_steps[prev];

        self.transition = Some(ActiveTransition::new(
            idx,
            prev,
            self.default_transition,
            TransitionDirection::Backward,
        ));
    }

    fn jump_to_slide(&mut self, index: usize) {
        if index < self.slide_count() && self.transition.is_none() {
            let cur = self.current_slide;
            self.scroll_offsets[cur] = 0.0;
            self.scroll_targets[cur] = 0.0;
            self.current_slide = index;
        }
    }

    fn toggle_theme(&mut self) {
        self.theme = self.theme.toggled();
        self.toast = Some(Toast::new(format!("Theme: {}", self.theme.name)));
    }

    fn cycle_transition(&mut self) {
        self.default_transition = match self.default_transition {
            TransitionKind::SlideHorizontal => TransitionKind::Fade,
            TransitionKind::Fade => TransitionKind::Spatial,
            TransitionKind::Spatial => TransitionKind::None,
            TransitionKind::None => TransitionKind::SlideHorizontal,
        };
        let name = match self.default_transition {
            TransitionKind::SlideHorizontal => "Slide",
            TransitionKind::Fade => "Fade",
            TransitionKind::Spatial => "Spatial",
            TransitionKind::None => "None",
        };
        self.toast = Some(Toast::new(format!("Transition: {name}")));
    }

    fn update_fps(&mut self) {
        self.frame_count += 1;
        let elapsed = self.fps_update.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            self.fps = self.frame_count as f32 / elapsed;
            self.frame_count = 0;
            self.fps_update = Instant::now();
        }
    }

    fn draw_slide(&self, ui: &egui::Ui, index: usize, rect: egui::Rect, opacity: f32, scale: f32) {
        if index < self.presentation.slides.len() {
            let reveal = self.reveal_steps.get(index).copied().unwrap_or(0);
            let timestamp = self.reveal_timestamps.get(index).copied().flatten();
            render::render_slide(
                ui,
                &self.presentation.slides[index],
                &self.theme,
                rect,
                opacity,
                &self.image_cache,
                reveal,
                timestamp,
                scale,
            );
        }
    }

    fn grid_columns(&self) -> usize {
        let count = self.slide_count();
        if count <= 4 {
            2
        } else if count <= 9 {
            3
        } else {
            4
        }
    }

    fn grid_cell_rect(
        &self,
        index: usize,
        rect: egui::Rect,
        scale: f32,
        scroll_offset: f32,
    ) -> egui::Rect {
        let cols = self.grid_columns();
        let count = self.slide_count();
        let rows = count.div_ceil(cols);

        let padding = 24.0 * scale;
        let gap = 12.0 * scale;

        let grid_top = rect.top() + padding + 40.0 * scale;
        let grid_width = rect.width() - padding * 2.0;
        let grid_height = rect.bottom() - grid_top - padding;

        let cell_width = (grid_width - gap * (cols as f32 - 1.0)) / cols as f32;
        let natural_height = cell_width * 9.0 / 16.0;
        let total_natural = rows as f32 * natural_height + (rows as f32 - 1.0) * gap;

        // If natural layout fits in the viewport, clamp to viewport; otherwise use natural size
        let cell_height = if total_natural <= grid_height {
            let cell_height_max = (grid_height - gap * (rows as f32 - 1.0)) / rows as f32;
            cell_height_max.min(natural_height)
        } else {
            natural_height
        };

        let col = index % cols;
        let row = index / cols;
        let x = rect.left() + padding + col as f32 * (cell_width + gap);
        let y = grid_top + row as f32 * (cell_height + gap) - scroll_offset;

        egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cell_width, cell_height))
    }

    /// Total content height of the grid (for scroll calculation)
    fn grid_content_height(&self, rect: egui::Rect, scale: f32) -> f32 {
        let cols = self.grid_columns();
        let count = self.slide_count();
        let rows = count.div_ceil(cols);

        let padding = 24.0 * scale;
        let gap = 12.0 * scale;
        let grid_width = rect.width() - padding * 2.0;
        let cell_width = (grid_width - gap * (cols as f32 - 1.0)) / cols as f32;
        let cell_height = cell_width * 9.0 / 16.0;

        rows as f32 * cell_height + (rows as f32 - 1.0) * gap
    }

    /// Available viewport height for grid content
    fn grid_available_height(&self, rect: egui::Rect, scale: f32) -> f32 {
        let padding = 24.0 * scale;
        let grid_top = rect.top() + padding + 40.0 * scale;
        rect.bottom() - grid_top - padding
    }

    fn compute_scale(rect: egui::Rect) -> f32 {
        let ref_w = 1920.0;
        let ref_h = 1080.0;
        (rect.width() / ref_w).min(rect.height() / ref_h)
    }

    /// Convert screen position to slide-local coordinates (accounting for scroll)
    fn screen_to_local(&self, screen_pos: egui::Pos2) -> egui::Pos2 {
        let rect = self.last_slide_rect;
        let scroll = self.scroll_offsets[self.current_slide];
        egui::pos2(
            screen_pos.x - rect.left(),
            screen_pos.y - rect.top() + scroll,
        )
    }

    /// Convert slide-local coordinates back to screen position
    fn local_to_screen(&self, local: egui::Pos2) -> egui::Pos2 {
        let rect = self.last_slide_rect;
        let scroll = self.scroll_offsets[self.current_slide];
        egui::pos2(local.x + rect.left(), local.y + rect.top() - scroll)
    }

    fn handle_mouse_input(&mut self, ctx: &egui::Context) {
        let (primary_pressed, primary_down, secondary_pressed, secondary_down, pointer_pos) = ctx
            .input(|i| {
                let pp = i.pointer.button_pressed(egui::PointerButton::Primary);
                let pd = i.pointer.button_down(egui::PointerButton::Primary);
                let sp = i.pointer.button_pressed(egui::PointerButton::Secondary);
                let sd = i.pointer.button_down(egui::PointerButton::Secondary);
                let pos = i.pointer.hover_pos();
                (pp, pd, sp, sd, pos)
            });

        let Some(pos) = pointer_pos else { return };
        let local = self.screen_to_local(pos);

        // Left button press → start PenPending
        if primary_pressed {
            self.active_draw = ActiveDraw::PenPending {
                origin: local,
                points: vec![local],
            };
            return;
        }

        // Right button press → start ArrowPending
        if secondary_pressed {
            self.active_draw = ActiveDraw::ArrowPending {
                origin: local,
                current: local,
            };
            return;
        }

        // Left button held
        if primary_down {
            match &mut self.active_draw {
                ActiveDraw::PenPending { origin, points } => {
                    points.push(local);
                    if origin.distance(local) > DRAG_THRESHOLD {
                        let pts = std::mem::take(points);
                        self.active_draw = ActiveDraw::PenDrawing { points: pts };
                    }
                }
                ActiveDraw::PenDrawing { points } => {
                    points.push(local);
                }
                _ => {}
            }
            ctx.request_repaint();
            return;
        }

        // Right button held
        if secondary_down {
            match &mut self.active_draw {
                ActiveDraw::ArrowPending { origin, current } => {
                    *current = local;
                    if origin.distance(local) > DRAG_THRESHOLD {
                        let from = *origin;
                        self.active_draw = ActiveDraw::ArrowDrawing {
                            from,
                            current: local,
                        };
                    }
                }
                ActiveDraw::ArrowDrawing { current, .. } => {
                    *current = local;
                }
                _ => {}
            }
            ctx.request_repaint();
            return;
        }

        // Button released — commit or navigate
        match std::mem::replace(&mut self.active_draw, ActiveDraw::None) {
            ActiveDraw::PenPending { .. } => {
                self.navigate_forward();
            }
            ActiveDraw::PenDrawing { points } => {
                if points.len() >= 2 {
                    self.pen_strokes.push(PenStroke {
                        points,
                        start: Instant::now(),
                        slide_index: self.current_slide,
                    });
                }
            }
            ActiveDraw::ArrowPending { .. } => {
                self.navigate_backward();
            }
            ActiveDraw::ArrowDrawing { from, current } => {
                self.arrows.push(ArrowAnnotation {
                    from,
                    to: current,
                    start: Instant::now(),
                    slide_index: self.current_slide,
                });
            }
            ActiveDraw::None => {}
        }
    }
}

impl eframe::App for PresentationApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_fps();

        let mode = self.mode;

        // Collect viewport commands to send AFTER the input closure
        // (sending inside ctx.input() causes RwLock deadlock)
        let mut viewport_cmds: Vec<egui::ViewportCommand> = Vec::new();

        // Handle keyboard input
        ctx.input(|i| {
            // Quit: Q from any mode
            if i.key_pressed(egui::Key::Q) {
                viewport_cmds.push(egui::ViewportCommand::Close);
                return;
            }

            // Ctrl+C double-tap to quit
            if i.modifiers.ctrl && i.key_pressed(egui::Key::C) {
                if let Some(last) = self.last_ctrl_c {
                    if last.elapsed().as_secs_f32() < 1.0 {
                        viewport_cmds.push(egui::ViewportCommand::Close);
                        return;
                    }
                }
                self.last_ctrl_c = Some(Instant::now());
                self.toast = Some(Toast::new("Press Ctrl+C again to quit".to_string()));
                return;
            }

            // ESC: clear drawings first (presentation mode), then double-tap to quit
            if i.key_pressed(egui::Key::Escape) {
                // In presentation mode, first ESC clears annotations if any exist
                if matches!(mode, AppMode::Presentation) {
                    let idx = self.current_slide;
                    let has_annotations = self.pen_strokes.iter().any(|s| s.slide_index == idx)
                        || self.arrows.iter().any(|a| a.slide_index == idx);
                    if has_annotations {
                        self.pen_strokes.retain(|s| s.slide_index != idx);
                        self.arrows.retain(|a| a.slide_index != idx);
                        self.last_esc = None;
                        return;
                    }
                }
                // Double-tap to quit (from any mode)
                if let Some(last) = self.last_esc {
                    if last.elapsed().as_secs_f32() < 1.0 {
                        viewport_cmds.push(egui::ViewportCommand::Close);
                        return;
                    }
                }
                self.last_esc = Some(Instant::now());
                self.toast = Some(Toast::new("Press Esc again to exit".to_string()));
                return;
            }

            // Fullscreen toggle: F (from any mode)
            if i.key_pressed(egui::Key::F) {
                viewport_cmds.push(egui::ViewportCommand::Fullscreen(
                    !i.viewport().fullscreen.unwrap_or(false),
                ));
                return;
            }

            // Theme toggle: D (from any mode)
            if i.key_pressed(egui::Key::D) {
                self.toggle_theme();
                return;
            }

            // Cycle transition: T (from any mode)
            if i.key_pressed(egui::Key::T) {
                self.cycle_transition();
                return;
            }

            match mode {
                AppMode::Presentation => {
                    // Forward: Right, N, Space
                    if i.key_pressed(egui::Key::ArrowRight)
                        || i.key_pressed(egui::Key::N)
                        || i.key_pressed(egui::Key::Space)
                    {
                        self.navigate_forward();
                    }
                    // Backward: Left, P
                    if i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::P) {
                        self.navigate_backward();
                    }
                    // Toggle HUD: H
                    if i.key_pressed(egui::Key::H) {
                        self.show_hud = !self.show_hud;
                    }
                    // Toggle raw markdown overlay: R
                    if i.key_pressed(egui::Key::R) {
                        self.show_raw_markdown = !self.show_raw_markdown;
                    }
                    // Scroll: Up/Down (animate toward target)
                    if i.key_pressed(egui::Key::ArrowUp) {
                        let idx = self.current_slide;
                        self.scroll_targets[idx] = (self.scroll_targets[idx] - 120.0).max(0.0);
                    }
                    if i.key_pressed(egui::Key::ArrowDown) {
                        let idx = self.current_slide;
                        // Max will be clamped at render time when we know content height
                        self.scroll_targets[idx] += 120.0;
                    }
                    // Mouse wheel scroll
                    let scroll = i.smooth_scroll_delta;
                    if scroll.y != 0.0 {
                        let idx = self.current_slide;
                        self.scroll_targets[idx] -= scroll.y;
                    }
                    // Home/End
                    if i.key_pressed(egui::Key::Home) {
                        self.jump_to_slide(0);
                    }
                    if i.key_pressed(egui::Key::End) {
                        self.jump_to_slide(self.slide_count().saturating_sub(1));
                    }
                    // G: animate into grid overview
                    if i.key_pressed(egui::Key::G) && self.transition.is_none() {
                        self.mode = AppMode::OverviewTransition {
                            selected: self.current_slide,
                            entering: true,
                        };
                        self.overview_transition_start = Some(Instant::now());
                        self.show_hud = false;
                        self.grid_scroll_offset = 0.0;
                        self.grid_scroll_target = 0.0;
                        self.hover_slide = None;
                        self.use_hover = false;
                    }
                }
                AppMode::Grid { selected } => {
                    let cols = self.grid_columns();
                    let count = self.slide_count();

                    // Arrow navigation in grid
                    if i.key_pressed(egui::Key::ArrowRight) {
                        let next = (selected + 1).min(count.saturating_sub(1));
                        self.mode = AppMode::Grid { selected: next };
                        self.use_hover = false;
                    }
                    if i.key_pressed(egui::Key::ArrowLeft) {
                        let prev = selected.saturating_sub(1);
                        self.mode = AppMode::Grid { selected: prev };
                        self.use_hover = false;
                    }
                    if i.key_pressed(egui::Key::ArrowDown) {
                        let next = (selected + cols).min(count.saturating_sub(1));
                        self.mode = AppMode::Grid { selected: next };
                        self.use_hover = false;
                    }
                    if i.key_pressed(egui::Key::ArrowUp) {
                        let prev = selected.saturating_sub(cols);
                        self.mode = AppMode::Grid { selected: prev };
                        self.use_hover = false;
                    }

                    // Enter / Space / E: animate back to selected slide
                    if i.key_pressed(egui::Key::Enter)
                        || i.key_pressed(egui::Key::Space)
                        || i.key_pressed(egui::Key::E)
                    {
                        self.use_hover = false;
                        self.mode = AppMode::OverviewTransition {
                            selected,
                            entering: false,
                        };
                        self.overview_transition_start = Some(Instant::now());
                    }
                }
                AppMode::OverviewTransition { .. } => {
                    // Block input during overview animation
                }
            }
        });

        // Send collected viewport commands outside the input closure
        for cmd in viewport_cmds {
            ctx.send_viewport_cmd(cmd);
        }

        // Mouse input handling (presentation mode only, outside ctx.input closure)
        if matches!(mode, AppMode::Presentation) && self.transition.is_none() {
            self.handle_mouse_input(ctx);
        }

        // Expire old annotations
        self.pen_strokes
            .retain(|s| s.start.elapsed().as_secs_f32() < DRAW_FADE_DURATION);
        self.arrows
            .retain(|a| a.start.elapsed().as_secs_f32() < DRAW_FADE_DURATION);
        if !self.pen_strokes.is_empty() || !self.arrows.is_empty() {
            ctx.request_repaint();
        }

        // Advance transition
        if let Some(ref t) = self.transition {
            if t.is_complete() {
                let to = t.to;
                self.transition = None;
                self.current_slide = to;
            }
        }

        // Complete overview transition
        if let AppMode::OverviewTransition { selected, entering } = self.mode {
            if let Some(start) = self.overview_transition_start {
                if start.elapsed().as_secs_f32() >= OVERVIEW_TRANSITION_DURATION {
                    if entering {
                        self.mode = AppMode::Grid { selected };
                    } else {
                        self.current_slide = selected;
                        self.mode = AppMode::Presentation;
                    }
                    self.overview_transition_start = None;
                }
            }
        }

        // Expire toast
        if self.toast.as_ref().is_some_and(|t| t.is_expired()) {
            self.toast = None;
        }

        let bg = self.theme.background;

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg).inner_margin(0.0))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 0.0, bg);

                let scale = Self::compute_scale(rect);

                match self.mode {
                    AppMode::Presentation => {
                        self.draw_presentation_with_scroll(ui, ctx, rect, scale);
                    }
                    AppMode::Grid { selected } => {
                        self.draw_grid(ui, ctx, rect, selected, scale);
                    }
                    AppMode::OverviewTransition { selected, entering } => {
                        self.draw_overview_transition(ui, ctx, rect, scale, selected, entering);
                    }
                }

                // Toast notification (shown in both modes)
                if let Some(ref toast) = self.toast {
                    let opacity = toast.opacity();
                    if opacity > 0.0 {
                        let toast_color = Theme::with_opacity(self.theme.foreground, opacity * 0.9);
                        let toast_bg =
                            Theme::with_opacity(self.theme.code_background, opacity * 0.9);
                        let galley = ui.painter().layout_no_wrap(
                            toast.message.clone(),
                            egui::FontId::proportional(20.0 * scale),
                            toast_color,
                        );
                        let padding = 16.0 * scale;
                        let toast_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                rect.center().x - galley.rect.width() / 2.0 - padding,
                                rect.bottom() - 80.0 * scale,
                            ),
                            egui::vec2(
                                galley.rect.width() + padding * 2.0,
                                galley.rect.height() + padding * 2.0,
                            ),
                        );
                        ui.painter().rect_filled(toast_rect, 8.0 * scale, toast_bg);
                        let text_pos =
                            egui::pos2(toast_rect.left() + padding, toast_rect.top() + padding);
                        ui.painter().galley(text_pos, galley, toast_color);
                        ctx.request_repaint();
                    }
                }

                // HUD overlay (presentation mode only)
                if self.show_hud && matches!(self.mode, AppMode::Presentation) {
                    draw_hud(ui, &self.theme, rect, scale);
                }

                // Raw markdown overlay (presentation mode only)
                if self.show_raw_markdown && matches!(self.mode, AppMode::Presentation) {
                    let raw = &self.presentation.slides[self.current_slide].raw_source;
                    draw_raw_markdown_overlay(ui, raw, &self.theme, rect, scale);
                }
            });
    }
}

impl PresentationApp {
    fn draw_presentation_with_scroll(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        rect: egui::Rect,
        scale: f32,
    ) {
        // Cache slide rect for mouse coordinate conversion
        self.last_slide_rect = rect;

        // During transitions, just render normally (no scroll)
        if self.transition.is_some() {
            self.draw_presentation(ui, ctx, rect, scale);
            self.draw_annotations(ui, scale);
            return;
        }

        let idx = self.current_slide;
        let slide = &self.presentation.slides[idx];
        let (content_height, available_height) =
            render::measure_slide_content_height(ui, slide, &self.theme, rect, scale);
        let overflow = content_height - available_height;

        if overflow <= 0.0 {
            // No overflow — render normally, reset scroll
            self.scroll_offsets[idx] = 0.0;
            self.scroll_targets[idx] = 0.0;
            self.draw_presentation(ui, ctx, rect, scale);
            self.draw_annotations(ui, scale);
            return;
        }

        // Clamp target
        self.scroll_targets[idx] = self.scroll_targets[idx].clamp(0.0, overflow);

        // Animate: lerp current offset toward target
        let target = self.scroll_targets[idx];
        let current = self.scroll_offsets[idx];
        let diff = target - current;
        if diff.abs() < 0.5 {
            self.scroll_offsets[idx] = target;
        } else {
            // Smooth ease: move 15% of remaining distance each frame
            self.scroll_offsets[idx] = current + diff * 0.15;
            ctx.request_repaint();
        }
        let scroll_offset = self.scroll_offsets[idx];

        // Render slide inside a clipped child UI so content doesn't bleed outside
        let scrolled_rect = rect.translate(egui::vec2(0.0, -scroll_offset));
        let reveal = self.reveal_steps.get(idx).copied().unwrap_or(0);
        let timestamp = self.reveal_timestamps.get(idx).copied().flatten();
        let child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt("scroll_clip"));
        render::render_slide(
            &child_ui,
            slide,
            &self.theme,
            scrolled_rect,
            1.0,
            &self.image_cache,
            reveal,
            timestamp,
            scale,
        );

        // Draw fade-out gradient at bottom
        let fade_h = 80.0 * scale;
        if scroll_offset < overflow - 0.5 {
            draw_fade_gradient(ui, rect, fade_h, &self.theme, false);
        }
        // Draw fade-in gradient at top when scrolled
        if scroll_offset > 0.5 {
            draw_fade_gradient(ui, rect, fade_h, &self.theme, true);
        }

        // Draw scroll indicators
        let indicator_color = Theme::with_opacity(self.theme.foreground, 0.35);
        let indicator_font = egui::FontId::proportional(self.theme.body_size * 0.4 * scale);
        if scroll_offset < overflow - 0.5 {
            let galley = ui.painter().layout_no_wrap(
                "\u{25BC}".to_string(),
                indicator_font.clone(),
                indicator_color,
            );
            let pos = egui::pos2(
                rect.center().x - galley.rect.width() / 2.0,
                rect.bottom() - 40.0 * scale,
            );
            ui.painter().galley(pos, galley, indicator_color);
        }
        if scroll_offset > 0.5 {
            let galley = ui.painter().layout_no_wrap(
                "\u{25B2}".to_string(),
                indicator_font,
                indicator_color,
            );
            let pos = egui::pos2(
                rect.center().x - galley.rect.width() / 2.0,
                rect.top() + 10.0 * scale,
            );
            ui.painter().galley(pos, galley, indicator_color);
        }

        // Draw annotations on top of slide content
        self.draw_annotations(ui, scale);

        // Footer, counter, FPS
        self.draw_presentation_chrome(ui, rect, scale);
    }

    fn draw_presentation(&self, ui: &egui::Ui, ctx: &egui::Context, rect: egui::Rect, scale: f32) {
        if let Some(ref t) = self.transition {
            let kind = t.kind;
            let from = t.from;
            let to = t.to;
            let progress = t.progress();
            let direction = t.direction;

            match kind {
                TransitionKind::Fade => {
                    self.draw_slide(ui, from, rect, 1.0 - progress, scale);
                    self.draw_slide(ui, to, rect, progress, scale);
                }
                TransitionKind::SlideHorizontal => {
                    let w = rect.width();
                    let sign = match direction {
                        TransitionDirection::Forward => -1.0,
                        TransitionDirection::Backward => 1.0,
                    };
                    let from_offset = sign * progress * w;
                    let to_offset = from_offset - sign * w;

                    let from_rect = rect.translate(egui::vec2(from_offset, 0.0));
                    let to_rect = rect.translate(egui::vec2(to_offset, 0.0));

                    self.draw_slide(ui, from, from_rect, 1.0, scale);
                    self.draw_slide(ui, to, to_rect, 1.0, scale);
                }
                TransitionKind::Spatial => {
                    let (dx, dy) = t.spatial_direction(self.grid_columns());
                    let w = rect.width();
                    let h = rect.height();

                    let from_rect =
                        rect.translate(egui::vec2(-dx * progress * w, -dy * progress * h));
                    let to_rect = rect.translate(egui::vec2(
                        dx * (1.0 - progress) * w,
                        dy * (1.0 - progress) * h,
                    ));

                    self.draw_slide(ui, from, from_rect, 1.0, scale);
                    self.draw_slide(ui, to, to_rect, 1.0, scale);
                }
                TransitionKind::None => {
                    self.draw_slide(ui, to, rect, 1.0, scale);
                }
            }
            ctx.request_repaint();
        } else {
            self.draw_slide(ui, self.current_slide, rect, 1.0, scale);
        }

        self.draw_presentation_chrome(ui, rect, scale);
    }

    fn draw_presentation_chrome(&self, ui: &egui::Ui, rect: egui::Rect, scale: f32) {
        // Footer
        if let Some(ref footer) = self.presentation.meta.footer {
            let footer_color = Theme::with_opacity(self.theme.foreground, 0.4);
            let galley = ui.painter().layout_no_wrap(
                footer.clone(),
                egui::FontId::proportional(14.0 * scale),
                footer_color,
            );
            let pos = egui::pos2(
                rect.center().x - galley.rect.width() / 2.0,
                rect.bottom() - 30.0 * scale,
            );
            ui.painter().galley(pos, galley, footer_color);
        }

        // Slide counter
        let counter_text = format!("{} / {}", self.current_slide + 1, self.slide_count());
        let counter_color = Theme::with_opacity(self.theme.foreground, 0.3);
        let counter_galley = ui.painter().layout_no_wrap(
            counter_text,
            egui::FontId::monospace(14.0 * scale),
            counter_color,
        );
        let counter_pos = egui::pos2(
            rect.right() - counter_galley.rect.width() - 16.0 * scale,
            rect.bottom() - 30.0 * scale,
        );
        ui.painter()
            .galley(counter_pos, counter_galley, counter_color);

        // FPS overlay
        let fps_text = format!("{:.0} fps", self.fps);
        let fps_color = Theme::with_opacity(self.theme.foreground, 0.3);
        let fps_galley =
            ui.painter()
                .layout_no_wrap(fps_text, egui::FontId::monospace(14.0 * scale), fps_color);
        let fps_pos = egui::pos2(
            rect.right() - fps_galley.rect.width() - 12.0 * scale,
            rect.top() + 10.0 * scale,
        );
        ui.painter().galley(fps_pos, fps_galley, fps_color);
    }

    fn draw_grid(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        rect: egui::Rect,
        selected: usize,
        scale: f32,
    ) {
        let count = self.slide_count();
        let padding = 24.0 * scale;

        // --- Grid scrolling ---
        let content_h = self.grid_content_height(rect, scale);
        let available_h = self.grid_available_height(rect, scale);
        let overflow = (content_h - available_h).max(0.0);

        // Mouse wheel scrolling in grid
        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 && overflow > 0.0 {
            self.grid_scroll_target = (self.grid_scroll_target - scroll_delta).clamp(0.0, overflow);
        }

        // Clamp target
        self.grid_scroll_target = self.grid_scroll_target.clamp(0.0, overflow);

        // Animate scroll
        let diff = self.grid_scroll_target - self.grid_scroll_offset;
        if diff.abs() < 0.5 {
            self.grid_scroll_offset = self.grid_scroll_target;
        } else {
            self.grid_scroll_offset += diff * 0.15;
            ctx.request_repaint();
        }

        let scroll = self.grid_scroll_offset;

        // --- Mouse hover detection ---
        let hover_pos = ctx.input(|i| i.pointer.hover_pos());
        let mut hovered: Option<usize> = None;
        // Clip area for grid cells (below title, above hint)
        let grid_top = rect.top() + padding + 40.0 * scale;
        let grid_bottom = rect.bottom() - padding;
        let clip_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), grid_top),
            egui::pos2(rect.right(), grid_bottom),
        );

        // Detect whether the mouse has actually moved since last frame
        let mouse_moved = match (hover_pos, self.last_hover_pos) {
            (Some(cur), Some(prev)) => cur.distance(prev) > 1.0,
            (Some(_), None) => true,
            _ => false,
        };
        self.last_hover_pos = hover_pos;

        if let Some(hp) = hover_pos {
            for i in 0..count {
                let cell_rect = self.grid_cell_rect(i, rect, scale, scroll);
                let visible = cell_rect.intersects(clip_rect);
                if visible && cell_rect.contains(hp) && clip_rect.contains(hp) {
                    hovered = Some(i);
                    break;
                }
            }
        }
        if hovered.is_some() {
            self.hover_slide = hovered;
            // Only re-enable hover when the mouse has actually moved
            if mouse_moved {
                self.use_hover = true;
            }
        } else if hover_pos.is_some() {
            self.hover_slide = None;
        }

        // --- Mouse click detection ---
        let clicked = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        if clicked {
            if let Some(hi) = self.hover_slide {
                // Click on a grid cell → zoom into that slide
                self.mode = AppMode::OverviewTransition {
                    selected: hi,
                    entering: false,
                };
                self.overview_transition_start = Some(Instant::now());
                return;
            }
        }

        // --- Ensure selected cell is visible when using keyboard ---
        if !self.use_hover && overflow > 0.0 {
            let sel_rect = self.grid_cell_rect(selected, rect, scale, scroll);
            if sel_rect.top() < grid_top {
                self.grid_scroll_target -= grid_top - sel_rect.top() + padding;
                self.grid_scroll_target = self.grid_scroll_target.max(0.0);
            } else if sel_rect.bottom() > grid_bottom {
                self.grid_scroll_target += sel_rect.bottom() - grid_bottom + padding;
                self.grid_scroll_target = self.grid_scroll_target.min(overflow);
            }
        }

        // Title
        let title_color = Theme::with_opacity(self.theme.heading_color, 0.9);
        let title_galley = ui.painter().layout_no_wrap(
            self.display_title(),
            egui::FontId::proportional(24.0 * scale),
            title_color,
        );
        let title_pos = egui::pos2(rect.left() + padding, rect.top() + padding);
        ui.painter().galley(title_pos, title_galley, title_color);

        // Render grid cells clipped to the grid area
        let mut grid_child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(clip_rect)
                .id_salt("grid_clip"),
        );

        for i in 0..count {
            let cell_rect = self.grid_cell_rect(i, rect, scale, scroll);

            // Skip cells entirely outside the visible area
            if !cell_rect.intersects(clip_rect) {
                continue;
            }

            let cell_scale = (cell_rect.width() / 1920.0).min(cell_rect.height() / 1080.0);

            // Fill cell with theme background
            grid_child
                .painter()
                .rect_filled(cell_rect, 4.0 * scale, self.theme.background);

            // Render actual slide content clipped to cell
            let child_ui = grid_child.new_child(
                egui::UiBuilder::new()
                    .max_rect(cell_rect)
                    .id_salt(("grid_cell", i)),
            );
            self.draw_slide(&child_ui, i, cell_rect, 1.0, cell_scale);

            // Slide number badge overlay
            self.draw_slide_badge(&grid_child, cell_rect, i, scale, 1.0);

            // Hover highlight (subtle glow, distinct from selection)
            if self.use_hover && self.hover_slide == Some(i) && i != selected {
                let hover_color = Theme::with_opacity(self.theme.accent, 0.12);
                grid_child
                    .painter()
                    .rect_filled(cell_rect, 4.0 * scale, hover_color);
                grid_child.painter().rect_stroke(
                    cell_rect.expand(2.0 * scale),
                    4.0 * scale,
                    egui::Stroke::new(2.0 * scale, Theme::with_opacity(self.theme.accent, 0.5)),
                    egui::StrokeKind::Outside,
                );
            }

            // Selected border (drawn AFTER preview so it's on top)
            if i == selected {
                grid_child.painter().rect_stroke(
                    cell_rect,
                    4.0 * scale,
                    egui::Stroke::new(3.0 * scale, self.theme.accent),
                    egui::StrokeKind::Outside,
                );
            }
        }

        // Fade gradients at screen edges when scrolled
        let fade_h = 60.0 * scale;
        if scroll > 0.5 {
            draw_fade_gradient(ui, rect, fade_h, &self.theme, true);
        }
        if scroll < overflow - 0.5 {
            draw_fade_gradient(ui, rect, fade_h, &self.theme, false);
        }

        // Navigation hint at bottom
        let hint = "Arrows/Mouse: navigate  |  Enter/Click: select  |  Q: quit";
        let hint_color = Theme::with_opacity(self.theme.foreground, 0.4);
        let hint_galley = ui.painter().layout_no_wrap(
            hint.to_string(),
            egui::FontId::proportional(14.0 * scale),
            hint_color,
        );
        let hint_pos = egui::pos2(
            rect.center().x - hint_galley.rect.width() / 2.0,
            rect.bottom() - 30.0 * scale,
        );
        ui.painter().galley(hint_pos, hint_galley, hint_color);
    }

    fn draw_slide_badge(
        &self,
        ui: &egui::Ui,
        cell_rect: egui::Rect,
        index: usize,
        scale: f32,
        opacity: f32,
    ) {
        if opacity < 0.01 {
            return;
        }
        let badge_bg = Theme::with_opacity(self.theme.code_background, 0.7 * opacity);
        let badge_text_color = Theme::with_opacity(self.theme.foreground, 0.9 * opacity);
        let badge_galley = ui.painter().layout_no_wrap(
            format!(" {} ", index + 1),
            egui::FontId::monospace(12.0 * scale),
            badge_text_color,
        );
        let badge_rect = egui::Rect::from_min_size(
            cell_rect.min + egui::vec2(4.0 * scale, 4.0 * scale),
            badge_galley.rect.size() + egui::vec2(4.0 * scale, 2.0 * scale),
        );
        ui.painter().rect_filled(badge_rect, 3.0 * scale, badge_bg);
        ui.painter().galley(
            badge_rect.min + egui::vec2(2.0 * scale, 1.0 * scale),
            badge_galley,
            badge_text_color,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_overview_transition(
        &self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        rect: egui::Rect,
        scale: f32,
        selected: usize,
        entering: bool,
    ) {
        let elapsed = self
            .overview_transition_start
            .map(|s| s.elapsed().as_secs_f32())
            .unwrap_or(0.0);
        let raw_t = (elapsed / OVERVIEW_TRANSITION_DURATION).clamp(0.0, 1.0);
        let t = ease_in_out(raw_t);

        // grid_amount: 0 = fullscreen presentation, 1 = grid view
        let grid_amount = if entering { t } else { 1.0 - t };

        let hero_index = if entering {
            self.current_slide
        } else {
            selected
        };
        let hero_cell_rect = self.grid_cell_rect(hero_index, rect, scale, 0.0);
        let hero_rect = lerp_rect(rect, hero_cell_rect, grid_amount);
        let hero_scale = (hero_rect.width() / 1920.0).min(hero_rect.height() / 1080.0);

        let count = self.slide_count();

        // Draw non-hero slides at their grid positions with fading opacity
        for i in 0..count {
            if i == hero_index {
                continue;
            }
            let cell_rect = self.grid_cell_rect(i, rect, scale, 0.0);
            let cell_scale = (cell_rect.width() / 1920.0).min(cell_rect.height() / 1080.0);

            ui.painter()
                .rect_filled(cell_rect, 4.0 * scale, self.theme.background);

            let child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(cell_rect)
                    .id_salt(("overview_cell", i)),
            );
            self.draw_slide(&child_ui, i, cell_rect, grid_amount, cell_scale);

            self.draw_slide_badge(ui, cell_rect, i, scale, grid_amount);

            if i == selected {
                let border_color = Theme::with_opacity(self.theme.accent, grid_amount);
                ui.painter().rect_stroke(
                    cell_rect,
                    4.0 * scale,
                    egui::Stroke::new(3.0 * scale, border_color),
                    egui::StrokeKind::Outside,
                );
            }
        }

        // Draw hero slide on top (interpolating from full-screen to grid cell)
        ui.painter()
            .rect_filled(hero_rect, 4.0 * scale * grid_amount, self.theme.background);

        let hero_child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(hero_rect)
                .id_salt("overview_hero"),
        );
        self.draw_slide(&hero_child_ui, hero_index, hero_rect, 1.0, hero_scale);

        self.draw_slide_badge(ui, hero_rect, hero_index, scale, grid_amount);

        if hero_index == selected {
            let border_color = Theme::with_opacity(self.theme.accent, grid_amount);
            ui.painter().rect_stroke(
                hero_rect,
                4.0 * scale * grid_amount,
                egui::Stroke::new(3.0 * scale, border_color),
                egui::StrokeKind::Outside,
            );
        }

        // Title and navigation hints fade in/out
        if grid_amount > 0.01 {
            let padding = 24.0 * scale;

            let title_color = Theme::with_opacity(self.theme.heading_color, 0.9 * grid_amount);
            let title_galley = ui.painter().layout_no_wrap(
                self.display_title(),
                egui::FontId::proportional(24.0 * scale),
                title_color,
            );
            let title_pos = egui::pos2(rect.left() + padding, rect.top() + padding);
            ui.painter().galley(title_pos, title_galley, title_color);

            let hint = "Arrows/Mouse: navigate  |  Enter/Click: select  |  Q: quit";
            let hint_color = Theme::with_opacity(self.theme.foreground, 0.4 * grid_amount);
            let hint_galley = ui.painter().layout_no_wrap(
                hint.to_string(),
                egui::FontId::proportional(14.0 * scale),
                hint_color,
            );
            let hint_pos = egui::pos2(
                rect.center().x - hint_galley.rect.width() / 2.0,
                rect.bottom() - 30.0 * scale,
            );
            ui.painter().galley(hint_pos, hint_galley, hint_color);
        }

        ctx.request_repaint();
    }

    /// Pen color: cyan/blue tones
    fn pen_color(&self, opacity: f32) -> egui::Color32 {
        if self.theme.name == "dark" {
            egui::Color32::from_rgba_unmultiplied(80, 200, 255, (opacity * 230.0) as u8)
        } else {
            egui::Color32::from_rgba_unmultiplied(30, 80, 200, (opacity * 230.0) as u8)
        }
    }

    /// Pen outline color: darker cyan/blue
    fn pen_outline_color(&self, opacity: f32) -> egui::Color32 {
        if self.theme.name == "dark" {
            egui::Color32::from_rgba_unmultiplied(30, 130, 180, (opacity * 140.0) as u8)
        } else {
            egui::Color32::from_rgba_unmultiplied(15, 40, 130, (opacity * 140.0) as u8)
        }
    }

    /// Arrow color: yellow-orange / red tones
    fn arrow_color(&self, opacity: f32) -> egui::Color32 {
        if self.theme.name == "dark" {
            egui::Color32::from_rgba_unmultiplied(255, 200, 50, (opacity * 230.0) as u8)
        } else {
            egui::Color32::from_rgba_unmultiplied(220, 40, 40, (opacity * 230.0) as u8)
        }
    }

    /// Arrow outline color: darker orange / red
    fn arrow_outline_color(&self, opacity: f32) -> egui::Color32 {
        if self.theme.name == "dark" {
            egui::Color32::from_rgba_unmultiplied(200, 140, 0, (opacity * 140.0) as u8)
        } else {
            egui::Color32::from_rgba_unmultiplied(150, 20, 20, (opacity * 140.0) as u8)
        }
    }

    /// Compute fade opacity for an annotation (1.0 for most of its life, fading in last 2s)
    fn annotation_opacity(start: Instant) -> f32 {
        let elapsed = start.elapsed().as_secs_f32();
        let fade_start = DRAW_FADE_DURATION - 2.0;
        if elapsed < fade_start {
            1.0
        } else if elapsed < DRAW_FADE_DURATION {
            1.0 - (elapsed - fade_start) / 2.0
        } else {
            0.0
        }
    }

    /// Draw all pen strokes and arrow annotations for the current slide
    fn draw_annotations(&self, ui: &egui::Ui, scale: f32) {
        let idx = self.current_slide;
        let pen_width = 6.0 * scale;
        let pen_outline_width = pen_width + 2.0 * scale;
        let arrow_width = 5.0 * scale;
        let arrow_outline_width = arrow_width + 2.0 * scale;
        let arrow_size = 22.0 * scale;
        let arrow_outline_size = arrow_size + 3.0 * scale;

        // Draw completed pen strokes
        for stroke in &self.pen_strokes {
            if stroke.slide_index != idx || stroke.points.len() < 2 {
                continue;
            }
            let opacity = Self::annotation_opacity(stroke.start);
            if opacity < 0.01 {
                continue;
            }
            let outline_color = self.pen_outline_color(opacity);
            let color = self.pen_color(opacity);
            let screen_points: Vec<egui::Pos2> = stroke
                .points
                .iter()
                .map(|p| self.local_to_screen(*p))
                .collect();
            // Outline pass
            ui.painter().add(egui::Shape::line(
                screen_points.clone(),
                egui::Stroke::new(pen_outline_width, outline_color),
            ));
            // Main pass
            ui.painter().add(egui::Shape::line(
                screen_points,
                egui::Stroke::new(pen_width, color),
            ));
        }

        // Draw completed arrows
        for arrow in &self.arrows {
            if arrow.slide_index != idx {
                continue;
            }
            let opacity = Self::annotation_opacity(arrow.start);
            if opacity < 0.01 {
                continue;
            }
            let outline_color = self.arrow_outline_color(opacity);
            let color = self.arrow_color(opacity);
            let from = self.local_to_screen(arrow.from);
            let to = self.local_to_screen(arrow.to);
            // Outline pass
            self.draw_arrow_shape(
                ui,
                from,
                to,
                arrow_outline_width,
                arrow_outline_size,
                outline_color,
            );
            // Main pass
            self.draw_arrow_shape(ui, from, to, arrow_width, arrow_size, color);
        }

        // Draw active drawing in progress
        match &self.active_draw {
            ActiveDraw::PenDrawing { points } if points.len() >= 2 => {
                let outline_color = self.pen_outline_color(1.0);
                let color = self.pen_color(1.0);
                let screen_points: Vec<egui::Pos2> =
                    points.iter().map(|p| self.local_to_screen(*p)).collect();
                ui.painter().add(egui::Shape::line(
                    screen_points.clone(),
                    egui::Stroke::new(pen_outline_width, outline_color),
                ));
                ui.painter().add(egui::Shape::line(
                    screen_points,
                    egui::Stroke::new(pen_width, color),
                ));
            }
            ActiveDraw::ArrowDrawing { from, current } => {
                let outline_color = self.arrow_outline_color(1.0);
                let color = self.arrow_color(1.0);
                let screen_from = self.local_to_screen(*from);
                let screen_to = self.local_to_screen(*current);
                self.draw_arrow_shape(
                    ui,
                    screen_from,
                    screen_to,
                    arrow_outline_width,
                    arrow_outline_size,
                    outline_color,
                );
                self.draw_arrow_shape(ui, screen_from, screen_to, arrow_width, arrow_size, color);
            }
            _ => {}
        }
    }

    /// Draw an arrow from `from` to `to` with a filled triangular arrowhead
    fn draw_arrow_shape(
        &self,
        ui: &egui::Ui,
        from: egui::Pos2,
        to: egui::Pos2,
        stroke_width: f32,
        arrow_size: f32,
        color: egui::Color32,
    ) {
        let delta = to - from;
        let len = delta.length();
        if len < 1.0 {
            return;
        }
        let dir = delta / len;
        let perp = egui::vec2(-dir.y, dir.x);

        // Arrowhead triangle points (wider spread)
        let p1 = to - dir * arrow_size + perp * arrow_size * 0.45;
        let p2 = to - dir * arrow_size - perp * arrow_size * 0.45;

        // Shaft (stop further back from head to avoid blunt overlap)
        ui.painter().line_segment(
            [from, to - dir * arrow_size * 0.7],
            egui::Stroke::new(stroke_width, color),
        );
        // Arrowhead
        ui.painter().add(egui::Shape::convex_polygon(
            vec![to, p1, p2],
            color,
            egui::Stroke::NONE,
        ));
    }
}

fn lerp_rect(a: egui::Rect, b: egui::Rect, t: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(
            a.min.x + (b.min.x - a.min.x) * t,
            a.min.y + (b.min.y - a.min.y) * t,
        ),
        egui::pos2(
            a.max.x + (b.max.x - a.max.x) * t,
            a.max.y + (b.max.y - a.max.y) * t,
        ),
    )
}

/// Draw a fade gradient at the top or bottom of a rect.
fn draw_fade_gradient(ui: &egui::Ui, rect: egui::Rect, fade_h: f32, theme: &Theme, top: bool) {
    let bg = theme.background;
    let transparent = egui::Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 0);
    let opaque = bg;

    let fade_rect = if top {
        egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.top()),
            egui::pos2(rect.right(), rect.top() + fade_h),
        )
    } else {
        egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - fade_h),
            egui::pos2(rect.right(), rect.bottom()),
        )
    };

    let mut mesh = egui::Mesh::default();
    // Four vertices: top-left, top-right, bottom-left, bottom-right
    let (top_color, bottom_color) = if top {
        (opaque, transparent)
    } else {
        (transparent, opaque)
    };

    mesh.colored_vertex(fade_rect.left_top(), top_color);
    mesh.colored_vertex(fade_rect.right_top(), top_color);
    mesh.colored_vertex(fade_rect.left_bottom(), bottom_color);
    mesh.colored_vertex(fade_rect.right_bottom(), bottom_color);
    // Two triangles: (0,1,2) and (1,3,2)
    mesh.add_triangle(0, 2, 1);
    mesh.add_triangle(1, 2, 3);

    ui.painter().add(egui::Shape::mesh(mesh));
}

fn draw_hud(ui: &egui::Ui, theme: &Theme, rect: egui::Rect, scale: f32) {
    let shortcuts = [
        ("Space / N / \u{2192}", "Next slide / reveal"),
        ("P / \u{2190}", "Previous slide / hide"),
        ("\u{2191} / \u{2193} / Wheel", "Scroll slide content"),
        ("Left click", "Next slide"),
        ("Right click", "Previous slide"),
        ("Left drag", "Freehand pen (blue)"),
        ("Right drag", "Draw arrow (orange)"),
        ("Esc", "Clear drawings / \u{00d7}2 exit"),
        ("G", "Grid view / overview"),
        ("T", "Cycle transition"),
        ("D", "Toggle theme"),
        ("F", "Toggle fullscreen"),
        ("H", "Toggle this HUD"),
        ("R", "Toggle raw markdown"),
        ("Q", "Quit"),
        ("Home", "First slide"),
        ("End", "Last slide"),
    ];

    let bg = Theme::with_opacity(theme.code_background, 0.9);
    let text_color = Theme::with_opacity(theme.foreground, 0.9);
    let key_color = Theme::with_opacity(theme.accent, 0.9);

    let padding = 24.0 * scale;
    let line_height = 32.0 * scale;
    let hud_height = shortcuts.len() as f32 * line_height + padding * 2.0 + 40.0 * scale;
    let hud_width = 360.0 * scale;

    let hud_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(hud_width, hud_height));

    ui.painter().rect_filled(hud_rect, 12.0 * scale, bg);

    // Title
    let title_galley = ui.painter().layout_no_wrap(
        "Keyboard Shortcuts".to_string(),
        egui::FontId::proportional(20.0 * scale),
        Theme::with_opacity(theme.heading_color, 0.9),
    );
    let title_pos = egui::pos2(hud_rect.left() + padding, hud_rect.top() + padding);
    ui.painter().galley(title_pos, title_galley, text_color);

    let mut y = hud_rect.top() + padding + 40.0 * scale;

    for (key, desc) in &shortcuts {
        let key_galley = ui.painter().layout_no_wrap(
            key.to_string(),
            egui::FontId::monospace(15.0 * scale),
            key_color,
        );
        ui.painter().galley(
            egui::pos2(hud_rect.left() + padding, y),
            key_galley,
            key_color,
        );

        let desc_galley = ui.painter().layout_no_wrap(
            desc.to_string(),
            egui::FontId::proportional(15.0 * scale),
            text_color,
        );
        ui.painter().galley(
            egui::pos2(hud_rect.left() + padding + 170.0 * scale, y),
            desc_galley,
            text_color,
        );

        y += line_height;
    }
}

fn draw_raw_markdown_overlay(
    ui: &egui::Ui,
    raw: &str,
    theme: &Theme,
    rect: egui::Rect,
    scale: f32,
) {
    let bg = Theme::with_opacity(theme.code_background, 0.92);
    let text_color = Theme::with_opacity(theme.code_foreground, 0.95);
    let title_color = Theme::with_opacity(theme.heading_color, 0.9);

    let padding = 32.0 * scale;
    let margin = 48.0 * scale;

    let overlay_rect = rect.shrink(margin);
    ui.painter().rect_filled(overlay_rect, 12.0 * scale, bg);

    // Title
    let title_galley = ui.painter().layout_no_wrap(
        "Raw Markdown".to_string(),
        egui::FontId::proportional(20.0 * scale),
        title_color,
    );
    let title_pos = egui::pos2(overlay_rect.left() + padding, overlay_rect.top() + padding);
    ui.painter().galley(title_pos, title_galley, title_color);

    // Dismiss hint
    let hint_color = Theme::with_opacity(theme.foreground, 0.5);
    let hint_galley = ui.painter().layout_no_wrap(
        "Press R to close".to_string(),
        egui::FontId::proportional(14.0 * scale),
        hint_color,
    );
    let hint_pos = egui::pos2(
        overlay_rect.right() - padding - hint_galley.rect.width(),
        overlay_rect.top() + padding + 3.0 * scale,
    );
    ui.painter().galley(hint_pos, hint_galley, hint_color);

    // Markdown content in monospace font
    let text_top = overlay_rect.top() + padding + 36.0 * scale;
    let text_width = overlay_rect.width() - padding * 2.0;
    let font = egui::FontId::monospace(14.0 * scale);

    let galley = ui
        .painter()
        .layout(raw.to_string(), font, text_color, text_width);
    let text_pos = egui::pos2(overlay_rect.left() + padding, text_top);
    ui.painter().galley(text_pos, galley, text_color);
}

fn load_app_icon() -> Option<egui::IconData> {
    let png_bytes = include_bytes!("../../../media/MDeck-logo.png");
    let image = image::load_from_memory(png_bytes).ok()?.into_rgba8();
    let (w, h) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width: w,
        height: h,
    })
}

pub fn run(
    file: PathBuf,
    windowed: bool,
    start_slide: Option<usize>,
    start_overview: bool,
) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(&file)?;
    let base_path = file.parent().unwrap_or(std::path::Path::new("."));
    let presentation = parser::parse(&content, base_path);

    if presentation.slides.is_empty() {
        anyhow::bail!("No slides found in {}", file.display());
    }

    let title = presentation.meta.title.clone().unwrap_or_else(|| {
        format!(
            "mdeck \u{2014} {}",
            file.file_name().unwrap_or_default().to_string_lossy()
        )
    });

    let slide_count = presentation.slides.len();

    // Determine start mode: CLI flags override config
    let config = Config::load_or_default();
    let config_start = config
        .defaults
        .as_ref()
        .and_then(|d| d.start_mode.as_deref());

    let (initial_slide, initial_overview) = if start_overview {
        // --overview flag: start in grid at current slide
        (start_slide.map(|s| s.saturating_sub(1)).unwrap_or(0), true)
    } else if let Some(s) = start_slide {
        // --slide N flag: start on that slide (1-indexed)
        (s.saturating_sub(1), false)
    } else {
        // Fall back to config
        match config_start {
            Some("overview") => (0, true),
            Some("first") | None => (0, false),
            Some(n) => {
                if let Ok(num) = n.parse::<usize>() {
                    (num.saturating_sub(1), false)
                } else {
                    (0, false)
                }
            }
        }
    };

    let initial_slide = initial_slide.min(slide_count.saturating_sub(1));

    let viewport = if windowed {
        egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title(&title)
    } else {
        egui::ViewportBuilder::default()
            .with_fullscreen(true)
            .with_title(&title)
    };

    let viewport = if let Some(icon) = load_app_icon() {
        viewport.with_icon(std::sync::Arc::new(icon))
    } else {
        viewport
    };

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        &title,
        options,
        Box::new(move |_cc| {
            let mut app = PresentationApp::new(file, presentation, windowed);
            app.current_slide = initial_slide;
            if initial_overview {
                app.mode = AppMode::Grid {
                    selected: initial_slide,
                };
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
