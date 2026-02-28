use eframe::egui;
use std::time::Instant;

const SLIDE_COUNT: usize = 3;
const TRANSITION_DURATION: f32 = 0.3;

struct Slide {
    heading: &'static str,
    body: &'static str,
}

const SLIDES: [Slide; SLIDE_COUNT] = [
    Slide {
        heading: "Welcome to Presemd",
        body: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod \
               tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, \
               quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.\n\n\
               Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu \
               fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in \
               culpa qui officia deserunt mollit anim id est laborum.\n\n\
               Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, \
               turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis \
               sollicitudin mauris.",
    },
    Slide {
        heading: "The Architecture",
        body: "Pellentesque habitant morbi tristique senectus et netus et malesuada fames ac \
               turpis egestas. Vestibulum tortor quam, feugiat vitae, ultricies eget, tempor \
               sit amet, ante. Donec eu libero sit amet quam egestas semper.\n\n\
               Aenean ultricies mi vitae est. Mauris placerat eleifend leo. Quisque sit amet \
               est et sapien ullamcorper pharetra. Vestibulum erat wisi, condimentum sed, \
               commodo vitae, ornare sit amet, wisi.\n\n\
               Morbi in sem quis dui placerat ornare. Pellentesque odio nisi, euismod in, \
               pharetra a, ultricies in, diam. Sed arcu. Cras consequat.",
    },
    Slide {
        heading: "What Comes Next",
        body: "Praesent dapibus, neque id cursus faucibus, tortor neque egestas augue, eu \
               vulputate magna eros eu erat. Aliquam erat volutpat. Nam dui mi, tincidunt \
               quis, accumsan porttitor, facilisis luctus, metus.\n\n\
               Phasellus ultrices nulla quis nibh. Quisque a lectus. Donec consectetuer ligula \
               vulputate sem tristique cursus. Nam nulla quam, gravida non, commodo a, sodales \
               sit amet, nisi.\n\n\
               Etiam vel neque nec dui dignissim bibendum. Vivamus id enim. Phasellus neque \
               orci, porta a, aliquet quis, semper a, massa. Nullam tristique diam non turpis. \
               Cras placerat accumsan nulla.",
    },
];

#[derive(Clone, Copy, PartialEq)]
enum TransitionKind {
    Fade,
    SlideHorizontal,
}

#[derive(Clone, Copy, PartialEq)]
enum TransitionDirection {
    Forward,
    Backward,
}

struct Transition {
    from: usize,
    to: usize,
    kind: TransitionKind,
    direction: TransitionDirection,
    start: Instant,
}

struct PresentationApp {
    current_slide: usize,
    transition: Option<Transition>,
    last_frame: Instant,
    frame_count: u32,
    fps: f32,
    fps_update: Instant,
}

impl PresentationApp {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            current_slide: 0,
            transition: None,
            last_frame: now,
            frame_count: 0,
            fps: 0.0,
            fps_update: now,
        }
    }

    fn navigate(&mut self, direction: TransitionDirection) {
        if self.transition.is_some() {
            return;
        }

        let (from, to) = match direction {
            TransitionDirection::Forward => {
                if self.current_slide >= SLIDE_COUNT - 1 {
                    return;
                }
                (self.current_slide, self.current_slide + 1)
            }
            TransitionDirection::Backward => {
                if self.current_slide == 0 {
                    return;
                }
                (self.current_slide, self.current_slide - 1)
            }
        };

        let kind = transition_kind_for(from, to);

        self.transition = Some(Transition {
            from,
            to,
            kind,
            direction,
            start: Instant::now(),
        });
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
}

fn transition_kind_for(from: usize, to: usize) -> TransitionKind {
    let lo = from.min(to);
    let hi = from.max(to);
    if lo == 0 && hi == 1 {
        TransitionKind::Fade
    } else {
        TransitionKind::SlideHorizontal
    }
}

fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

