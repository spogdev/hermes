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
    /// Solid outline colour (everything except the sidebar and window outlines).
    pub const EDGE: Color32 = Color32::from_rgb(0x8C, 0xCF, 0xEE);
    /// Popups: an opaque card, its hairline edge, and its input fields.
    pub const POPUP_FILL: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const POPUP_EDGE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 22);
    pub const FIELD_FILL: Color32 = Color32::from_rgb(0xF2, 0xF3, 0xF7);
    pub const FIELD_EDGE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 26);
    // Glass approximation: dark tints over the white panel colour.
    pub const GLASS_TINT: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 5);
    pub const GLASS_HOVER: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 12);
    pub const GLASS_PRESS: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 22);
    pub const FROST: Color32 = Color32::from_rgb(0xEC, 0xF6, 0xFF);
    pub const TEXT_SELECTION: Color32 = Color32::from_rgba_premultiplied(18, 40, 50, 60);
    /// Background blobs: colour and strength (0..1) over `BG`.
    pub const BLOBS: [(Color32, f32); 5] = [
        (Color32::from_rgb(0xC2, 0xDD, 0xF5), 0.36),
        (Color32::from_rgb(0xBC, 0xE8, 0xE6), 0.33),
        (Color32::from_rgb(0xCF, 0xD2, 0xF5), 0.3),
        (Color32::from_rgb(0xB8, 0xE2, 0xF2), 0.3),
        (Color32::from_rgb(0x5A, 0xC8, 0xFA), 0.07),
    ];
}

/// Ink & Gold (the default look; owner picked it in the colour revamp): after
/// Hermes himself, near-black ink-blue surfaces, warm ivory text and one gold
/// accent for everything that acts or is selected.
#[allow(dead_code)]
mod dark {
    use eframe::egui::Color32;
    // Ink: blue-black, a touch cooler than neutral so the gold reads warm against it.
    pub const BG: Color32 = Color32::from_rgb(0x0E, 0x11, 0x18);
    pub const PANEL: Color32 = Color32::from_rgb(0x17, 0x1C, 0x26);
    pub const OUTLINE: Color32 = Color32::from_rgb(0x2A, 0x31, 0x40);
    /// Warm ivory (16:1 on `BG`).
    pub const TEXT: Color32 = Color32::from_rgb(0xF3, 0xEE, 0xE3);
    /// The accent (Launch, Create, Add Mods, Save, toggles, links, selection):
    /// gold (owner: less orange), 10:1 on `PANEL`, with ink text on it (11:1).
    pub const PLAY: Color32 = Color32::from_rgb(0xE8, 0xC5, 0x5F);
    pub const PLAY_TEXT: Color32 = Color32::from_rgb(0x1A, 0x13, 0x05);
    /// Light end of the diamond outline: pale champagne.
    pub const SHINE: Color32 = Color32::from_rgb(0xF8, 0xEB, 0xC2);
    /// Stop and Delete: a warm red with white text (5.2:1).
    pub const STOP: Color32 = Color32::from_rgb(0xC8, 0x38, 0x2F);
    pub const DANGER: Color32 = Color32::from_rgb(0xFF, 0x5A, 0x4E);
    /// Error text on dark surfaces (8:1 on `PANEL`; `DANGER` itself is lower).
    pub const DANGER_TEXT: Color32 = Color32::from_rgb(0xFF, 0x9A, 0x8F);
    /// Keyboard focus ring: a lighter gold than the accent, so it shows on gold buttons too.
    pub const FOCUS: Color32 = Color32::from_rgb(0xFF, 0xE3, 0x9A);
    pub const HEAD_PLACEHOLDER: Color32 = Color32::from_rgb(0xE8, 0xC5, 0x5F);
    /// Solid outline colour for everything except the sidebar and the window:
    /// the ivory text colour, partly transparent (43%) so it stays subtle (owner
    /// request: white outlines). Selected elements brighten it to the gold.
    pub const EDGE: Color32 = Color32::from_rgba_premultiplied(105, 102, 98, 110);
    /// Popups: an opaque card (a step lighter than the panels), its hairline
    /// edge, and its input fields (a step darker, recessed).
    pub const POPUP_FILL: Color32 = Color32::from_rgb(0x1A, 0x1F, 0x2A);
    pub const POPUP_EDGE: Color32 = Color32::from_rgba_premultiplied(26, 25, 24, 26);
    pub const FIELD_FILL: Color32 = Color32::from_rgb(0x10, 0x14, 0x1C);
    pub const FIELD_EDGE: Color32 = Color32::from_rgba_premultiplied(20, 19, 18, 20);
    // Glass approximation: warm white tints over the panel colour.
    pub const GLASS_TINT: Color32 = Color32::from_rgba_premultiplied(14, 14, 13, 14);
    pub const GLASS_HOVER: Color32 = Color32::from_rgba_premultiplied(22, 21, 20, 22);
    pub const GLASS_PRESS: Color32 = Color32::from_rgba_premultiplied(34, 33, 31, 34);
    /// The frosted glass rings (pane rims, selection boxes): ivory with a hint of gold.
    pub const FROST: Color32 = Color32::from_rgb(0xFF, 0xF4, 0xDE);
    /// Text selection behind highlighted characters, and the text caret.
    pub const TEXT_SELECTION: Color32 = Color32::from_rgba_premultiplied(82, 70, 34, 90);
    /// Background blobs: colour and strength (0..1) over `BG`. Night colours
    /// around the ink (deep indigo, oxblood, midnight blue, bronze) and a trace
    /// of gold, so the lower background stays dark.
    pub const BLOBS: [(Color32, f32); 5] = [
        (Color32::from_rgb(0x26, 0x2B, 0x6A), 0.42),
        (Color32::from_rgb(0x5A, 0x1E, 0x2A), 0.30),
        (Color32::from_rgb(0x18, 0x2E, 0x58), 0.40),
        (Color32::from_rgb(0x5C, 0x40, 0x1C), 0.20),
        (Color32::from_rgb(0xE8, 0xC5, 0x5F), 0.06),
    ];
}

