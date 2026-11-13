//! The one design every screen shares (see `app-design.md`). Change looks here, not per widget.

use eframe::egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Id, Painter, Pos2, Rect, Shadow, Stroke,
    StrokeKind, Vec2, pos2, vec2,
};
use eframe::epaint::{CornerRadiusF32, PathShape, PathStroke, RoundedRect, tessellator::path};

// Two palettes with the same names. Dark is the look; light was a temporary
// trial (owner request) and is kept for later.
pub use dark::*;

/// iOS light system colours.
#[allow(dead_code)]
mod light {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0xF2, 0xF2, 0xF7);
    pub const PANEL: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const OUTLINE: Color32 = Color32::from_rgb(0xC6, 0xC6, 0xC8);
    pub const TEXT: Color32 = Color32::from_rgb(0x1C, 0x1C, 0x1E);
    /// Play button: light blue (owner request). Outlines run from this blue to `SHINE`.
    pub const PLAY: Color32 = Color32::from_rgb(0x5A, 0xC8, 0xFA);
    pub const PLAY_TEXT: Color32 = Color32::from_rgb(0x06, 0x22, 0x36);
    /// Light end of the diamond outline gradient; a touch deeper than on dark so it
    /// still shows against white.
    pub const SHINE: Color32 = Color32::from_rgb(0xB4, 0xE6, 0xFC);
    /// Stop button (shown while the game runs): red with white text (5:1).
    pub const STOP: Color32 = Color32::from_rgb(0xD7, 0x26, 0x3D);
    pub const DANGER: Color32 = Color32::from_rgb(0xFF, 0x3B, 0x30);
    /// Error text on white (5:1).
    pub const DANGER_TEXT: Color32 = Color32::from_rgb(0xD7, 0x00, 0x15);
    /// Keyboard focus ring.
    pub const FOCUS: Color32 = Color32::from_rgb(0x00, 0x7A, 0xFF);
    /// Placeholder for the selected account's head until skins are fetched.
    pub const HEAD_PLACEHOLDER: Color32 = Color32::from_rgb(0x5A, 0xC8, 0xFA);
    // Glass approximation: dark tints over the white panel colour.
    pub const GLASS_TINT: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 5);
    pub const GLASS_HOVER: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 12);
    pub const GLASS_PRESS: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 22);
}

/// iOS dark system colours (the default look).
#[allow(dead_code)]
mod dark {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0x1C, 0x1C, 0x1E);
    pub const PANEL: Color32 = Color32::from_rgb(0x2C, 0x2C, 0x2E);
    pub const OUTLINE: Color32 = Color32::from_rgb(0x4C, 0x4C, 0x50);
    pub const TEXT: Color32 = Color32::from_rgb(0xF2, 0xF2, 0xF7);
    pub const PLAY: Color32 = Color32::from_rgb(0x5A, 0xC8, 0xFA);
    pub const PLAY_TEXT: Color32 = Color32::from_rgb(0x06, 0x22, 0x36);
    pub const SHINE: Color32 = Color32::from_rgb(0xD6, 0xF2, 0xFF);
    pub const STOP: Color32 = Color32::from_rgb(0xD7, 0x26, 0x3D);
    pub const DANGER: Color32 = Color32::from_rgb(0xFF, 0x45, 0x3A);
    /// Error text on dark surfaces (6:1 on `PANEL`; `DANGER` itself is only 4:1).
    pub const DANGER_TEXT: Color32 = Color32::from_rgb(0xFF, 0x8A, 0x84);
    pub const FOCUS: Color32 = Color32::from_rgb(0x0A, 0x84, 0xFF);
    pub const HEAD_PLACEHOLDER: Color32 = Color32::from_rgb(0x64, 0xD2, 0xFF);
    // Glass approximation: white tints over the panel colour.
    pub const GLASS_TINT: Color32 = Color32::from_rgba_premultiplied(14, 14, 14, 14);
    pub const GLASS_HOVER: Color32 = Color32::from_rgba_premultiplied(22, 22, 22, 22);
    pub const GLASS_PRESS: Color32 = Color32::from_rgba_premultiplied(34, 34, 34, 34);
}

