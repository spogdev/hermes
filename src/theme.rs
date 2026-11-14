//! The one design every screen shares (see `app-design.md`). Change looks here, not per widget.

use eframe::egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Id, Painter, Pos2, Rect, Shadow,
    Stroke, StrokeKind, Vec2, pos2, vec2,
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
    // Cool slate instead of neutral grey, so the surfaces sit with the blue accents.
    pub const BG: Color32 = Color32::from_rgb(0x14, 0x1C, 0x26);
    pub const PANEL: Color32 = Color32::from_rgb(0x1E, 0x29, 0x36);
    pub const OUTLINE: Color32 = Color32::from_rgb(0x33, 0x43, 0x55);
    pub const TEXT: Color32 = Color32::from_rgb(0xEE, 0xF4, 0xFA);
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
pub const ICON_STROKE: f32 = 2.5;
pub const WINDOW_RADIUS: u8 = 14;
/// Large surfaces: sidebar, main content panel.
pub const PANEL_RADIUS: u8 = 12;
/// Every control (Play button, dropdowns) shares this shape.
pub const CONTROL_RADIUS: u8 = 14;
/// Small controls (window buttons) and inner highlights.
pub const SMALL_RADIUS: u8 = 9;
/// Height of the draggable top bar (window buttons and account switcher).
pub const BAR_HEIGHT: f32 = 50.0;
/// The window itself (not counting the room the sidebar may grow into).
pub const WINDOW_WIDTH: f32 = 1200.0;
pub const WINDOW_HEIGHT: f32 = 707.0;
/// Collapsed sidebar width. It sits inside the window, flush with its left,
/// top and bottom edges.
pub const SIDEBAR_WIDTH: f32 = 75.0;
/// How far the hovered sidebar grows past the window's top, bottom and left edges.
pub const SIDEBAR_OUTSET: f32 = 6.0;
/// Transparent room kept around the window for that growth plus its spring overshoot.
const SIDEBAR_ROOM: f32 = 8.0;
/// Width the sidebar grows to while hovered.
pub const SIDEBAR_EXPANDED_WIDTH: f32 = 310.0;
/// Extra width allowed for the sidebar's spring overshoot when it opens.
const SIDEBAR_BOUNCE_ROOM: f32 = 40.0;
/// Logo size in the collapsed sidebar, and while it is expanded.
pub const LOGO_SIZE: f32 = 48.0;
pub const LOGO_SIZE_EXPANDED: f32 = 54.0;
/// Spacing between the main areas (content box, bottom controls, edges).
pub const GAP: f32 = 14.0;
/// Launch button and profile dropdown size.
pub const CONTROL_SIZE: Vec2 = vec2(230.0, 52.0);
/// Account switcher in the top bar.
pub const ACCOUNT_SIZE: Vec2 = vec2(210.0, 36.0);
/// Every text size is scaled by this (owner: about 15% bigger).
pub const TEXT_SCALE: f32 = 1.15;
/// Fully transparent border around the painted outline. The window clip
/// (`window_shape`) sits here, so its jagged per-pixel edge never touches a
/// painted pixel and the anti-aliased outline blends smoothly with the desktop.
pub const EDGE_MARGIN: f32 = 2.0;

fn main_size() -> Vec2 {
    vec2(WINDOW_WIDTH, WINDOW_HEIGHT)
}

/// Fixed native window size: the window, the room the hovered sidebar grows
/// into (left, top, bottom), and the transparent edge margin.
pub fn window_size() -> Vec2 {
    main_size() + vec2(SIDEBAR_ROOM, 2.0 * SIDEBAR_ROOM) + Vec2::splat(2.0 * EDGE_MARGIN)
}

/// Splits the native window area into the window and its collapsed sidebar,
/// which runs down the window's left edge.
pub fn frame_layout(area: Rect) -> (Rect, Rect) {
    let area = area.shrink(EDGE_MARGIN);
    let main = Rect::from_min_max(area.min + Vec2::splat(SIDEBAR_ROOM), area.max - vec2(0.0, SIDEBAR_ROOM));
    (main, Rect::from_min_max(main.min, pos2(main.min.x + SIDEBAR_WIDTH, main.max.y)))
}