pub const STROKE_WIDTH: f32 = 2.0;
/// The diamond outline (the window and the sidebar): a little thicker (owner request).
pub const DIAMOND_STROKE_WIDTH: f32 = 3.0;
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
/// Sidebar navigation rows (Home at the top, Settings at the bottom).
pub const NAV_ROW_HEIGHT: f32 = 50.0;
/// Space between stacked navigation rows (Home, Profiles).
pub const NAV_ROW_GAP: f32 = 6.0;
/// Side and edge spacing of the rows inside the sidebar.
pub const NAV_INSET: f32 = 10.0;
/// Navigation icon size (smaller than the 48 px logo).
pub const NAV_ICON_SIZE: f32 = 32.0;
/// Navigation label size (before `TEXT_SCALE`).
pub const NAV_TEXT_SIZE: f32 = 17.0;
/// The hollow outline around each row: a fixed length, the full width of the open
/// sidebar; while the sidebar is narrower it's simply cut off by the sidebar's edge.
pub const NAV_ROW_WIDTH: f32 = SIDEBAR_EXPANDED_WIDTH - 2.0 * NAV_INSET;
/// Spacing between the main areas (content box, bottom controls, edges).
pub const GAP: f32 = 14.0;
/// Size of a standard large button (the old Launch button): wide controls shrink
/// by as many pixels as this does when pressed (`press_scale_sized`).
pub const CONTROL_SIZE: Vec2 = vec2(230.0, 52.0);
/// A profile's page: the left column's share of the width, the profile icon,
/// the round header buttons.
pub const PROFILE_LEFT_FRACTION: f32 = 0.4;
pub const PROFILE_ICON_SIZE: f32 = 128.0;
pub const CIRCLE_BUTTON_SIZE: f32 = 44.0;
/// The profile page's Launch button (as tall as the round buttons) and the gap
/// between the header's buttons.
pub const LAUNCH_BUTTON_WIDTH: f32 = 120.0;
pub const HEADER_BUTTON_GAP: f32 = 10.0;
/// The mods table: header row, side padding, column starts (fractions of the
/// inner width: Project, Author, Version, Options), rows and their icons.
pub const TABLE_HEADER_HEIGHT: f32 = 42.0;
pub const TABLE_PADDING: f32 = 18.0;
pub const TABLE_COLUMNS: [f32; 4] = [0.0, 0.43, 0.65, 0.84];
pub const MOD_ROW_HEIGHT: f32 = 50.0;
pub const MOD_ICON_SIZE: f32 = 32.0;
/// Result page buttons while browsing mods.
pub const PAGER_HEIGHT: f32 = 32.0;
/// The mod detail pane: its icon and padding.
pub const MOD_DETAIL_ICON_SIZE: f32 = 76.0;
pub const DETAIL_PADDING: f32 = 16.0;
/// Profile cards (Home and Profiles pages).
pub const CARD_HEIGHT: f32 = 68.0;
pub const CARD_GAP: f32 = 10.0;
pub const CARD_ICON_SIZE: f32 = 44.0;
/// The Profiles page's Create button, above the list on the right.
pub const CREATE_BUTTON_SIZE: Vec2 = vec2(132.0, 36.0);
/// The create-profile popup.
pub const POPUP_SIZE: Vec2 = vec2(580.0, 352.0);
pub const POPUP_PADDING: f32 = 28.0;
pub const POPUP_BUTTON_SIZE: Vec2 = vec2(140.0, 42.0);
/// Popup corners, and the corners of its fields and buttons.
pub const POPUP_RADIUS: u8 = 22;
pub const FIELD_RADIUS: u8 = 11;
/// Text fields in popups.
pub const FIELD_HEIGHT: f32 = 40.0;
/// The profile popup's max-RAM slider: its row (the knob's room), the track's
/// thickness and the knob.
pub const SLIDER_HEIGHT: f32 = 24.0;
pub const SLIDER_TRACK: f32 = 6.0;
pub const SLIDER_KNOB_RADIUS: f32 = 9.0;
/// The icon drop area in the create-profile popup.
pub const CREATE_ICON_SIZE: f32 = 128.0;
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

