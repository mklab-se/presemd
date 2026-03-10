//! Prompt construction helpers for AI image generation.

pub const DEFAULT_IMAGE_STYLE: &str = "Modern, clean, and visually striking. \
    Professional color palette with subtle gradients. Polished and contemporary, \
    suitable for business or technical presentations. No text overlays, watermarks, \
    or cluttered compositions. Clear visual communication with elegant simplicity.";

pub const DEFAULT_ICON_STYLE: &str = "Clean, modern icon illustration. \
    Minimalist design with subtle 3D feel and soft lighting. Recognizable at small \
    sizes with clear silhouette and balanced proportions. No text, no background clutter.";

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum Orientation {
    Horizontal,
    Vertical,
    Square,
}

/// Build a prompt for a presentation image by combining style + user prompt + orientation hint.
pub fn build_image_prompt(style: &str, user_prompt: &str, orientation: Orientation) -> String {
    let orientation_hint = match orientation {
        Orientation::Horizontal => "Horizontal format with wide aspect ratio.",
        Orientation::Vertical => "Vertical/portrait format with tall aspect ratio.",
        Orientation::Square => "Square format.",
    };

    format!(
        "Style: {style}\n\n\
         {user_prompt}\n\n\
         {orientation_hint} \
         The image should work well on both light and dark backgrounds."
    )
}

/// Build a prompt for a diagram icon by combining style + user prompt.
pub fn build_icon_prompt(style: &str, user_prompt: &str) -> String {
    format!(
        "Style: {style}\n\n\
         {user_prompt}\n\n\
         Square format, transparent background, suitable as a diagram icon. \
         The icon should work well on both light and dark backgrounds."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_image_prompt() {
        let prompt = build_image_prompt(
            "Modern style",
            "A sunset over mountains",
            Orientation::Horizontal,
        );
        assert!(prompt.contains("Modern style"));
        assert!(prompt.contains("A sunset over mountains"));
        assert!(prompt.contains("Horizontal"));
    }

    #[test]
    fn test_build_icon_prompt() {
        let prompt = build_icon_prompt("Clean icon", "A database");
        assert!(prompt.contains("Clean icon"));
        assert!(prompt.contains("A database"));
        assert!(prompt.contains("transparent background"));
    }
}
