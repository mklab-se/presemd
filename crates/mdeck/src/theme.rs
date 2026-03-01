use eframe::egui::Color32;

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub background: Color32,
    pub foreground: Color32,
    pub heading_color: Color32,
    pub accent: Color32,
    pub code_background: Color32,
    pub code_foreground: Color32,
    pub h1_size: f32,
    pub h2_size: f32,
    pub h3_size: f32,
    pub body_size: f32,
    pub code_size: f32,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            name: "dark".to_string(),
            background: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            foreground: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            heading_color: Color32::WHITE,
            accent: Color32::from_rgb(0x52, 0x94, 0xE2),
            code_background: Color32::from_rgb(0x2D, 0x2D, 0x2D),
            code_foreground: Color32::from_rgb(0xD4, 0xD4, 0xD4),
            h1_size: 96.0,
            h2_size: 72.0,
            h3_size: 52.0,
            body_size: 44.0,
            code_size: 30.0,
        }
    }

    pub fn light() -> Self {
        Self {
            name: "light".to_string(),
            background: Color32::WHITE,
            foreground: Color32::from_rgb(0x1A, 0x1A, 0x2E),
            heading_color: Color32::from_rgb(0x16, 0x21, 0x3E),
            accent: Color32::from_rgb(0x0F, 0x34, 0x60),
            code_background: Color32::from_rgb(0xF5, 0xF5, 0xF5),
            code_foreground: Color32::from_rgb(0x33, 0x33, 0x33),
            h1_size: 96.0,
            h2_size: 72.0,
            h3_size: 52.0,
            body_size: 44.0,
            code_size: 30.0,
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "dark" => Self::dark(),
            _ => Self::light(),
        }
    }

    pub fn toggled(&self) -> Self {
        if self.name == "dark" {
            Self::light()
        } else {
            Self::dark()
        }
    }

    /// Apply opacity to a color
    pub fn with_opacity(color: Color32, opacity: f32) -> Color32 {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (opacity * 255.0) as u8)
    }

    pub fn heading_size(&self, level: u8) -> f32 {
        match level {
            1 => self.h1_size,
            2 => self.h2_size,
            3 => self.h3_size,
            _ => self.body_size,
        }
    }

    /// Return the syntect theme name that matches this presentation theme.
    pub fn syntect_theme_name(&self) -> &str {
        if self.name == "dark" {
            "base16-ocean.dark"
        } else {
            "InspiredGitHub"
        }
    }

    /// Return a palette of distinct colors for diagram edges.
    /// Colors are chosen to be visually distinct and readable against the theme background.
    pub fn edge_palette(&self) -> Vec<Color32> {
        if self.name == "dark" {
            vec![
                Color32::from_rgb(0x5C, 0xB8, 0xFF), // bright blue
                Color32::from_rgb(0xFF, 0x7E, 0x67), // coral
                Color32::from_rgb(0x5C, 0xDB, 0x95), // mint green
                Color32::from_rgb(0xE8, 0xA8, 0x38), // amber
                Color32::from_rgb(0xC0, 0x7E, 0xF1), // purple
                Color32::from_rgb(0x4E, 0xD4, 0xD4), // teal
                Color32::from_rgb(0xF0, 0x6E, 0xAA), // pink
                Color32::from_rgb(0xA3, 0xBE, 0x58), // olive green
            ]
        } else {
            vec![
                Color32::from_rgb(0x1A, 0x6B, 0xB5), // deep blue
                Color32::from_rgb(0xC7, 0x3E, 0x1D), // brick red
                Color32::from_rgb(0x1E, 0x8A, 0x5A), // forest green
                Color32::from_rgb(0xB8, 0x7B, 0x0A), // dark amber
                Color32::from_rgb(0x7B, 0x3F, 0xA0), // purple
                Color32::from_rgb(0x18, 0x8A, 0x8D), // teal
                Color32::from_rgb(0xC4, 0x3B, 0x7A), // magenta
                Color32::from_rgb(0x5A, 0x7A, 0x2B), // olive
            ]
        }
    }
}
