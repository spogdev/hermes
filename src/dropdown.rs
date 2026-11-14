//! The app's dropdown: a glass pill that expands in place into its options,
//! with the dropdown arrow morphing into a check mark on the selected option.

use eframe::egui::{
    Align2, Area, Color32, Context, CursorIcon, Id, Key, Order, Painter, Pos2, Rect, Role, Sense, Stroke,
    Vec2, WidgetInfo, pos2, vec2,
};

use eframe::egui::text::{LayoutJob, TextWrapping};

use crate::theme;

pub struct Item {
    pub label: String,
    /// Placeholder icon colour (the account head until skins are fetched).
    pub icon: Option<Color32>,
}



fn open_id(id: Id) -> Id {
    id.with("open")
}

/// Id of the collapsed pill's response (keyboard focus target).
pub fn pill_id(id: Id) -> Id {
    id.with("pill")
}

#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub fn set_open(ctx: &Context, id: Id, open: bool) {
    ctx.data_mut(|d| d.insert_temp(open_id(id), open));
}

/// Draws the dropdown over `anchor` (its collapsed size). Returns the index of
/// the option the user picked this frame, if any.
pub fn show(ctx: &Context, id: Id, anchor: Rect, items: &[Item], selected: usize) -> Option<usize> {
    let mut open = ctx.data(|d| d.get_temp::<bool>(open_id(id))).unwrap_or(false);
    // Opens and closes on the app's shared spring (like presses and the sidebar):
    // it overshoots slightly when opening; closing never shrinks below the pill.
    let t = theme::spring(ctx, id.with("expand"), if open { 1.0 } else { 0.0 }).0.max(0.0);
    let row_h = anchor.height();
    let rows = items.len().max(1);
    // The list grows downward from the pill.
    let expanded_top = anchor.min.y;
    let row_at = |k: usize| Rect::from_min_size(pos2(anchor.min.x, expanded_top + row_h * k as f32), anchor.size());
    let expanded = Rect::from_min_max(pos2(anchor.min.x, expanded_top), pos2(anchor.max.x, expanded_top + row_h * rows as f32));
    let panel = lerp_rect(anchor, expanded, t);
    let rounding = theme::CONTROL_RADIUS;
    let mut picked = None;

    Area::new(id).order(Order::Foreground).fixed_pos(panel.min).show(ctx, |ui| {
        ui.allocate_rect(panel, Sense::hover());
        let response = ui.interact(panel, pill_id(id), Sense::click());
        let label = items.get(selected).map_or("", |i| i.label.as_str());
        response.widget_info(|| WidgetInfo::labeled(Role::ComboBox, true, label));
        // Pressing the pill or any row shrinks the whole control; it springs back on release.
        // Only the drawing scales; hit areas stay put so the press can't slip off.
        let pressed = ui.input(|i| i.pointer.primary_down() && i.pointer.press_origin().is_some_and(|p| panel.contains(p)));
        let scale = theme::press_scale(ctx, id, pressed);
        let center = panel.center();
        let sc = |r: Rect| theme::scale_about(r, center, scale);
        let highlight = theme::selected_amount(ctx, id, open || response.hovered() || response.has_focus());
        theme::glass(&ui.painter().with_clip_rect(sc(panel).expand(24.0)), sc(panel), theme::radius(rounding), t, RESTING_SHADOW, highlight, theme::Edge::Solid);
        let clip = ui.painter().with_clip_rect(sc(panel).shrink(1.0));
        if !open && (response.hovered() || response.is_pointer_button_down_on()) {
            let tint = if response.is_pointer_button_down_on() { theme::GLASS_PRESS } else { theme::GLASS_HOVER };
            clip.rect_filled(sc(panel), theme::radius(rounding), tint);
        }

        // Collapsed, only the selected option shows, in the pill; expanded, every
        // row slides to its place in the list and the others fade in.
        for (k, item) in items.iter().enumerate() {
            let row = lerp_rect(anchor, row_at(k), t);
            let alpha = if k == selected { 1.0 } else { t.min(1.0) };
            if alpha <= 0.01 {
                continue;
            }
            if open {
                let hit = ui.interact(row, id.with(("row", k)), Sense::click());
                hit.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, true, k == selected, &item.label));
                if hit.hovered() || hit.is_pointer_button_down_on() {
                    let tint = if hit.is_pointer_button_down_on() { theme::GLASS_PRESS } else { theme::GLASS_HOVER };
                    clip.rect_filled(sc(row).shrink(3.0), theme::radius(theme::SMALL_RADIUS), tint);
                }
                if hit.has_focus() {
                    theme::focus_ring(&clip, sc(row).shrink(4.0), theme::SMALL_RADIUS);
                }
                if hit.clicked() {
                    picked = Some(k);
                    open = false;
                }
            }
            paint_row(&clip, sc(row), item, alpha);
        }

        // The indicator rides with the selected row: arrow when collapsed, check when open.
        paint_indicator(&clip, sc(lerp_rect(anchor, row_at(selected), t)), t);

        if !open && response.has_focus() {
            theme::focus_ring(ui.painter(), sc(panel), rounding);
        }
        // One pointer cursor over the whole dropdown, in every state: open, closed,
        // mid-animation and between rows, so it never flickers back to the arrow.
        if ui.input(|i| i.pointer.hover_pos().is_some_and(|p| panel.contains(p))) {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if !open && t <= 0.0 && response.clicked() {
            open = true;
        }
        let pressed_outside =
            ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !panel.contains(p)));
        if open && (pressed_outside || ui.input(|i| i.key_pressed(Key::Escape))) {
            open = false;
        }
    });

    ctx.data_mut(|d| d.insert_temp(open_id(id), open));
    picked
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::from_min_max(a.min.lerp(b.min, t), a.max.lerp(b.max, t))
}