fn draw_slide(ui: &mut egui::Ui, index: usize, rect: egui::Rect, opacity: f32) {
    let slide = &SLIDES[index];
    let heading_color =
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, (opacity * 255.0) as u8);
    let body_color = egui::Color32::from_rgba_unmultiplied(200, 200, 200, (opacity * 255.0) as u8);

    let padding = 60.0;
    let content_rect = rect.shrink(padding);

    let heading_galley = ui.painter().layout(
        slide.heading.to_string(),
        egui::FontId::proportional(52.0),
        heading_color,
        content_rect.width(),
    );
    let heading_height = heading_galley.rect.height();
    let heading_pos = content_rect.left_top() + egui::vec2(0.0, 40.0);
    ui.painter()
        .galley(heading_pos, heading_galley, heading_color);

    let body_top = heading_pos.y + heading_height + 30.0;
    let body_galley = ui.painter().layout(
        slide.body.to_string(),
        egui::FontId::proportional(22.0),
        body_color,
        content_rect.width(),
    );
    let body_pos = egui::pos2(content_rect.left(), body_top);
    ui.painter().galley(body_pos, body_galley, body_color);
}

impl eframe::App for PresentationApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_fps();

        ctx.input(|i| {
            if i.key_pressed(egui::Key::ArrowRight) {
                self.navigate(TransitionDirection::Forward);
            }
            if i.key_pressed(egui::Key::ArrowLeft) {
                self.navigate(TransitionDirection::Backward);
            }
        });

        // Advance transition
        let mut finished_to: Option<usize> = None;
        if let Some(ref t) = self.transition {
            let elapsed = t.start.elapsed().as_secs_f32();
            if elapsed >= TRANSITION_DURATION {
                finished_to = Some(t.to);
            }
        }
        if let Some(to) = finished_to {
            self.current_slide = to;
            self.transition = None;
        }

        let bg = egui::Color32::from_rgb(30, 30, 30);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg).inner_margin(0.0))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 0.0, bg);

                if let Some(ref t) = self.transition {
                    let raw_t =
                        (t.start.elapsed().as_secs_f32() / TRANSITION_DURATION).clamp(0.0, 1.0);
                    let progress = ease_in_out(raw_t);

                    match t.kind {
                        TransitionKind::Fade => {
                            draw_slide(ui, t.from, rect, 1.0 - progress);
                            draw_slide(ui, t.to, rect, progress);
                        }
                        TransitionKind::SlideHorizontal => {
                            let w = rect.width();
                            let sign = match t.direction {
                                TransitionDirection::Forward => -1.0,
                                TransitionDirection::Backward => 1.0,
                            };
                            let from_offset = sign * progress * w;
                            let to_offset = from_offset - sign * w;

                            let from_rect = rect.translate(egui::vec2(from_offset, 0.0));
                            let to_rect = rect.translate(egui::vec2(to_offset, 0.0));

                            draw_slide(ui, t.from, from_rect, 1.0);
                            draw_slide(ui, t.to, to_rect, 1.0);
                        }
                    }
                    ctx.request_repaint();
                } else {
                    draw_slide(ui, self.current_slide, rect, 1.0);
                }

                // FPS overlay
                let fps_text = format!("{:.0} fps", self.fps);
                let fps_galley = ui.painter().layout_no_wrap(
                    fps_text,
                    egui::FontId::monospace(14.0),
                    egui::Color32::from_rgba_unmultiplied(180, 180, 180, 160),
                );
                let fps_pos = egui::pos2(
                    rect.right() - fps_galley.rect.width() - 12.0,
                    rect.top() + 10.0,
                );
                ui.painter()
                    .galley(fps_pos, fps_galley, egui::Color32::TRANSPARENT);
            });

        self.last_frame = Instant::now();
    }
}

fn main() -> eframe::Result {
    // Support --version for packaging (Homebrew, cargo-binstall)
    if std::env::args().any(|a| a == "--version") {
        println!("presemd {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Presemd"),
        ..Default::default()
    };

    eframe::run_native(
        "Presemd",
        options,
        Box::new(|_cc| Ok(Box::new(PresentationApp::new()))),
    )
}