pub const STROKE_WIDTH: f32 = 2.0;
/// Line weight of every drawn icon (window buttons, dropdown arrow/check).
pub const ICON_STROKE: f32 = 2.0;
pub const WINDOW_RADIUS: u8 = 14;
/// Large surfaces: sidebar, main content panel.
pub const PANEL_RADIUS: u8 = 12;
/// Every control (Play button, dropdowns) shares this shape.
pub const CONTROL_RADIUS: u8 = 14;
/// Small controls (window buttons) and inner highlights.
pub const SMALL_RADIUS: u8 = 9;
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

/// Length of every expand/collapse animation (dropdowns, sidebar).
pub const EXPAND_SECONDS: f32 = 0.28;

/// The app's outline: a 2 px edge that shifts from the Play blue to `SHINE` along
/// the diagonal, with faint facet bands, so it catches light like a cut diamond.
pub fn outline(painter: &Painter, rect: Rect, r: u8) {
    let half = STROKE_WIDTH / 2.0;
    let mut points = Vec::new();
    // The path runs down the middle of the stroke, so its radius is reduced to stay concentric.
    path::rounded_rectangle(&mut points, RoundedRect::new(rect.shrink(half), CornerRadiusF32::same((r as f32 - half).max(0.0))));
    painter.add(PathShape::closed_line(points, PathStroke::new_uv(STROKE_WIDTH, |bounds, p| diamond(bounds, p))));
}

fn diamond(bounds: Rect, p: Pos2) -> Color32 {
    let u = ((p.x - bounds.min.x) / bounds.width().max(1.0) + (p.y - bounds.min.y) / bounds.height().max(1.0)) / 2.0;
    // Main sweep blue -> light, plus a faint ripple for the facet look.
    let t = (u + 0.08 * (u * std::f32::consts::TAU * 3.0).sin()).clamp(0.0, 1.0);
    PLAY.lerp_to_gamma(SHINE, t)
}

/// Glass surface used by everything that expands (dropdowns, sidebar): panel fill,
/// a white tint, the diamond outline, and a shadow that grows as it opens
/// (`t` 0 = collapsed, 1 = open; `resting_shadow` = shadow alpha when collapsed).
pub fn glass(painter: &Painter, rect: Rect, r: u8, t: f32, resting_shadow: u8) {
    let alpha = resting_shadow as f32 + (120.0 - resting_shadow as f32) * t;
    if alpha >= 1.0 {
        let shadow = Shadow {
            offset: [0, (2.0 + 6.0 * t) as i8],
            blur: (6.0 + 18.0 * t) as u8,
            spread: 0,
            color: Color32::from_black_alpha(alpha as u8),
        };
        painter.add(shadow.as_shape(rect, radius(r)));
    }
    painter.rect_filled(rect, radius(r), PANEL);
    painter.rect_filled(rect, radius(r), GLASS_TINT);
    outline(painter, rect, r);
}

/// Width the sidebar grows to while hovered.
pub const SIDEBAR_EXPANDED_WIDTH: f32 = 220.0;
/// Extra width the window clip leaves for the sidebar's spring overshoot.
pub const SIDEBAR_BOUNCE_ROOM: f32 = 32.0;

pub fn sidebar_expanded(collapsed: Rect) -> Rect {
    Rect::from_min_size(collapsed.min, vec2(SIDEBAR_EXPANDED_WIDTH.max(collapsed.width()), collapsed.height()))
}

/// How far a pressed control shrinks (fraction of its size).
const PRESS_DEPTH: f32 = 0.06;
/// The app's one spring, used by press feedback and the sidebar so they bounce
/// alike: stiff enough to feel immediate, slightly under-damped so a move
/// overshoots a little (about 14% of its distance) before settling.
const SPRING_STIFFNESS: f32 = 600.0;
const SPRING_DAMPING: f32 = 26.0;

#[derive(Clone, Copy, Default)]
struct Spring {
    value: f32,
    velocity: f32,
}