/// The sidebar at `width_t` (how far it has widened) and `outset_t` (how far it
/// has grown past the window's top, bottom and left edges): 0 collapsed, 1
/// expanded; springs may pass 1. They move separately so the growth can trail
/// the widening.
pub fn sidebar_rect(collapsed: Rect, width_t: f32, outset_t: f32) -> Rect {
    let out = SIDEBAR_OUTSET * outset_t;
    let width = SIDEBAR_WIDTH + (SIDEBAR_EXPANDED_WIDTH - SIDEBAR_WIDTH) * width_t;
    Rect::from_min_max(collapsed.min - Vec2::splat(out), pos2(collapsed.min.x + width, collapsed.max.y + out))
}

/// Everything the sidebar can cover, overshoot included (for the window clip).
pub fn sidebar_reach(collapsed: Rect) -> Rect {
    let full = sidebar_rect(collapsed, 1.0, 1.0);
    Rect::from_min_max(
        collapsed.min - Vec2::splat(SIDEBAR_ROOM),
        pos2(full.max.x + SIDEBAR_BOUNCE_ROOM, collapsed.max.y + SIDEBAR_ROOM),
    )
}

/// Pointer speed (points/second) above which a move counts as heading somewhere:
/// leaving the sidebar, or (moving right) sweeping in from the left, which
/// doesn't open the collapsed sidebar.
pub const EXIT_SPEED: f32 = 400.0;
/// Seconds the pointer must be over the collapsed sidebar, after coming in from
/// the left or from outside the window, before it can open: long enough for a
/// real speed reading, so a fast sweep in has already passed through.
pub const SIDEBAR_ENTRY_DWELL: f64 = 0.08;
/// How far ahead the pointer's path is projected when predicting an exit.
const EXIT_LOOKAHEAD: f32 = 0.12;

/// Whether a pointer at `pos` moving at `velocity` (points/second) is about to
/// leave `area`: it's moving fast and its position a moment from now lies
/// outside. Fast moves that stay within the area don't count, and neither do
/// flicks that are more vertical than horizontal (moving up or down the menu).
pub fn heading_out(area: Rect, pos: Pos2, velocity: Vec2) -> bool {
    velocity.x.abs() > velocity.y.abs()
        && velocity.length() >= EXIT_SPEED
        && !area.contains(pos + velocity * EXIT_LOOKAHEAD)
}

/// Areas of the window below the top bar.
pub struct Body {
    /// The home page's pane: the right half.
    pub content: Rect,
    /// A full-width pane: everything between the sidebar and the right edge.
    pub wide: Rect,
}

/// Lays out the window body below the top bar, down to the bottom edge (there's
/// no Launch button any more: profiles launch from their cards).
pub fn body_layout(window: Rect, sidebar: Rect) -> Body {
    // The content box takes the right half of the space between the sidebar and
    // the right edge (owner request); the left half is left free.
    let left = sidebar.max.x + GAP;
    let right = window.max.x - GAP;
    let wide = Rect::from_min_max(pos2(left, window.min.y + BAR_HEIGHT + GAP), pos2(right, window.max.y - GAP));
    let content = Rect::from_min_max(pos2((left + right) / 2.0, wide.min.y), wide.max);
    Body { content, wide }
}

pub fn radius(r: u8) -> CornerRadius {
    CornerRadius::same(r)
}

/// How an outline is coloured.
#[derive(Clone, Copy, PartialEq)]
pub enum Edge {
    /// Shifts from the gold accent to `SHINE` along the diagonal with faint facet
    /// bands, like a cut diamond. Only the window and the sidebar use it.
    Diamond,
    /// One colour (`EDGE`, brightening to `PLAY` when selected). Everything else.
    Solid,
}

/// An outline: 3 px diamond (window, sidebar) or 2 px solid (everything else).
pub fn outline(painter: &Painter, rect: Rect, r: u8, edge: Edge) {
    let time = painter.ctx().input(|i| i.time) as f32;
    selectable_outline(painter, rect, radius(r), 0.0, time, edge);
}

/// Extra outline width when an element is fully selected (hovered, focused or open).
const SELECTED_EXTRA_WIDTH: f32 = 1.0;
/// How fast the light band travels along a selected outline (laps per second).
const SHIMMER_SPEED: f32 = 0.35;
/// At rest a diamond outline (window, sidebar) is animated too (owner request):
/// a fainter glint drifts slowly around it, one lap every ~9 s. It hands over
/// to the brighter, faster band as the element is selected.
const GLINT_SPEED: f32 = 0.11;
const GLINT_STRENGTH: f32 = 0.6;

