use eframe::egui;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ImageCache {
    base_path: PathBuf,
    textures: RefCell<HashMap<String, Option<egui::TextureHandle>>>,
}

impl ImageCache {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            textures: RefCell::new(HashMap::new()),
        }
    }

    /// Clear all cached textures so images reload on next access.
    pub fn clear(&mut self) {
        self.textures.get_mut().clear();
    }

    /// Get a texture by image path, loading lazily on first access.
    pub fn get_or_load(&self, ui: &egui::Ui, path: &str) -> Option<egui::TextureHandle> {
        let mut cache = self.textures.borrow_mut();

        if let Some(entry) = cache.get(path) {
            return entry.clone();
        }

        // Resolve relative paths against base_path
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.base_path.join(path)
        };

        let texture = load_texture(ui, &full_path, path);
        cache.insert(path.to_string(), texture.clone());
        texture
    }
}

fn load_texture(ui: &egui::Ui, path: &Path, name: &str) -> Option<egui::TextureHandle> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
    let pixels = rgba.into_raw();

    let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], &pixels);
    let texture = ui
        .ctx()
        .load_texture(name, color_image, egui::TextureOptions::LINEAR);
    Some(texture)
}
