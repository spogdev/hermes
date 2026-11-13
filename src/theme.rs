//! The one design every screen shares (see `app-design.md`). Change looks here, not per widget.

use eframe::egui::{Color32, Context, CornerRadius, Id, Painter, Pos2, Rect, Shadow, Stroke, StrokeKind, Vec2, pos2, vec2};

// Typical dark-mode palette (iOS dark system colours) until the owner supplies one.
pub const BG: Color32 = Color32::from_rgb(0x1C, 0x1C, 0x1E);
pub const PANEL: Color32 = Color32::from_rgb(0x2C, 0x2C, 0x2E);
pub const OUTLINE: Color32 = Color32::from_rgb(0x4C, 0x4C, 0x50);
pub const TEXT: Color32 = Color32::from_rgb(0xF2, 0xF2, 0xF7);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8E, 0x8E, 0x93);
pub const PLAY: Color32 = Color32::from_rgb(0x30, 0xD1, 0x58);
pub const PLAY_TEXT: Color32 = Color32::from_rgb(0x0B, 0x24, 0x12);
pub const DANGER: Color32 = Color32::from_rgb(0xFF, 0x45, 0x3A);
/// Text on dark surfaces in the error state (6:1 on `PANEL`; `DANGER` itself is only 4:1).
pub const DANGER_TEXT: Color32 = Color32::from_rgb(0xFF, 0x8A, 0x84);
/// Keyboard focus ring.
pub const FOCUS: Color32 = Color32::from_rgb(0x0A, 0x84, 0xFF);
/// Placeholder for the selected account's head until skins are fetched.
pub const HEAD_PLACEHOLDER: Color32 = Color32::from_rgb(0x64, 0xD2, 0xFF);

// Liquid-glass approximation: white tints over the panel colour.
pub const GLASS_TINT: Color32 = Color32::from_rgba_premultiplied(14, 14, 14, 14);
pub const GLASS_EDGE: Color32 = Color32::from_rgba_premultiplied(80, 80, 80, 80);
pub const GLASS_HOVER: Color32 = Color32::from_rgba_premultiplied(22, 22, 22, 22);
/// Pressed: a slightly stronger tint than hover.
pub const GLASS_PRESS: Color32 = Color32::from_rgba_premultiplied(34, 34, 34, 34);

pub const STROKE_WIDTH: f32 = 2.0;
/// Line weight of every drawn icon (window buttons, dropdown arrow/check).
pub const ICON_STROKE: f32 = 2.0;
pub const WINDOW_RADIUS: u8 = 14;
/// Large surfaces: sidebar, main content panel.
pub const PANEL_RADIUS: u8 = 12;
/// Every control (Play button, dropdowns) shares this shape.
pub const CONTROL_RADIUS: u8 = 10;
/// Small controls (window buttons) and inner highlights.
pub const SMALL_RADIUS: u8 = 7;
pub const BAR_HEIGHT: f32 = 36.0;
/// Width of the main window (the template frame), not counting the sidebar overhang.
pub const WINDOW_WIDTH: f32 = 1200.0;
/// How far the sidebar sticks out past the main window's top, bottom and left edges.
pub const SIDEBAR_POP: f32 = 8.0;
/// Fully transparent border around the painted outline. The window clip
/// (`window_shape`) sits here, so its jagged per-pixel edge never touches a
/// painted pixel and the anti-aliased outline blends smoothly with the desktop.
pub const EDGE_MARGIN: f32 = 2.0;

/// The owner's v1 template frame size; layout rects in `app-design.md` use these units.
const TEMPLATE: Vec2 = vec2(1340.0, 749.0);

fn main_size() -> Vec2 {
    vec2(WINDOW_WIDTH, BAR_HEIGHT + TEMPLATE.y * WINDOW_WIDTH / TEMPLATE.x)
}

/// Fixed native window size: the main window plus the sidebar overhang.
pub fn window_size() -> Vec2 {
    main_size() + vec2(SIDEBAR_POP, 2.0 * SIDEBAR_POP) + Vec2::splat(2.0 * EDGE_MARGIN)
}

/// Splits the native window area into the main window and the sidebar, which
/// overhangs the main window on the top, bottom and left.
pub fn frame_layout(area: Rect) -> (Rect, Rect) {
    let area = area.shrink(EDGE_MARGIN);
    let main = Rect::from_min_max(area.min + Vec2::splat(SIDEBAR_POP), area.max - vec2(0.0, SIDEBAR_POP));
    let sidebar_right = place(main, 66.0, 0.0, 0.0, 0.0).min.x;
    (main, Rect::from_min_max(area.min, pos2(sidebar_right, area.max.y)))
}

/// Maps a rect given in template units onto `body` (the window area below the bar).
pub fn place(body: Rect, x: f32, y: f32, w: f32, h: f32) -> Rect {
    let s = body.width() / TEMPLATE.x;
    Rect::from_min_size(pos2(body.min.x + x * s, body.min.y + y * s), vec2(w * s, h * s))
}

pub fn radius(r: u8) -> CornerRadius {
    CornerRadius::same(r)
}