/// The outline of an element that can be selected. `selected` (0..1, animated by
/// the caller) thickens it slightly; a diamond outline also gets a soft band of
/// light that travels along the gradient, a solid one brightens to the Launch
/// gold. `time` is the UI clock in seconds.
pub fn selectable_outline(painter: &Painter, rect: Rect, corners: CornerRadius, selected: f32, time: f32, edge: Edge) {
    let base = if edge == Edge::Diamond { DIAMOND_STROKE_WIDTH } else { STROKE_WIDTH };
    let width = base + SELECTED_EXTRA_WIDTH * selected;
    let half = width / 2.0;
    let mut points = Vec::new();
    // The path runs down the middle of the stroke, so its radii are reduced to stay concentric.
    let inset = |r: u8| (r as f32 - half).max(0.0);
    let path_corners = CornerRadiusF32 { nw: inset(corners.nw), ne: inset(corners.ne), sw: inset(corners.sw), se: inset(corners.se) };
    path::rounded_rectangle(&mut points, RoundedRect::new(rect.shrink(half), path_corners));
    if edge == Edge::Solid {
        painter.add(PathShape::closed_line(points, PathStroke::new(width, EDGE.lerp_to_gamma(PLAY, selected))));
        return;
    }
    // With animations turned off in Windows the light band holds still and the
    // resting glint is off.
    let still = reduce_motion();
    let phase = if still { 0.25 } else { (time * SHIMMER_SPEED).fract() };
    let glint = if still { None } else { Some((time * GLINT_SPEED).fract()) };
    // Colours are worked out per point, so the straight sides get extra points
    // for the bands to show along them (not just at the corners).
    let points = subdivide(&points, 6.0);
    painter.add(PathShape::closed_line(points, PathStroke::new_uv(width, move |bounds, p| diamond(bounds, p, selected, phase, glint))));
    if !still {
        // The glint is slow, so the backdrop's 30 fps is enough at rest; the
        // selected band moves faster and asks for every frame.
        if selected > 0.0 {
            painter.ctx().request_repaint();
        } else {
            painter.ctx().request_repaint_after(std::time::Duration::from_millis(33));
        }
    }
}

/// A closed path with extra points so no segment is longer than `step`.
fn subdivide(points: &[Pos2], step: f32) -> Vec<Pos2> {
    let mut out = Vec::with_capacity(points.len() * 2);
    for (i, &a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        let n = ((b - a).length() / step).ceil().max(1.0) as usize;
        out.extend((0..n).map(|k| a + (b - a) * (k as f32 / n as f32)));
    }
    out
}

fn diamond(bounds: Rect, p: Pos2, selected: f32, phase: f32, glint: Option<f32>) -> Color32 {
    let u = ((p.x - bounds.min.x) / bounds.width().max(1.0) + (p.y - bounds.min.y) / bounds.height().max(1.0)) / 2.0;
    // Main sweep gold -> light, plus a faint ripple for the facet look.
    let t = (u + 0.08 * (u * std::f32::consts::TAU * 3.0).sin()).clamp(0.0, 1.0);
    let base = PLAY.lerp_to_gamma(SHINE, t);
    // A soft highlight band that wraps around the diagonal as its phase advances.
    let band_at = |phase: f32, width: f32| {
        let d = (u - phase).rem_euclid(1.0);
        let d = d.min(1.0 - d);
        (-(d / width).powi(2)).exp()
    };
    // The resting glint: wider and fainter, fading out as the element is selected.
    let base = match glint {
        Some(g) if selected < 1.0 => base.lerp_to_gamma(Color32::WHITE, GLINT_STRENGTH * band_at(g, 0.18) * (1.0 - selected)),
        _ => base,
    };
    if selected <= 0.0 {
        return base;
    }
    let band = band_at(phase, 0.14);
    base.lerp_to_gamma(Color32::WHITE, 0.55 * band * selected)
}

/// Glass surface used by everything that expands (dropdowns, sidebar): panel fill,
/// a white tint, the diamond outline, and a shadow that grows as it opens
/// (`t` 0 = collapsed, 1 = open; `resting_shadow` = shadow alpha when collapsed;
/// `selected` 0..1 drives the selected outline, see `selectable_outline`).
pub fn glass(painter: &Painter, rect: Rect, corners: CornerRadius, t: f32, resting_shadow: u8, selected: f32, edge: Edge) {
    let alpha = resting_shadow as f32 + (120.0 - resting_shadow as f32) * t;
    if alpha >= 1.0 {
        let shadow = Shadow {
            offset: [0, (2.0 + 6.0 * t) as i8],
            blur: (6.0 + 18.0 * t) as u8,
            spread: 0,
            color: Color32::from_black_alpha(alpha as u8),
        };
        painter.add(shadow.as_shape(rect, corners));
    }
    painter.rect_filled(rect, corners, PANEL);
    painter.rect_filled(rect, corners, GLASS_TINT);
    selectable_outline(painter, rect, corners, selected, painter.ctx().input(|i| i.time) as f32, edge);
}

/// Width of the frosted selector ring.
const FROSTED_WIDTH: f32 = 4.0;

/// The sidebar's selector: a hollow ring of frosted glass, iOS style. No fill;
/// the thick edge is translucent warm white (`FROST`) that brightens where light catches
/// the glass (the top-left and bottom-right corners, and the top more than the
/// bottom), with a fine bright inner rim and a faint dark outer rim for depth.
/// Faded by `alpha` (0..1).
pub fn frosted(painter: &Painter, rect: Rect, r: u8, alpha: f32) {
    frosted_ring(painter, rect, r, alpha, Light::PANE);
}

/// The selection box (sidebar rows, mods table rows): the frosted ring, lighter
/// and whiter than on panes (owner request), over a faint white tint.
pub fn selector(painter: &Painter, rect: Rect, r: u8, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    painter.rect_filled(rect, radius(r), Color32::from_white_alpha((14.0 * alpha) as u8));
    frosted_ring(painter, rect, r, alpha, Light::SELECTOR);
}