/// The sidebar at expansion `t` (0 collapsed, 1 expanded; the spring may pass 1):
/// it widens over the window and grows past its top, bottom and left edges.
pub fn sidebar_rect(collapsed: Rect, t: f32) -> Rect {
    let out = SIDEBAR_OUTSET * t;
    let width = SIDEBAR_WIDTH + (SIDEBAR_EXPANDED_WIDTH - SIDEBAR_WIDTH) * t;
    Rect::from_min_max(collapsed.min - Vec2::splat(out), pos2(collapsed.min.x + width, collapsed.max.y + out))
}

/// Everything the sidebar can cover, overshoot included (for the window clip).
pub fn sidebar_reach(collapsed: Rect) -> Rect {
    let full = sidebar_rect(collapsed, 1.0);
    Rect::from_min_max(
        collapsed.min - Vec2::splat(SIDEBAR_ROOM),
        pos2(full.max.x + SIDEBAR_BOUNCE_ROOM, collapsed.max.y + SIDEBAR_ROOM),
    )
}

/// Areas of the window below the top bar.
pub struct Body {
    pub content: Rect,
    pub profile: Rect,
    pub play: Rect,
}

/// Lays out the window body: Launch bottom-right, the profile dropdown to its
/// left, and the content box filling the rest between the sidebar and the edges.
pub fn body_layout(window: Rect, sidebar: Rect) -> Body {
    let play = Rect::from_min_size(window.max - CONTROL_SIZE - Vec2::splat(GAP), CONTROL_SIZE);
    let profile = play.translate(vec2(-(CONTROL_SIZE.x + GAP), 0.0));
    let content = Rect::from_min_max(
        pos2(sidebar.max.x + GAP, window.min.y + BAR_HEIGHT + GAP),
        pos2(window.max.x - GAP, play.min.y - GAP),
    );
    Body { content, profile, play }
}

pub fn radius(r: u8) -> CornerRadius {
    CornerRadius::same(r)
}

/// The app's outline: a 2 px edge that shifts from the Play blue to `SHINE` along
/// the diagonal, with faint facet bands, so it catches light like a cut diamond.
pub fn outline(painter: &Painter, rect: Rect, r: u8) {
    selectable_outline(painter, rect, r, 0.0, 0.0);
}

/// Extra outline width when an element is fully selected (hovered, focused or open).
const SELECTED_EXTRA_WIDTH: f32 = 1.0;
/// How fast the light band travels along a selected outline (laps per second).
const SHIMMER_SPEED: f32 = 0.35;

/// The diamond outline of an element that can be selected. `selected` (0..1,
/// animated by the caller) thickens it slightly and adds a soft band of light
/// that travels along the gradient; `time` is the UI clock in seconds.
pub fn selectable_outline(painter: &Painter, rect: Rect, r: u8, selected: f32, time: f32) {
    let width = STROKE_WIDTH + SELECTED_EXTRA_WIDTH * selected;
    let half = width / 2.0;
    let mut points = Vec::new();
    // The path runs down the middle of the stroke, so its radius is reduced to stay concentric.
    path::rounded_rectangle(&mut points, RoundedRect::new(rect.shrink(half), CornerRadiusF32::same((r as f32 - half).max(0.0))));
    let phase = (time * SHIMMER_SPEED).fract();
    painter.add(PathShape::closed_line(points, PathStroke::new_uv(width, move |bounds, p| diamond(bounds, p, selected, phase))));
    if selected > 0.0 {
        painter.ctx().request_repaint();
    }
}

fn diamond(bounds: Rect, p: Pos2, selected: f32, phase: f32) -> Color32 {
    let u = ((p.x - bounds.min.x) / bounds.width().max(1.0) + (p.y - bounds.min.y) / bounds.height().max(1.0)) / 2.0;
    // Main sweep blue -> light, plus a faint ripple for the facet look.
    let t = (u + 0.08 * (u * std::f32::consts::TAU * 3.0).sin()).clamp(0.0, 1.0);
    let base = PLAY.lerp_to_gamma(SHINE, t);
    if selected <= 0.0 {
        return base;
    }
    // A soft highlight band that wraps around the diagonal as `phase` advances.
    let d = (u - phase).rem_euclid(1.0);
    let d = d.min(1.0 - d);
    let band = (-(d / 0.14).powi(2)).exp();
    base.lerp_to_gamma(Color32::WHITE, 0.55 * band * selected)
}

