//! The one design every screen shares (see `app-design.md`). Change looks here, not per widget.

use eframe::egui::{Color32, CornerRadius, Rect, Vec2, pos2, vec2};

// Typical dark-mode palette (iOS dark system colours) until the owner supplies one.
pub const BG: Color32 = Color32::from_rgb(0x1C, 0x1C, 0x1E);
pub const PANEL: Color32 = Color32::from_rgb(0x2C, 0x2C, 0x2E);
pub const OUTLINE: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x3C);
pub const TEXT: Color32 = Color32::from_rgb(0xF2, 0xF2, 0xF7);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8E, 0x8E, 0x93);
pub const PLAY: Color32 = Color32::from_rgb(0x30, 0xD1, 0x58);
pub const PLAY_TEXT: Color32 = Color32::from_rgb(0x0B, 0x24, 0x12);
pub const DANGER: Color32 = Color32::from_rgb(0xFF, 0x45, 0x3A);
/// Placeholder for the selected account's head until skins are fetched.
pub const HEAD_PLACEHOLDER: Color32 = Color32::from_rgb(0x64, 0xD2, 0xFF);

// Liquid-glass approximation: white tints over the panel colour.
pub const GLASS_TINT: Color32 = Color32::from_rgba_premultiplied(14, 14, 14, 14);
pub const GLASS_EDGE: Color32 = Color32::from_rgba_premultiplied(56, 56, 56, 56);
pub const GLASS_HOVER: Color32 = Color32::from_rgba_premultiplied(22, 22, 22, 22);

pub const STROKE_WIDTH: f32 = 1.5;
pub const WINDOW_RADIUS: u8 = 30;
pub const PANEL_RADIUS: u8 = 16;
pub const BAR_HEIGHT: f32 = 36.0;
pub const WINDOW_WIDTH: f32 = 1000.0;

/// The owner's v1 template frame size; layout rects in `app-design.md` use these units.
const TEMPLATE: Vec2 = vec2(1340.0, 749.0);

/// Fixed window size: the custom bar plus the template scaled to `WINDOW_WIDTH`.
pub fn window_size() -> Vec2 {
    vec2(WINDOW_WIDTH, BAR_HEIGHT + TEMPLATE.y * WINDOW_WIDTH / TEMPLATE.x)
}

/// Maps a rect given in template units onto `body` (the window area below the bar).
pub fn place(body: Rect, x: f32, y: f32, w: f32, h: f32) -> Rect {
    let s = body.width() / TEMPLATE.x;
    Rect::from_min_size(pos2(body.min.x + x * s, body.min.y + y * s), vec2(w * s, h * s))
}

pub fn radius(r: u8) -> CornerRadius {
    CornerRadius::same(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_scales_to_body() {
        let body = Rect::from_min_size(pos2(0.0, BAR_HEIGHT), vec2(WINDOW_WIDTH, window_size().y - BAR_HEIGHT));
        let full = place(body, 0.0, 0.0, TEMPLATE.x, TEMPLATE.y);
        assert!((full.max.x - body.max.x).abs() < 0.01);
        assert!((full.max.y - body.max.y).abs() < 0.01);
    }
}