/// How bright a frosted ring is: base light, extra at the top, at the lit
/// corners, its cap, and the inner rim's alpha.
struct Light {
    base: f32,
    top: f32,
    specular: f32,
    max: f32,
    rim: f32,
}

impl Light {
    // Panes: lighter and shinier (owner request), bright glints at the lit corners.
    const PANE: Light = Light { base: 0.28, top: 0.16, specular: 0.62, max: 0.96, rim: 58.0 };
    const SELECTOR: Light = Light { base: 0.36, top: 0.16, specular: 0.42, max: 0.92, rim: 70.0 };
}

fn frosted_ring(painter: &Painter, rect: Rect, r: u8, alpha: f32, light: Light) {
    if alpha <= 0.0 {
        return;
    }
    let Light { base, top, specular: shine, max, rim } = light;
    let ring = |rect: Rect, width: f32, r: f32| {
        let mut points = Vec::new();
        path::rounded_rectangle(&mut points, RoundedRect::new(rect, CornerRadiusF32::same(r.max(0.0))));
        (points, width)
    };
    let half = FROSTED_WIDTH / 2.0;
    let r = r as f32;

    // Depth: a faint dark line just outside the glass.
    let (outer, w) = ring(rect.expand(0.5), 1.0, r + 0.5);
    painter.add(PathShape::closed_line(outer, PathStroke::new(w, Color32::from_black_alpha((60.0 * alpha) as u8))));

    // The glass edge itself.
    let (body, w) = ring(rect.shrink(half), FROSTED_WIDTH, r - half);
    painter.add(PathShape::closed_line(
        body,
        PathStroke::new_uv(w, move |bounds, p| {
            let x = (p.x - bounds.min.x) / bounds.width().max(1.0);
            let y = (p.y - bounds.min.y) / bounds.height().max(1.0);
            let diagonal = (x + y) / 2.0;
            let specular = (-(diagonal / 0.22).powi(2)).exp() + (-((1.0 - diagonal) / 0.22).powi(2)).exp();
            let light = base + top * (1.0 - y) + shine * specular;
            Color32::from_rgba_unmultiplied(FROST.r(), FROST.g(), FROST.b(), (light.min(max) * 255.0 * alpha) as u8)
        }),
    ));

    // Refraction rim: a fine bright line along the inside of the glass.
    let (inner, w) = ring(rect.shrink(FROSTED_WIDTH + 0.5), 1.0, r - FROSTED_WIDTH - 0.5);
    painter.add(PathShape::closed_line(inner, PathStroke::new(w, Color32::from_white_alpha((rim * alpha) as u8))));
}

/// A popup's card: opaque (owner request: popups have no see-through
/// background), with a hairline edge and a large soft shadow.
pub fn popup_card(painter: &Painter, rect: Rect) {
    let corners = radius(POPUP_RADIUS);
    painter.add(Shadow { offset: [0, 18], blur: 48, spread: 0, color: Color32::from_black_alpha(150) }.as_shape(rect, corners));
    painter.rect_filled(rect, corners, POPUP_FILL);
    painter.rect_stroke(rect, corners, Stroke::new(1.0, POPUP_EDGE), StrokeKind::Inside);
}

/// An input field in a popup: recessed fill with a hairline edge that turns
/// the gold accent as `focus` (0..1) rises.
pub fn field(painter: &Painter, rect: Rect, focus: f32) {
    let corners = radius(FIELD_RADIUS);
    painter.rect_filled(rect, corners, FIELD_FILL);
    painter.rect_stroke(rect, corners, Stroke::new(1.0 + 0.5 * focus, FIELD_EDGE.lerp_to_gamma(PLAY, focus)), StrokeKind::Inside);
}

/// How opaque a pane's glass body is: the moving background shows through.
const PANE_OPACITY: f32 = 0.55;
/// White sheen at the top of a pane, fading to none at the bottom.
const PANE_SHEEN: f32 = 0.06;

/// A content pane in the style of Apple's Liquid Glass (owner request): a
/// translucent body the drifting background shows through, lit by a soft sheen
/// that fades from the top, edged with the same frosted rim as the sidebar
/// selector (bright where light catches the top-left and bottom-right corners).
pub fn pane(painter: &Painter, rect: Rect, r: u8) {
    let mut points = Vec::new();
    path::rounded_rectangle(&mut points, RoundedRect::new(rect, CornerRadiusF32::same(r as f32)));
    // The shape is convex, so a fan from the centre fills it; colour varies
    // linearly with height, which the triangles reproduce exactly.
    let color = |y: f32| {
        let sheen = PANE_SHEEN * (rect.max.y - y) / rect.height().max(1.0);
        let base = PANE_OPACITY * (1.0 - sheen);
        let channel = |c: u8| (c as f32 * base + 255.0 * sheen) as u8;
        Color32::from_rgba_premultiplied(channel(PANEL.r()), channel(PANEL.g()), channel(PANEL.b()), (255.0 * (base + sheen)) as u8)
    };
    let mut mesh = eframe::egui::Mesh::default();
    mesh.colored_vertex(rect.center(), color(rect.center().y));
    for p in &points {
        mesh.colored_vertex(*p, color(p.y));
    }
    let n = points.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(mesh);
    frosted(painter, rect, r, 1.0);
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

/// How much a spring bounces. All share the same stiffness, so they move at the
/// same pace; only the overshoot differs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Bounce {
    /// Presses and dropdowns: about 14% overshoot.
    Lively,
    /// The sidebar panel: a little calmer, about 7% overshoot (owner request).
    Calm,
    /// No overshoot at all: text and the logo, so they never wobble.
    None,
}

