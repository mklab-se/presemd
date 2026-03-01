use std::path::{Path, PathBuf};

use eframe::egui;

use crate::parser::{self, Presentation};
use crate::render;
use crate::render::image_cache::ImageCache;
use crate::theme::Theme;

struct ExportApp {
    presentation: Presentation,
    theme: Theme,
    image_cache: ImageCache,
    output_dir: PathBuf,
    current_slide: usize,
    screenshot_requested: bool,
    max_steps: Vec<usize>,
    done: bool,
}

impl ExportApp {
    fn new(presentation: Presentation, base_path: &Path, output_dir: PathBuf) -> Self {
        let theme_name = presentation.meta.theme.as_deref().unwrap_or("light");
        let theme = Theme::from_name(theme_name);
        let image_cache = ImageCache::new(base_path.to_path_buf());
        let max_steps: Vec<usize> = presentation
            .slides
            .iter()
            .map(|s| parser::compute_max_steps(&s.blocks))
            .collect();

        Self {
            presentation,
            theme,
            image_cache,
            output_dir,
            current_slide: 0,
            screenshot_requested: false,
            max_steps,
            done: false,
        }
    }

    fn slide_count(&self) -> usize {
        self.presentation.slides.len()
    }
}

impl eframe::App for ExportApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.done {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Check for screenshot result from previous frame
        let mut got_screenshot = false;
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    let filename = format!("slide-{:02}.png", self.current_slide + 1);
                    let path = self.output_dir.join(&filename);
                    save_color_image(image, &path);
                    eprintln!("  Saved {filename}");
                    got_screenshot = true;
                }
            }
        });

        if got_screenshot {
            self.screenshot_requested = false;
            self.current_slide += 1;
            if self.current_slide >= self.slide_count() {
                self.done = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }

        let bg = self.theme.background;

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg).inner_margin(0.0))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 0.0, bg);

                let scale = {
                    let ref_w = 1920.0;
                    let ref_h = 1080.0;
                    (rect.width() / ref_w).min(rect.height() / ref_h)
                };

                let idx = self.current_slide;
                if idx < self.presentation.slides.len() {
                    let reveal = self.max_steps.get(idx).copied().unwrap_or(0);
                    render::render_slide(
                        ui,
                        &self.presentation.slides[idx],
                        &self.theme,
                        rect,
                        1.0,
                        &self.image_cache,
                        reveal,
                        None, // no animation in export
                        scale,
                    );
                }
            });

        // Request screenshot after rendering (will arrive next frame)
        if !self.screenshot_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.screenshot_requested = true;
        }

        ctx.request_repaint();
    }
}

fn save_color_image(image: &egui::ColorImage, path: &Path) {
    let width = image.width() as u32;
    let height = image.height() as u32;
    let pixels: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|c| [c.r(), c.g(), c.b(), c.a()])
        .collect();

    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)
        .unwrap_or_else(|e| eprintln!("Failed to save {}: {e}", path.display()));
}

pub fn run(file: PathBuf, output_dir: PathBuf, width: u32, height: u32) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(&file)?;
    let base_path = file
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    let presentation = parser::parse(&content, &base_path);

    if presentation.slides.is_empty() {
        anyhow::bail!("No slides found in {}", file.display());
    }

    std::fs::create_dir_all(&output_dir)?;

    let slide_count = presentation.slides.len();
    eprintln!(
        "Exporting {} slides to {} ({}x{})",
        slide_count,
        output_dir.display(),
        width,
        height,
    );

    let title = presentation
        .meta
        .title
        .clone()
        .unwrap_or_else(|| "mdeck export".to_string());

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([width as f32, height as f32])
        .with_title(&title)
        .with_decorations(false);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let output_dir_clone = output_dir.clone();
    eframe::run_native(
        &title,
        options,
        Box::new(move |_cc| {
            Ok(Box::new(ExportApp::new(
                presentation,
                &base_path,
                output_dir_clone,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    eprintln!("Export complete.");
    Ok(())
}