/// How far a pressed control shrinks (fraction of its size).
const PRESS_DEPTH: f32 = 0.06;
/// Press spring: stiff enough to feel immediate, under-damped so the release
/// overshoots slightly past full size before settling ("shrink, then return a little").
const PRESS_STIFFNESS: f32 = 600.0;
const PRESS_DAMPING: f32 = 22.0;

#[derive(Clone, Copy, Default)]
struct PressSpring {
    /// Scale minus 1.
    offset: f32,
    velocity: f32,
}

/// Scale (around 1.0) of a control that shrinks while `pressed` and springs back
/// when released. Every pressable control uses this so they all move the same way.
pub fn press_scale(ctx: &Context, id: Id, pressed: bool) -> f32 {
    let id = id.with("press_spring");
    let mut spring = ctx.data(|d| d.get_temp::<PressSpring>(id)).unwrap_or_default();
    let target = if pressed { -PRESS_DEPTH } else { 0.0 };
    if spring.offset == target && spring.velocity == 0.0 {
        return 1.0 + target;
    }
    spring = spring_step(spring, target, ctx.input(|i| i.stable_dt).min(1.0 / 20.0));
    if (spring.offset - target).abs() < 1e-4 && spring.velocity.abs() < 1e-3 {
        spring = PressSpring { offset: target, velocity: 0.0 };
    } else {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id, spring));
    1.0 + spring.offset
}

/// Advances the spring by `elapsed` seconds in fixed small steps, so it stays
/// stable at any frame rate.
fn spring_step(mut spring: PressSpring, target: f32, mut elapsed: f32) -> PressSpring {
    while elapsed > 0.0 {
        let dt = elapsed.min(1.0 / 240.0);
        let accel = -PRESS_STIFFNESS * (spring.offset - target) - PRESS_DAMPING * spring.velocity;
        spring.velocity += accel * dt;
        spring.offset += spring.velocity * dt;
        elapsed -= dt;
    }
    spring
}

/// `rect` scaled by `scale` around `center` (used to draw pressed controls).
pub fn scale_about(rect: Rect, center: Pos2, scale: f32) -> Rect {
    Rect::from_min_max(center + (rect.min - center) * scale, center + (rect.max - center) * scale)
}

/// Keyboard focus ring drawn just outside a control of the given radius.
pub fn focus_ring(painter: &Painter, rect: Rect, r: u8) {
    painter.rect_stroke(rect.expand(3.0), radius(r.saturating_add(3)), Stroke::new(2.0, FOCUS), StrokeKind::Outside);
}

/// Styles egui's own surfaces (tooltips, popups) with the app palette.
pub fn apply_style(ctx: &Context) {
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        v.panel_fill = BG;
        v.window_fill = PANEL;
        v.window_stroke = Stroke::new(1.0, OUTLINE);
        v.window_corner_radius = radius(CONTROL_RADIUS);
        v.menu_corner_radius = radius(CONTROL_RADIUS);
        v.popup_shadow = Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(90) };
        v.override_text_color = Some(TEXT);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_shrinks_then_springs_back_past_full_size_and_settles() {
        // Held: settles at the pressed size.
        let held = spring_step(PressSpring::default(), -PRESS_DEPTH, 0.5);
        assert!((held.offset + PRESS_DEPTH).abs() < 1e-3);
        // Released: overshoots slightly above full size, then settles at it.
        let mut s = held;
        let mut peak = f32::MIN;
        for _ in 0..60 {
            s = spring_step(s, 0.0, 1.0 / 120.0);
            peak = peak.max(s.offset);
        }
        assert!(peak > 0.002 && peak < 0.02, "overshoot {peak}");
        assert!(spring_step(s, 0.0, 1.0).offset.abs() < 1e-3);
        // A quick click (80 ms) still visibly shrinks.
        let quick = spring_step(PressSpring::default(), -PRESS_DEPTH, 0.08);
        assert!(quick.offset < -PRESS_DEPTH * 0.5, "{}", quick.offset);
    }

    #[test]
    fn scale_about_keeps_center() {
        let r = Rect::from_min_size(pos2(10.0, 10.0), vec2(100.0, 40.0));
        let s = scale_about(r, r.center(), 0.9);
        assert_eq!(s.center(), r.center());
        assert!((s.width() - 90.0).abs() < 1e-4);
    }

    #[test]
    fn template_scales_to_body() {
        let body = Rect::from_min_size(pos2(0.0, BAR_HEIGHT), vec2(WINDOW_WIDTH, main_size().y - BAR_HEIGHT));
        let full = place(body, 0.0, 0.0, TEMPLATE.x, TEMPLATE.y);
        assert!((full.max.x - body.max.x).abs() < 0.01);
        assert!((full.max.y - body.max.y).abs() < 0.01);
    }

    #[test]
    fn sidebar_overhangs_main_window() {
        let area = Rect::from_min_size(pos2(0.0, 0.0), window_size());
        let (main, sidebar) = frame_layout(area);
        assert_eq!(main.size(), main_size());
        assert_eq!(sidebar.min, area.min + Vec2::splat(EDGE_MARGIN));
        assert_eq!(sidebar.max.y, area.max.y - EDGE_MARGIN);
        assert!(sidebar.min.y < main.min.y && sidebar.max.y > main.max.y && sidebar.min.x < main.min.x);
        assert!(sidebar.max.x > main.min.x);
    }
}
