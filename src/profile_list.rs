//! Profile cards: the lists on the Home and Profiles pages. Each card shows the
//! profile's icon (rounded; a "?" tile without one), its name and Minecraft
//! version, and a pin on the right. Hovering a card turns its icon into a Launch
//! button; clicking anywhere else on it opens the profile editor. The profile
//! being launched shows its progress, Stop or error in its own card.

use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, Id, Painter, Pos2, Rect, Sense, Stroke, TextureHandle};

use crate::lang::{tr, tr_args};
use crate::profiles::Profile;
use crate::theme;

/// What a card shows about launching.
pub enum Status {
    /// Not the profile being launched. `can_launch`: no other game is starting
    /// or running, so hovering offers Launch.
    Idle { can_launch: bool },
    /// Being installed or started: the step's text and, while downloading, how far.
    Working { text: String, progress: Option<f32> },
    /// Its game is running: the icon is a Stop button.
    Running,
    /// Its last launch failed (details in the tooltip); hovering offers Launch again.
    Failed(String),
}

pub struct Card<'a> {
    pub profile: &'a Profile,
    pub icon: Option<&'a TextureHandle>,
    pub status: Status,
}

/// What was clicked on a card.
pub enum Action {
    Open,
    Launch,
    Stop,
    TogglePin,
}

/// Draws `cards` top to bottom in `pane` and returns the click, if any.
/// `pin_icons`: the pin's outline and filled versions.
pub fn show(ui: &mut egui::Ui, pane: Rect, list_id: &str, cards: &[Card], pin_icons: &[TextureHandle; 2]) -> Option<(String, Action)> {
    let clip = pane.shrink(theme::STROKE_WIDTH);
    let painter = ui.painter().with_clip_rect(clip);
    let width = pane.width() - 2.0 * theme::GAP;
    let mut picked = None;
    for (i, card) in cards.iter().enumerate() {
        let y = pane.min.y + theme::GAP + i as f32 * (theme::CARD_HEIGHT + theme::CARD_GAP);
        let row = Rect::from_min_size(pos2(pane.min.x + theme::GAP, y), vec2(width, theme::CARD_HEIGHT));
        if row.min.y >= clip.max.y {
            break;
        }
        if let Some(action) = card_ui(ui, &painter, clip, row, Id::unique((list_id, &card.profile.id)), card, pin_icons) {
            picked = Some((card.profile.id.clone(), action));
        }
    }
    picked
}