fn paint_row(painter: &Painter, row: Rect, item: &Item, alpha: f32) {
    let pad = row.height() * 0.16;
    let text_x = match item.icon {
        None => row.min.x + pad * 2.4,
        Some(icon) => {
            let rect = Rect::from_min_size(row.min + vec2(pad * 1.6, pad), Vec2::splat(row.height() - 2.0 * pad));
            painter.rect_filled(rect, theme::radius(theme::SMALL_RADIUS), icon.gamma_multiply(alpha));
            rect.max.x + pad * 1.4
        }
    };
    // Truncate with an ellipsis before the arrow/check column.
    let color = theme::TEXT.gamma_multiply(alpha);
    let mut job = LayoutJob::simple_singleline(item.label.clone(), theme::text_font((row.height() * 0.4).min(15.0)), color);
    job.wrap = TextWrapping::truncate_at_width((indicator_center(row).x - INDICATOR_COLUMN / 2.0 - text_x).max(0.0));
    let galley = painter.layout_job(job);
    let pos = Align2::LEFT_CENTER.anchor_size(pos2(text_x, row.center().y), galley.size()).min;
    painter.galley(pos, galley, color);
}

/// Width reserved at the right of each row for the arrow / check mark.
const INDICATOR_COLUMN: f32 = 30.0;
/// Collapsed dropdowns keep a faint shadow so they read as raised controls.
const RESTING_SHADOW: u8 = 50;

fn indicator_center(row: Rect) -> Pos2 {
    pos2(row.max.x - row.height().min(37.0) * 0.5, row.center().y)
}

/// Morphs a down chevron (t = 0) into a check mark (t = 1).
fn paint_indicator(painter: &Painter, row: Rect, t: f32) {
    let s = (row.height() * 0.17).min(6.5);
    let c = indicator_center(row);
    let arrow = [vec2(-s, -0.45 * s), vec2(0.0, 0.5 * s), vec2(s, -0.45 * s)];
    let check = [vec2(-s, 0.05 * s), vec2(-0.3 * s, 0.7 * s), vec2(s, -0.75 * s)];
    let points: Vec<Pos2> = arrow.iter().zip(check).map(|(a, b)| c + *a + (b - *a) * t).collect();
    painter.line(points, Stroke::new(theme::ICON_STROKE, theme::TEXT));
}