impl Bounce {
    fn damping(self) -> f32 {
        match self {
            Bounce::Lively => SPRING_DAMPING,
            Bounce::Calm => 32.0,
            // Critical damping: the fastest approach with no overshoot.
            Bounce::None => 2.0 * SPRING_STIFFNESS.sqrt(),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Spring {
    value: f32,
    velocity: f32,
}

/// State of a value moving on the shared spring.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub value: f32,
    /// Units per second; used for effects that respond to speed.
    pub velocity: f32,
    pub settled: bool,
}

/// Grid spacing of the backdrop mesh (its colours are blended between points).
const BACKDROP_STEP: f32 = 14.0;
/// How far in from each edge the backdrop fades out; more at the top so it
/// blends into the plain top bar.
const BACKDROP_FADE: f32 = 40.0;
const BACKDROP_FADE_TOP: f32 = 90.0;

/// Soft gradient blobs drifting slowly around `rect` (the window body below the
/// top bar). Each blob wanders its own looping path, 30-50 s per loop. They fade
/// out towards every edge, so the window's rounded corners stay clean. `time` is
/// seconds; with reduced motion the blobs hold still.
pub fn backdrop(painter: &Painter, rect: Rect, time: f64) {
    let t = if reduce_motion() { 0.0 } else { time as f32 };
    // Per blob: size (fraction of the area's height), loop speeds (rad/s) and phase.
    // Small enough that each reads as its own round spot, never the whole area.
    const PATHS: [(f32, f32, f32, f32); 5] = [
        (0.30, 0.16, 0.21, 0.0),
        (0.27, 0.19, 0.13, 1.3),
        (0.25, 0.14, 0.17, 2.6),
        (0.23, 0.12, 0.19, 3.9),
        (0.18, 0.21, 0.15, 5.2),
    ];
    let blobs: Vec<(Pos2, f32, Color32, f32)> = BLOBS
        .iter()
        .zip(PATHS)
        .map(|(&(color, strength), (size, sx, sy, phase))| {
            let radius = size * rect.height();
            // Centres stay a radius in from the edges, so the edge fade never
            // squares a blob off.
            let reach = (rect.size() / 2.0 - Vec2::splat(radius)).max(Vec2::ZERO);
            let wander = vec2(reach.x * (t * sx + phase).sin(), reach.y * (t * sy + 1.7 * phase).cos());
            (rect.center() + wander, radius, color, strength)
        })
        .collect();
    let smooth = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let shade = |p: Pos2| {
        let edge = smooth((p.x - rect.min.x) / BACKDROP_FADE)
            * smooth((rect.max.x - p.x) / BACKDROP_FADE)
            * smooth((rect.max.y - p.y) / BACKDROP_FADE)
            * smooth((p.y - rect.min.y) / BACKDROP_FADE_TOP);
        // Premultiplied blend of every blob over the background.
        let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 0.0, 0.0);
        for &(centre, radius, color, strength) in &blobs {
            let d = (p - centre).length() / radius;
            let w = strength * (-d * d).exp() * edge;
            r += color.r() as f32 * w;
            g += color.g() as f32 * w;
            b += color.b() as f32 * w;
            a += 255.0 * w;
        }
        let a = a.min(255.0);
        Color32::from_rgba_premultiplied(r.min(a) as u8, g.min(a) as u8, b.min(a) as u8, a as u8)
    };

    let cols = (rect.width() / BACKDROP_STEP).ceil().max(1.0) as u32;
    let rows = (rect.height() / BACKDROP_STEP).ceil().max(1.0) as u32;
    let mut mesh = eframe::egui::Mesh::default();
    for j in 0..=rows {
        for i in 0..=cols {
            let p = pos2(
                eframe::egui::lerp(rect.min.x..=rect.max.x, i as f32 / cols as f32),
                eframe::egui::lerp(rect.min.y..=rect.max.y, j as f32 / rows as f32),
            );
            mesh.colored_vertex(p, shade(p));
        }
    }
    let at = |i: u32, j: u32| j * (cols + 1) + i;
    for j in 0..rows {
        for i in 0..cols {
            mesh.add_triangle(at(i, j), at(i + 1, j), at(i, j + 1));
            mesh.add_triangle(at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
        }
    }
    painter.add(mesh);
}

/// True when Windows' "Show animations in Windows" setting is off. Read once.
pub fn reduce_motion() -> bool {
    static REDUCE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *REDUCE.get_or_init(|| {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW};
        let mut enabled: windows_sys::core::BOOL = 1;
        // SAFETY: the out-pointer is a valid BOOL for this query.
        let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&mut enabled as *mut windows_sys::core::BOOL).cast(), 0) };
        ok != 0 && enabled == 0
    })
}

fn spring_id(id: Id) -> Id {
    id.with("spring")
}