fn card_ui(ui: &mut egui::Ui, painter: &Painter, clip: Rect, row: Rect, id: Id, card: &Card, pin_icons: &[TextureHandle; 2]) -> Option<Action> {
    let ctx = ui.ctx().clone();
    let info = &card.profile.info;
    let name = info.name.clone().unwrap_or_else(|| tr("profile.unnamed"));
    let minecraft = info.minecraft.clone().unwrap_or_else(|| crate::launch::MC_VERSION.to_string());
    let version = tr_args("profile.version", &[&minecraft]);
    let mut action = None;

    // The card itself (opens the editor); the icon button and the pin sit on top of it.
    let response = ui.interact(row.intersect(clip), id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
    let hovered = ui.rect_contains_pointer(row.intersect(clip));
    let pressed = response.is_pointer_button_down_on();
    // Pressing shrinks the whole card, its contents included (owner request).
    let s = theme::press_scale_sized(&ctx, id, pressed, row.width());
    let sc = |r: Rect| theme::scale_about(r, row.center(), s);
    let sp = |p: Pos2| row.center() + (p - row.center()) * s;
    let drawn = sc(row);
    if response.clicked() {
        action = Some(Action::Open);
    }

    let radius = theme::radius(theme::CONTROL_RADIUS);
    painter.rect_filled(drawn, radius, theme::GLASS_TINT);
    if hovered && pressed {
        painter.rect_filled(drawn, radius, theme::GLASS_PRESS);
    }
    // No outline at rest (owner request); the profile being launched or played
    // has a gold (accent) one.
    let active = matches!(card.status, Status::Working { .. } | Status::Running);
    let lit = theme::selected_amount(&ctx, id, active);
    if lit > 0.0 {
        painter.rect_stroke(drawn, radius, Stroke::new(theme::STROKE_WIDTH + lit, theme::PLAY.gamma_multiply(lit)), egui::StrokeKind::Inside);
    }
    // Hovered cards get the sidebar's selection box over their edge (owner request).
    let selection = ctx.animate_bool_with_time(id.with("selector"), hovered, 0.12);
    theme::selector(painter, drawn, theme::CONTROL_RADIUS, selection);
    if response.has_focus() {
        theme::focus_ring(painter, drawn, theme::CONTROL_RADIUS);
    }

    // Icon, or the Launch / Stop button in its place.
    let pad = (theme::CARD_HEIGHT - theme::CARD_ICON_SIZE) / 2.0;
    let tile = Rect::from_min_size(row.min + vec2(pad, pad), egui::Vec2::splat(theme::CARD_ICON_SIZE));
    let tile_id = id.with("tile");
    let launchable = match &card.status {
        Status::Idle { can_launch } => *can_launch,
        Status::Failed(_) => true,
        _ => false,
    };
    let running = matches!(card.status, Status::Running);
    let button = (hovered && launchable) || running;
    let tile_response = button.then(|| {
        let key = if running { "status.stop" } else { "play" };
        let r = ui.interact(tile.intersect(clip), tile_id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr(key)));
        r
    });
    let tile_pressed = tile_response.as_ref().is_some_and(|r| r.is_pointer_button_down_on());
    let tile_shown = sc(tile);
    let tile_drawn = theme::scale_about(tile_shown, tile_shown.center(), theme::press_scale(&ctx, tile_id, tile_pressed));
    let corners = theme::radius(theme::SMALL_RADIUS);
    let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    match (&tile_response, running) {
        (Some(r), true) => {
            let fill = if tile_pressed { theme::STOP.lerp_to_gamma(theme::BG, 0.2) } else if r.hovered() { theme::STOP.lerp_to_gamma(Color32::WHITE, 0.08) } else { theme::STOP };
            painter.rect_filled(tile_drawn, corners, fill);
            painter.rect_filled(Rect::from_center_size(tile_drawn.center(), egui::Vec2::splat(12.0 * s)), theme::radius(2), Color32::WHITE);
            if r.clicked() {
                action = Some(Action::Stop);
            }
        }
        (Some(r), false) => {
            let fill = if tile_pressed { theme::PLAY.lerp_to_gamma(theme::BG, 0.15) } else if r.hovered() { theme::PLAY.lerp_to_gamma(Color32::WHITE, 0.12) } else { theme::PLAY };
            painter.rect_filled(tile_drawn, corners, fill);
            play_icon(painter, tile_drawn.center(), s, theme::PLAY_TEXT);
            if r.clicked() {
                action = Some(Action::Launch);
            }
        }
        (None, _) => {
            match card.icon {
                Some(texture) => {
                    painter.add(egui::epaint::RectShape::filled(tile_shown, corners, Color32::WHITE).with_texture(texture.id(), full));
                }
                None => {
                    painter.rect_filled(tile_shown, corners, theme::GLASS_HOVER);
                    painter.text(tile_shown.center(), Align2::CENTER_CENTER, "?", theme::text_font(22.0 * s), theme::TEXT.gamma_multiply(0.6));
                }
            }
            if let Status::Working { progress, .. } = &card.status {
                painter.rect_filled(tile_shown, corners, Color32::from_black_alpha(120));
                spinner(painter, tile_shown.center(), 11.0 * s, *progress, ui.input(|i| i.time) as f32, theme::PLAY);
                ctx.request_repaint();
            }
        }
    }

    // Pin (right).
    let pin = Rect::from_center_size(pos2(row.max.x - pad - 16.0, row.center().y), egui::Vec2::splat(32.0));
    let pin_id = id.with("pin");
    let pin_response = ui.interact(pin.intersect(clip), pin_id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    pin_response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, info.pinned, tr("profile.pin")));
    if pin_response.hovered() {
        painter.rect_filled(sc(pin), theme::radius(theme::SMALL_RADIUS), theme::GLASS_HOVER);
    }
    let pin_scale = theme::press_scale(&ctx, pin_id, pin_response.is_pointer_button_down_on()) * s;
    let shown = info.pinned || hovered;
    if shown {
        let color = if info.pinned { theme::TEXT } else { theme::TEXT.gamma_multiply(0.6) };
        pin_icon(painter, sp(pin.center()), pin_scale, pin_icons, info.pinned, color);
    }
    if pin_response.clicked() {
        action = Some(Action::TogglePin);
    }

    // Name (bold) with the version, or the launch status, under it (regular).
    let text_x = tile.max.x + 14.0;
    let text_w = (pin.min.x - 12.0 - text_x).max(0.0);
    let line = |text: String, font: egui::FontId, color: Color32, y: f32| {
        let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
        job.wrap = egui::text::TextWrapping::truncate_at_width(text_w * s);
        let galley = painter.layout_job(job);
        painter.galley(Align2::LEFT_CENTER.anchor_size(sp(pos2(text_x, y)), galley.size()).min, galley, color);
    };
    line(name, theme::text_font(16.0 * s), theme::TEXT, row.center().y - 10.0);
    let detail = theme::TEXT.gamma_multiply(0.65);
    let (second, color) = match &card.status {
        Status::Working { text, .. } => (text.clone(), detail),
        Status::Failed(_) => (tr("status.failed"), theme::DANGER_TEXT),
        _ => (version, detail),
    };
    line(second, theme::regular_font(14.0 * s), color, row.center().y + 11.0);
    if let Status::Failed(error) = &card.status {
        response.on_hover_text(tr_args("status.failed.details", &[error]));
    }
    action
}