/// Moves the value stored under `id` toward `target` on the shared spring and
/// returns `(value, settled)`. The value starts at 0.
pub fn spring(ctx: &Context, id: Id, target: f32) -> (f32, bool) {
    let id = id.with("spring");
    let mut s = ctx.data(|d| d.get_temp::<Spring>(id)).unwrap_or_default();
    if s.value == target && s.velocity == 0.0 {
        return (target, true);
    }
    s = spring_step(s, target, ctx.input(|i| i.stable_dt).min(1.0 / 20.0));
    let settled = (s.value - target).abs() < 1e-3 && s.velocity.abs() < 1e-2;
    if settled {
        s = Spring { value: target, velocity: 0.0 };
    } else {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id, s));
    (s.value, settled)
}

/// Scale (around 1.0) of a control that shrinks while `pressed` and springs back
/// when released. Every pressable control uses this so they all move the same way.
pub fn press_scale(ctx: &Context, id: Id, pressed: bool) -> f32 {
    1.0 + spring(ctx, id.with("press"), if pressed { -PRESS_DEPTH } else { 0.0 }).0
}

/// Advances a spring by `elapsed` seconds in fixed small steps, so it stays
/// stable at any frame rate.
fn spring_step(mut s: Spring, target: f32, mut elapsed: f32) -> Spring {
    while elapsed > 0.0 {
        let dt = elapsed.min(1.0 / 240.0);
        let accel = -SPRING_STIFFNESS * (s.value - target) - SPRING_DAMPING * s.velocity;
        s.velocity += accel * dt;
        s.value += s.velocity * dt;
        elapsed -= dt;
    }
    s
}

/// `rect` scaled by `scale` around `center` (used to draw pressed controls).
pub fn scale_about(rect: Rect, center: Pos2, scale: f32) -> Rect {
    Rect::from_min_max(center + (rect.min - center) * scale, center + (rect.max - center) * scale)
}

/// Keyboard focus ring drawn just outside a control of the given radius.
pub fn focus_ring(painter: &Painter, rect: Rect, r: u8) {
    painter.rect_stroke(rect.expand(3.0), radius(r.saturating_add(3)), Stroke::new(2.0, FOCUS), StrokeKind::Outside);
}

/// Font family of the "Hermes" wordmark: Amsterdam Four when the font file was
/// present at build time (see build.rs), the regular UI font otherwise.
pub fn wordmark_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(WORDMARK_FAMILY.into()))
}

const WORDMARK_FAMILY: &str = "wordmark";

pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    let family = FontFamily::Name(WORDMARK_FAMILY.into());
    #[cfg(wordmark_font)]
    {
        let data = FontData::from_static(include_bytes!("../assets/fonts/amsterdam_four.ttf"));
        fonts.font_data.insert("amsterdam_four".into(), std::sync::Arc::new(data));
        fonts.families.insert(family.clone(), vec!["amsterdam_four".into()]);
    }
    // Glyphs the wordmark font lacks (or the whole font, if absent) come from the UI font.
    let fallback = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    fonts.families.entry(family).or_default().extend(fallback);
    ctx.set_fonts(fonts);
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
        let held = spring_step(Spring::default(), -PRESS_DEPTH, 0.5);
        assert!((held.value + PRESS_DEPTH).abs() < 1e-3);
        // Released: overshoots slightly above full size, then settles at it.
        let mut s = held;
        let mut peak = f32::MIN;
        for _ in 0..60 {
            s = spring_step(s, 0.0, 1.0 / 120.0);
            peak = peak.max(s.value);
        }
        // A small bounce: 5-16% of the distance travelled.
        let bounce = peak / PRESS_DEPTH;
        assert!(bounce > 0.05 && bounce < 0.16, "bounce {bounce}");
        assert!(spring_step(s, 0.0, 1.0).value.abs() < 1e-3);
        // A quick click (80 ms) still visibly shrinks.
        let quick = spring_step(Spring::default(), -PRESS_DEPTH, 0.08);
        assert!(quick.value < -PRESS_DEPTH * 0.5, "{}", quick.value);
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