/// Moves the value stored under `id` toward `target` on the shared spring. The
/// value starts at 0. With animations turned off in Windows it jumps straight
/// to the target. Call once per frame per `id` (use `spring_peek` to only read).
pub fn spring_motion(ctx: &Context, id: Id, target: f32) -> Motion {
    spring_motion_with(ctx, id, target, Bounce::Lively)
}

/// `spring_motion` with a chosen amount of bounce.
pub fn spring_motion_with(ctx: &Context, id: Id, target: f32, bounce: Bounce) -> Motion {
    let id = spring_id(id);
    let mut s = ctx.data(|d| d.get_temp::<Spring>(id)).unwrap_or_default();
    if (s.value == target && s.velocity == 0.0) || reduce_motion() {
        ctx.data_mut(|d| d.insert_temp(id, Spring { value: target, velocity: 0.0 }));
        return Motion { value: target, velocity: 0.0, settled: true };
    }
    s = spring_step(s, target, ctx.input(|i| i.stable_dt).min(1.0 / 20.0), bounce.damping());
    let settled = (s.value - target).abs() < 1e-3 && s.velocity.abs() < 1e-2;
    if settled {
        s = Spring { value: target, velocity: 0.0 };
    } else {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id, s));
    Motion { value: s.value, velocity: s.velocity, settled }
}

/// Reads a spring without advancing it (settled = at rest).
pub fn spring_peek(ctx: &Context, id: Id) -> Motion {
    let s = ctx.data(|d| d.get_temp::<Spring>(spring_id(id))).unwrap_or_default();
    Motion { value: s.value, velocity: s.velocity, settled: s.velocity == 0.0 }
}

/// `spring_motion` as `(value, settled)`.
pub fn spring(ctx: &Context, id: Id, target: f32) -> (f32, bool) {
    let m = spring_motion(ctx, id, target);
    (m.value, m.settled)
}

/// Scale (around 1.0) of a control that shrinks while `pressed` and springs back
/// when released. Every pressable control uses this so they all move the same way.
pub fn press_scale(ctx: &Context, id: Id, pressed: bool) -> f32 {
    press_scale_by(ctx, id, pressed, PRESS_DEPTH)
}

/// `press_scale` for wide controls (list rows): they shrink by about as many
/// pixels as the Launch button does rather than by the same fraction, so a
/// long row barely moves (owner request: rows shouldn't shrink as much).
pub fn press_scale_sized(ctx: &Context, id: Id, pressed: bool, width: f32) -> f32 {
    press_scale_by(ctx, id, pressed, PRESS_DEPTH * (CONTROL_SIZE.x / width.max(1.0)).min(1.0))
}

fn press_scale_by(ctx: &Context, id: Id, pressed: bool, depth: f32) -> f32 {
    1.0 + spring(ctx, id.with("press"), if pressed { -depth } else { 0.0 }).0
}