/// A rounded play triangle, slightly right of centre so it looks centred,
/// `scale` times its usual size.
pub(crate) fn play_icon(painter: &Painter, c: Pos2, scale: f32, color: Color32) {
    let s = 8.5 * scale;
    let corners = [c + vec2(-0.65 * s, -s), c + vec2(1.05 * s, 0.0), c + vec2(-0.65 * s, s)];
    // Well-rounded corners (owner request): each corner is a curve between points
    // a third of the way along its two sides.
    let mut points = Vec::new();
    for i in 0..3 {
        let (prev, at, next) = (corners[(i + 2) % 3], corners[i], corners[(i + 1) % 3]);
        let a = at + (prev - at) * 0.34;
        let b = at + (next - at) * 0.34;
        for k in 0..=8 {
            let t = k as f32 / 8.0;
            // Quadratic Bezier from a to b, pulled towards the corner.
            let p = a.to_vec2() * (1.0 - t) * (1.0 - t) + at.to_vec2() * 2.0 * t * (1.0 - t) + b.to_vec2() * t * t;
            points.push(p.to_pos2());
        }
    }
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

/// The pin (owner artwork): `icons` are the outline and filled versions;
/// filled when pinned. Drawn `PIN_SIZE` points wide, times `scale`.
fn pin_icon(painter: &Painter, c: Pos2, scale: f32, icons: &[TextureHandle; 2], filled: bool, color: Color32) {
    let rect = Rect::from_center_size(c, egui::Vec2::splat(PIN_SIZE * scale));
    painter.image(icons[filled as usize].id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), color);
}

/// How big the pin icon is drawn.
const PIN_SIZE: f32 = 20.0;

/// A ring that fills with `progress` (0..1), or a turning arc while it's unknown.
pub(crate) fn spinner(painter: &Painter, c: Pos2, r: f32, progress: Option<f32>, time: f32, color: Color32) {
    use std::f32::consts::TAU;
    painter.circle_stroke(c, r, Stroke::new(3.0, Color32::from_white_alpha(50)));
    let (start, sweep) = match progress {
        Some(p) => (-TAU / 4.0, TAU * p.clamp(0.0, 1.0)),
        None => (time * 5.0, TAU * 0.3),
    };
    let steps = 32;
    let points: Vec<Pos2> = (0..=steps)
        .map(|k| {
            let a = start + sweep * k as f32 / steps as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(3.0, color)));
}