/// Glass surface used by everything that expands (dropdowns, sidebar): panel fill,
/// a white tint, the diamond outline, and a shadow that grows as it opens
/// (`t` 0 = collapsed, 1 = open; `resting_shadow` = shadow alpha when collapsed;
/// `selected` 0..1 drives the selected outline, see `selectable_outline`).
pub fn glass(painter: &Painter, rect: Rect, r: u8, t: f32, resting_shadow: u8, selected: f32) {
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
    selectable_outline(painter, rect, r, selected, painter.ctx().input(|i| i.time) as f32);
}

/// Animated 0..1 "selected" amount for an element (hovered, focused or open), so
/// the outline eases into and out of its selected look.
pub fn selected_amount(ctx: &Context, id: Id, selected: bool) -> f32 {
    ctx.animate_bool_with_time(id.with("selected"), selected, 0.15)
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

const WORDMARK_FAMILY: &str = "wordmark";

/// Fonts (both SIL Open Font License, licences in `assets/fonts/`): Afacad Flux
/// Bold for all text, and Pacifico only for the "Hermes" wordmark.
pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::empty();
    let text = "afacad_flux_bold".to_string();
    let wordmark = "pacifico".to_string();
    let bold = FontData::from_static(include_bytes!("../assets/fonts/AfacadFlux-Bold.ttf"));
    let pacifico = FontData::from_static(include_bytes!("../assets/fonts/Pacifico-Regular.ttf"));
    fonts.font_data.insert(text.clone(), std::sync::Arc::new(bold));
    fonts.font_data.insert(wordmark.clone(), std::sync::Arc::new(pacifico));
    // egui expects both built-in families to exist; the app has no monospace text.
    fonts.families.insert(FontFamily::Proportional, vec![text.clone()]);
    fonts.families.insert(FontFamily::Monospace, vec![text.clone()]);
    fonts.families.insert(FontFamily::Name(WORDMARK_FAMILY.into()), vec![wordmark, text]);
    ctx.set_fonts(fonts);
}

/// Pacifico, for the "Hermes" wordmark only.
pub fn wordmark_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(WORDMARK_FAMILY.into()))
}

/// The app's text font at `size` before the global text scale.
pub fn text_font(size: f32) -> FontId {
    FontId::proportional(size * TEXT_SCALE)
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
        // Tooltips and any egui-drawn text follow the same scale as the app's own text.
        for font in style.text_styles.values_mut() {
            font.size *= TEXT_SCALE;
        }
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
    fn sidebar_sits_in_the_window_and_grows_out_of_it() {
        let area = Rect::from_min_size(pos2(0.0, 0.0), window_size());
        let (main, sidebar) = frame_layout(area);
        assert_eq!(main.size(), main_size());
        assert!(main.contains_rect(sidebar));
        assert_eq!((sidebar.min, sidebar.max.y, sidebar.width()), (main.min, main.max.y, SIDEBAR_WIDTH));
        // Expanded, it passes the window's top, bottom and left edges by the outset...
        let open = sidebar_rect(sidebar, 1.0);
        assert_eq!(open.min, main.min - Vec2::splat(SIDEBAR_OUTSET));
        assert_eq!(open.max.y, main.max.y + SIDEBAR_OUTSET);
        // ...and even with spring overshoot it stays inside the clip and the native window.
        let reach = sidebar_reach(sidebar);
        assert!(reach.contains_rect(sidebar_rect(sidebar, 1.15)));
        assert!(area.contains_rect(reach.shrink(0.01)));
    }

    #[test]
    fn body_areas_fit_without_overlapping() {
        let area = Rect::from_min_size(pos2(0.0, 0.0), window_size());
        let (main, sidebar) = frame_layout(area);
        let body = body_layout(main, sidebar);
        for r in [body.content, body.profile, body.play] {
            assert!(main.contains_rect(r));
            assert!(!r.intersects(sidebar));
        }
        assert!(!body.content.intersects(body.play) && !body.content.intersects(body.profile));
        assert!(!body.profile.intersects(body.play));
        assert!(body.content.min.y >= main.min.y + BAR_HEIGHT);
    }
}