/// Advances a spring by `elapsed` seconds in fixed small steps, so it stays
/// stable at any frame rate.
fn spring_step(mut s: Spring, target: f32, mut elapsed: f32, damping: f32) -> Spring {
    while elapsed > 0.0 {
        let dt = elapsed.min(1.0 / 240.0);
        let accel = -SPRING_STIFFNESS * (s.value - target) - damping * s.velocity;
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
const REGULAR_FAMILY: &str = "regular";

/// Fonts (both SIL Open Font License, licences in `assets/fonts/`): Afacad Flux
/// Bold for all text, and Pacifico only for the "Hermes" wordmark.
pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::empty();
    let text = "afacad_flux_bold".to_string();
    let regular = "afacad_flux_regular".to_string();
    let wordmark = "pacifico".to_string();
    let bold = FontData::from_static(include_bytes!("../assets/fonts/AfacadFlux-Bold.ttf"));
    let regular_data = FontData::from_static(include_bytes!("../assets/fonts/AfacadFlux-Regular.ttf"));
    let pacifico = FontData::from_static(include_bytes!("../assets/fonts/Pacifico-Regular.ttf"));
    fonts.font_data.insert(text.clone(), std::sync::Arc::new(bold));
    fonts.font_data.insert(regular.clone(), std::sync::Arc::new(regular_data));
    fonts.font_data.insert(wordmark.clone(), std::sync::Arc::new(pacifico));
    fonts.families.insert(FontFamily::Name(REGULAR_FAMILY.into()), vec![regular, text.clone()]);
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

/// Afacad Flux Regular, for secondary text (a profile's version under its name).
pub fn regular_font(size: f32) -> FontId {
    FontId::new(size * TEXT_SCALE, FontFamily::Name(REGULAR_FAMILY.into()))
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
        // Text fields: gold selection and caret instead of egui's blue.
        v.selection.bg_fill = TEXT_SELECTION;
        v.selection.stroke = Stroke::new(1.0, TEXT);
        v.text_cursor.stroke = Stroke::new(2.0, PLAY);
        v.hyperlink_color = PLAY;
        // Tooltips and any egui-drawn text follow the same scale as the app's own text.
        for font in style.text_styles.values_mut() {
            font.size *= TEXT_SCALE;
        }
    });
    // egui stops treating a press as a press after 0.8 s, which made held
    // buttons spring back while still held (owner request: they stay pressed
    // for as long as the button is held).
    ctx.options_mut(|o| o.input_options.max_click_duration = f64::INFINITY);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_shrinks_then_springs_back_past_full_size_and_settles() {
        // Held: settles at the pressed size.
        let held = spring_step(Spring::default(), -PRESS_DEPTH, 0.5, SPRING_DAMPING);
        assert!((held.value + PRESS_DEPTH).abs() < 1e-3);
        // Released: overshoots slightly above full size, then settles at it.
        let mut s = held;
        let mut peak = f32::MIN;
        for _ in 0..60 {
            s = spring_step(s, 0.0, 1.0 / 120.0, SPRING_DAMPING);
            peak = peak.max(s.value);
        }
        // A small bounce: 5-16% of the distance travelled.
        let bounce = peak / PRESS_DEPTH;
        assert!(bounce > 0.05 && bounce < 0.16, "bounce {bounce}");
        assert!(spring_step(s, 0.0, 1.0, SPRING_DAMPING).value.abs() < 1e-3);
        // A quick click (80 ms) still visibly shrinks.
        let quick = spring_step(Spring::default(), -PRESS_DEPTH, 0.08, SPRING_DAMPING);
        assert!(quick.value < -PRESS_DEPTH * 0.5, "{}", quick.value);
    }

    #[test]
    fn fast_moves_toward_an_edge_predict_an_exit() {
        let menu = Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 700.0));
        let near_right_edge = pos2(250.0, 300.0);
        // Flicking right toward the edge: about to leave.
        assert!(heading_out(menu, near_right_edge, vec2(1500.0, 0.0)));
        // The same direction, but slowly: just moving within the menu.
        assert!(!heading_out(menu, near_right_edge, vec2(300.0, 0.0)));
        // Fast but along the menu (up/down): stays inside, keep it open.
        assert!(!heading_out(menu, pos2(150.0, 300.0), vec2(0.0, 1500.0)));
        // Fast toward the far edge from deep inside: lands inside, keep it open.
        assert!(!heading_out(menu, pos2(20.0, 300.0), vec2(1500.0, 0.0)));
        // A mostly vertical flick that would leave through the bottom: keep it open.
        assert!(!heading_out(menu, pos2(150.0, 680.0), vec2(400.0, 1500.0)));
        // Mostly horizontal but angled: still an exit.
        assert!(heading_out(menu, near_right_edge, vec2(1500.0, 900.0)));
    }

    #[test]
    fn calmer_and_unbouncy_springs() {
        let peak = |damping| {
            let (mut s, mut peak) = (Spring::default(), f32::MIN);
            for _ in 0..240 {
                s = spring_step(s, 1.0, 1.0 / 120.0, damping);
                peak = peak.max(s.value);
            }
            peak - 1.0
        };
        let (lively, calm, none) = (peak(Bounce::Lively.damping()), peak(Bounce::Calm.damping()), peak(Bounce::None.damping()));
        assert!(calm > 0.02 && calm < lively, "calm {calm}, lively {lively}");
        assert!(none < 1e-3, "no-bounce overshoot {none}");
    }

    #[test]
    fn follower_springs_trail_the_lead_and_settle_together() {
        // The sidebar's growth and logo chase the width on the same spring.
        let (mut lead, mut follow) = (Spring::default(), Spring::default());
        let mut trailed = false;
        for _ in 0..240 {
            lead = spring_step(lead, 1.0, 1.0 / 120.0, SPRING_DAMPING);
            follow = spring_step(follow, lead.value, 1.0 / 120.0, SPRING_DAMPING);
            trailed |= lead.value - follow.value > 0.1;
        }
        assert!(trailed, "the follower should visibly lag the lead");
        assert!((lead.value - 1.0).abs() < 1e-3 && (follow.value - 1.0).abs() < 1e-3);
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
        let open = sidebar_rect(sidebar, 1.0, 1.0);
        assert_eq!(open.min, main.min - Vec2::splat(SIDEBAR_OUTSET));
        assert_eq!(open.max.y, main.max.y + SIDEBAR_OUTSET);
        // ...and even with spring overshoot it stays inside the clip and the native window.
        let reach = sidebar_reach(sidebar);
        assert!(reach.contains_rect(sidebar_rect(sidebar, 1.15, 1.15)));
        assert!(area.contains_rect(reach.shrink(0.01)));
    }

    #[test]
    fn body_areas_fit_without_overlapping() {
        let area = Rect::from_min_size(pos2(0.0, 0.0), window_size());
        let (main, sidebar) = frame_layout(area);
        let body = body_layout(main, sidebar);
        for r in [body.content, body.wide] {
            assert!(main.contains_rect(r));
            assert!(!r.intersects(sidebar));
            assert_eq!(r.max.y, main.max.y - GAP);
        }
        assert!(body.content.min.y >= main.min.y + BAR_HEIGHT);
        // The content box is the right half of the space between the sidebar and the edge.
        let free_left = sidebar.max.x + GAP;
        assert!((body.content.min.x - (free_left + body.content.max.x) / 2.0).abs() < 0.01);
        assert_eq!(body.content.max.x, main.max.x - GAP);
    }
}
