//! The app's dropdown: a glass pill that expands in place into its options,
//! with the dropdown arrow morphing into a check mark on the selected option.

use eframe::egui::{
    self, Align2, Area, Color32, Context, CursorIcon, FontId, Id, Key, Order, Painter, Pos2, Rect, Sense, Shadow, Stroke,
    StrokeKind, emath, pos2, vec2,
};

use crate::theme;

pub struct Item<'a> {
    pub label: &'a str,
    /// Placeholder icon colour (e.g. the account head) until real images exist.
    pub icon: Color32,
}

const ANIMATION_SECONDS: f32 = 0.28;

fn open_id(id: Id) -> Id {
    id.with("open")
}

#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub fn set_open(ctx: &Context, id: Id, open: bool) {
    ctx.data_mut(|d| d.insert_temp(open_id(id), open));
}

/// Draws the dropdown over `anchor` (its collapsed size). Returns true when the selection changed.
pub fn show(ctx: &Context, id: Id, anchor: Rect, items: &[Item], selected: &mut usize) -> bool {
    let mut open = ctx.data(|d| d.get_temp::<bool>(open_id(id))).unwrap_or(false);
    let t = ctx.animate_bool_with_time_and_easing(id.with("anim"), open, ANIMATION_SECONDS, emath::easing::cubic_out);
    let row_h = anchor.height();
    let rows = items.len().max(1) as f32;
    let panel = Rect::from_min_size(anchor.min, vec2(anchor.width(), emath::lerp(row_h..=row_h * rows, t)));
    let rounding = (row_h / 2.0) as u8;
    let mut changed = false;

    Area::new(id).order(Order::Foreground).fixed_pos(anchor.min).show(ctx, |ui| {
        let response = ui.allocate_rect(panel, Sense::click());
        let painter = ui.painter().with_clip_rect(panel.expand(24.0));
        paint_glass(&painter, panel, rounding, t);
        let clip = painter.with_clip_rect(panel.shrink(1.0));

        for (i, item) in items.iter().enumerate() {
            // Collapsed, only the selected item shows, in the first row; expanded,
            // every item sits in its own row and the others fade in.
            let collapsed_y = anchor.min.y;
            let expanded_y = anchor.min.y + row_h * i as f32;
            let row = Rect::from_min_size(pos2(anchor.min.x, emath::lerp(collapsed_y..=expanded_y, t)), anchor.size());
            let alpha = if i == *selected { 1.0 } else { t };
            if alpha <= 0.01 {
                continue;
            }
            if open {
                let row_response = ui.interact(row, id.with(("row", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                if row_response.hovered() {
                    clip.rect_filled(row.shrink(3.0), theme::radius(rounding.saturating_sub(3)), theme::GLASS_HOVER);
                }
                if row_response.clicked() {
                    changed |= *selected != i;
                    *selected = i;
                    open = false;
                }
            }
            paint_row(&clip, row, item, alpha);
        }

        // The indicator rides with the selected row: arrow when collapsed, check when open.
        let selected_y = anchor.min.y + row_h * *selected as f32 * t;
        let indicator_row = Rect::from_min_size(pos2(anchor.min.x, selected_y), anchor.size());
        paint_indicator(&clip, indicator_row, t);

        if !open && t < 0.01 && response.clone().on_hover_cursor(CursorIcon::PointingHand).clicked() {
            open = true;
        }
        let pressed_outside = ui.input(|i| {
            i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !panel.contains(p))
        });
        if open && (pressed_outside || ui.input(|i| i.key_pressed(Key::Escape))) {
            open = false;
        }
    });

    ctx.data_mut(|d| d.insert_temp(open_id(id), open));
    changed
}

fn paint_glass(painter: &Painter, rect: Rect, rounding: u8, t: f32) {
    let shadow = Shadow {
        offset: [0, (2.0 + 6.0 * t) as i8],
        blur: (6.0 + 18.0 * t) as u8,
        spread: 0,
        color: Color32::from_black_alpha((50.0 + 70.0 * t) as u8),
    };
    painter.add(shadow.as_shape(rect, theme::radius(rounding)));
    painter.rect_filled(rect, theme::radius(rounding), theme::PANEL);
    painter.rect_filled(rect, theme::radius(rounding), theme::GLASS_TINT);
    painter.rect_stroke(rect, theme::radius(rounding), Stroke::new(1.0, theme::GLASS_EDGE), StrokeKind::Inside);
}

fn paint_row(painter: &Painter, row: Rect, item: &Item, alpha: f32) {
    let pad = row.height() * 0.16;
    let icon = Rect::from_min_size(row.min + vec2(pad * 1.6, pad), egui::Vec2::splat(row.height() - 2.0 * pad));
    painter.rect_filled(icon, theme::radius(6), item.icon.gamma_multiply(alpha));
    painter.text(
        pos2(icon.max.x + pad * 1.4, row.center().y),
        Align2::LEFT_CENTER,
        item.label,
        FontId::proportional(row.height() * 0.4),
        theme::TEXT.gamma_multiply(alpha),
    );
}

/// Morphs a down chevron (t = 0) into a check mark (t = 1).
fn paint_indicator(painter: &Painter, row: Rect, t: f32) {
    let s = row.height() * 0.17;
    let c = pos2(row.max.x - row.height() * 0.5, row.center().y);
    let arrow = [vec2(-s, -0.45 * s), vec2(0.0, 0.5 * s), vec2(s, -0.45 * s)];
    let check = [vec2(-s, 0.05 * s), vec2(-0.3 * s, 0.7 * s), vec2(s, -0.75 * s)];
    let points: Vec<Pos2> = arrow.iter().zip(check).map(|(a, b)| c + *a + (b - *a) * t).collect();
    painter.line(points, Stroke::new(2.2, theme::TEXT));
}
