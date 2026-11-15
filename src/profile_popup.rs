//! The profile popup, for creating a profile (Profiles page's Create button) or
//! editing one (clicking its card): a liquid-glass card over the dimmed window
//! with the profile's icon (drop an image on the popup or click to browse), its
//! name, its Minecraft version, the most RAM its game may use (a slider) and a
//! Create / Save button; when editing, a
//! Delete button that asks first. The card is opaque. Everything behind it is
//! darkened and can't be clicked until it closes (Esc or its close button).

use std::path::PathBuf;
use std::sync::mpsc;

use eframe::egui::{self, pos2, vec2, Align2, Context, CursorIcon, Id, LayerId, Order, Rect, Sense, TextureHandle};

use crate::lang::{tr, tr_args};
use crate::{dropdown, profiles, theme};

/// Minecraft versions a profile can use. Only one for now.
const VERSIONS: &[&str] = &[crate::launch::MC_VERSION];

/// The max-RAM slider moves in half-gigabyte steps, from 1 GB up to the
/// installed RAM.
const MEMORY_STEP_MB: u32 = 512;
const MEMORY_MIN_MB: u32 = 1024;
/// The slider's top end when Windows can't say how much RAM there is.
const MEMORY_FALLBACK_MB: u32 = 16384;

const POPUP_ID: &str = "create_profile";
const VERSION_DROPDOWN: &str = "create_profile_version";

/// What the popup asks the app to do this frame.
pub enum Action {
    None,
    Close,
    /// Create the profile, or save the edited one.
    Save,
    /// Delete the edited profile (confirmed).
    Delete,
}

/// What `Action::Save` should write.
pub struct Request<'a> {
    /// `None` when left empty (an unnamed profile).
    pub name: Option<String>,
    pub minecraft: &'static str,
    /// Most RAM the game may use, in MB.
    pub max_memory_mb: u32,
    /// A newly chosen icon, if any.
    pub icon: Option<&'a image::RgbaImage>,
}

pub struct ProfilePopup {
    /// The profile being edited; `None` when creating one.
    pub editing: Option<String>,
    name: String,
    /// The icon shown: a newly chosen image (cropped and sized) with its
    /// preview texture, or the profile's current icon (no image to save).
    icon: Option<(Option<image::RgbaImage>, TextureHandle)>,
    version: usize,
    /// The max-RAM slider's value and top end (installed RAM), in MB.
    memory_mb: u32,
    memory_max_mb: u32,
    /// An open file dialog, running on its own thread so the window keeps drawing.
    picking: Option<mpsc::Receiver<Option<PathBuf>>>,
    /// Shown above the Create button: why the icon or the creation failed.
    pub error: Option<String>,
    opened: bool,
    /// Delete was clicked: the footer asks to confirm.
    confirming_delete: bool,
    /// Set by the app each frame: false while the profile's game is starting or
    /// running (its folder is in use).
    pub can_delete: bool,
}

impl ProfilePopup {
    /// For creating a new profile.
    pub fn create() -> Self {
        let installed = match crate::launch::system_memory_mb() {
            0 => MEMORY_FALLBACK_MB,
            mb => mb,
        };
        let memory_max_mb = (installed / MEMORY_STEP_MB * MEMORY_STEP_MB).max(MEMORY_MIN_MB);
        ProfilePopup {
            editing: None,
            name: String::new(),
            icon: None,
            version: 0,
            memory_mb: crate::launch::DEFAULT_MEMORY_MB.clamp(MEMORY_MIN_MB, memory_max_mb),
            memory_max_mb,
            picking: None,
            error: None,
            opened: false,
            confirming_delete: false,
            can_delete: true,
        }
    }

    /// For editing `profile`, filled in with its details and current icon.
    pub fn edit(profile: &profiles::Profile, icon: Option<&TextureHandle>) -> Self {
        let version = profile.info.minecraft.as_deref().and_then(|v| VERSIONS.iter().position(|x| *x == v)).unwrap_or(0);
        let popup = Self::create();
        let memory = profile.info.max_memory_mb.unwrap_or(crate::launch::DEFAULT_MEMORY_MB);
        ProfilePopup {
            editing: Some(profile.id.clone()),
            name: profile.info.name.clone().unwrap_or_default(),
            icon: icon.map(|t| (None, t.clone())),
            version,
            memory_mb: memory.clamp(MEMORY_MIN_MB, popup.memory_max_mb),
            ..popup
        }
    }

    pub fn request(&self) -> Request<'_> {
        Request {
            name: Some(self.name.trim().to_string()).filter(|n| !n.is_empty()),
            minecraft: VERSIONS[self.version],
            max_memory_mb: self.memory_mb,
            icon: self.icon.as_ref().and_then(|(img, _)| img.as_ref()),
        }
    }

    fn set_icon(&mut self, ctx: &Context, path: &std::path::Path) {
        match profiles::icon_from_file(path) {
            Ok(img) => {
                let size = [img.width() as usize, img.height() as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                let texture = ctx.load_texture("create-profile-icon", color, egui::TextureOptions::LINEAR);
                self.icon = Some((Some(img), texture));
                self.error = None;
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    /// Draws the popup centred over `window`.
    pub fn show(&mut self, ctx: &Context, window: Rect) -> Action {
        // A file chosen in the dialog, or one dropped anywhere on the window.
        if let Some(rx) = &self.picking {
            match rx.try_recv() {
                Ok(path) => {
                    self.picking = None;
                    if let Some(path) = path {
                        self.set_icon(ctx, &path);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
                Err(mpsc::TryRecvError::Disconnected) => self.picking = None,
            }
        }
        let dropped = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()));
        if let Some(path) = dropped {
            self.set_icon(ctx, &path);
        }
        let dragging_file = ctx.input(|i| !i.raw.hovered_files.is_empty());

        let mut action = Action::None;
        let popup = Rect::from_center_size(window.center(), theme::POPUP_SIZE);
        // Grows in on the shared spring, like the dropdowns.
        let grow = theme::spring_motion(ctx, Id::unique(POPUP_ID).with("grow"), 1.0).value;
        let dim = ctx.animate_bool_with_time(Id::unique(POPUP_ID).with("dim"), true, 0.15);
        let area = egui::Area::new(Id::unique(POPUP_ID))
            .kind(egui::UiKind::Modal)
            .order(Order::Foreground)
            .fixed_pos(window.min)
            .interactable(true);
        egui::Modal::new(Id::unique(POPUP_ID)).area(area).backdrop_color(egui::Color32::TRANSPARENT).frame(egui::Frame::NONE).show(ctx, |ui| {
            // Darken the window (only the window: around it is transparent).
            let painter = ui.painter().with_clip_rect(ctx.content_rect());
            painter.rect_filled(window, theme::radius(theme::WINDOW_RADIUS), egui::Color32::from_black_alpha((110.0 * dim) as u8));
            ui.allocate_rect(window, Sense::hover());

            let card = theme::scale_about(popup, popup.center(), 0.94 + 0.06 * grow);
            theme::popup_card(&painter, card);

            let inner = popup.shrink(theme::POPUP_PADDING);
            let label = |text: &str, at: egui::Pos2| {
                painter.text(at, Align2::LEFT_TOP, text, theme::regular_font(13.0), theme::TEXT.gamma_multiply(0.6));
            };

            // Close (top right).
            let close = Rect::from_min_size(pos2(popup.max.x - 14.0 - 32.0, popup.min.y + 14.0), vec2(32.0, 32.0));
            if crate::ui::bar_button(ui, close, "create.close", false, crate::ui::close_icon) {
                action = Action::Close;
            }

            // Icon (left): drop an image here, or click to browse.
            label(&tr("create.icon"), inner.min);
            let drop = Rect::from_min_size(inner.min + vec2(0.0, 22.0), egui::Vec2::splat(theme::CREATE_ICON_SIZE));
            let id = Id::unique("create_icon_drop");
            let response = ui.interact(drop, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr("create.icon")));
            let scale = theme::press_scale(ctx, id, response.is_pointer_button_down_on());
            let drop_drawn = theme::scale_about(drop, drop.center(), scale);
            let corners = theme::radius(theme::FIELD_RADIUS + 5);
            let emphasis = theme::selected_amount(ctx, id, response.hovered() || dragging_file);
            match &self.icon {
                Some((_, texture)) => {
                    painter.add(egui::epaint::RectShape::filled(drop_drawn, corners, egui::Color32::WHITE).with_texture(texture.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))));
                    painter.rect_stroke(drop_drawn, corners, egui::Stroke::new(1.5 * emphasis, theme::PLAY.gamma_multiply(emphasis)), egui::StrokeKind::Inside);
                }
                None => {
                    painter.rect_filled(drop_drawn, corners, theme::FIELD_FILL);
                    painter.rect_stroke(drop_drawn, corners, egui::Stroke::new(1.0 + 0.5 * emphasis, theme::FIELD_EDGE.lerp_to_gamma(theme::PLAY, emphasis)), egui::StrokeKind::Inside);
                    plus_icon(&painter, drop_drawn.center(), 11.0, theme::TEXT.gamma_multiply(0.55 + 0.45 * emphasis));
                }
            }
            if response.has_focus() {
                theme::focus_ring(&painter, drop_drawn, theme::FIELD_RADIUS + 5);
            }
            if response.clicked() && self.picking.is_none() {
                let (tx, rx) = mpsc::channel();
                let repaint = ctx.clone();
                std::thread::spawn(move || {
                    let file = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg"]).pick_file();
                    let _ = tx.send(file);
                    repaint.request_repaint();
                });
                self.picking = Some(rx);
            }

            // Name and Minecraft version (right).
            let fields_x = drop.max.x + theme::POPUP_PADDING;
            let field_w = inner.max.x - fields_x;
            label(&tr("create.name"), pos2(fields_x, inner.min.y));
            let name_rect = Rect::from_min_size(pos2(fields_x, inner.min.y + 22.0), vec2(field_w, theme::FIELD_HEIGHT));
            let edit_rect = name_rect.shrink2(vec2(14.0, 0.0));
            let edit = egui::TextEdit::singleline(&mut self.name)
                .id(Id::unique("create_name"))
                .frame(egui::Frame::NONE)
                .font(theme::text_font(16.0))
                .text_color(theme::TEXT)
                .char_limit(48)
                .align(Align2::LEFT_CENTER)
                .desired_width(edit_rect.width());
            let focus_id = Id::unique("create_name");
            let focused = ctx.memory(|m| m.has_focus(focus_id));
            theme::field(&painter, name_rect, theme::selected_amount(ctx, focus_id.with("field"), focused));
            let name_response = ui.put(edit_rect, edit);
            name_response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, tr("create.name")));
            if !self.opened {
                name_response.request_focus();
                self.opened = true;
            }

            let version_y = name_rect.max.y + 18.0;
            label(&tr("create.version"), pos2(fields_x, version_y));
            let version_rect = Rect::from_min_size(pos2(fields_x, version_y + 22.0), vec2(field_w, theme::ACCOUNT_SIZE.y));
            let items: Vec<dropdown::Item> = VERSIONS.iter().map(|v| dropdown::Item { label: v.to_string(), icon: None }).collect();
            if let Some(i) = dropdown::show(ctx, Id::unique(VERSION_DROPDOWN), version_rect, &items, self.version) {
                self.version = i;
            }

            // Max RAM (full width, under the icon): label left, value right, slider below.
            let memory_y = drop.max.y + 18.0;
            label(&tr("create.memory"), pos2(inner.min.x, memory_y));
            let value = tr_args("create.memory.value", &[&gigabytes(self.memory_mb)]);
            painter.text(pos2(inner.max.x, memory_y), Align2::RIGHT_TOP, value, theme::regular_font(13.0), theme::TEXT);
            let slider = Rect::from_min_size(pos2(inner.min.x, memory_y + 22.0), vec2(inner.width(), theme::SLIDER_HEIGHT));
            memory_slider(ui, slider, Id::unique("create_memory"), &mut self.memory_mb, MEMORY_MIN_MB..=self.memory_max_mb);

            // Footer: Create / Save at the right; when editing, Delete at the left,
            // which asks first (the question, Cancel and a red Delete take the footer).
            let button = theme::POPUP_BUTTON_SIZE;
            let confirm = Rect::from_min_size(inner.max - button, button);
            let footer_y = confirm.center().y;
            let mut error_left = inner.min.x;
            if self.confirming_delete {
                if crate::ui::solid_button(ui, confirm, Id::unique("profile_delete_confirm"), &tr("edit.delete"), theme::STOP, egui::Color32::WHITE, theme::FIELD_RADIUS) {
                    action = Action::Delete;
                }
                let cancel = confirm.translate(vec2(-(button.x + 10.0), 0.0));
                if text_button(ui, cancel, Id::unique("profile_delete_cancel"), &tr("edit.delete.cancel"), theme::TEXT) {
                    self.confirming_delete = false;
                }
                let mut job = egui::text::LayoutJob::simple(tr("edit.delete.confirm"), theme::text_font(15.0), theme::TEXT, cancel.min.x - inner.min.x - 12.0);
                job.wrap.max_rows = 2;
                let galley = painter.layout_job(job);
                painter.galley(pos2(inner.min.x, footer_y - galley.size().y / 2.0), galley, theme::TEXT);
            } else {
                let key = if self.editing.is_some() { "edit.save" } else { "create.confirm" };
                if crate::ui::solid_button(ui, confirm, Id::unique("create_confirm"), &tr(key), theme::PLAY, theme::PLAY_TEXT, theme::FIELD_RADIUS) {
                    action = Action::Save;
                }
                if self.editing.is_some() {
                    let delete = Rect::from_min_size(pos2(inner.min.x, confirm.min.y), button);
                    let enabled = self.can_delete;
                    if delete_button(ui, delete, enabled) {
                        self.confirming_delete = true;
                        self.error = None;
                    }
                    error_left = delete.max.x + 12.0;
                }
            }
            if let Some(error) = &self.error {
                let mut job = egui::text::LayoutJob::simple(error.clone(), theme::regular_font(13.0), theme::DANGER_TEXT, (confirm.min.x - error_left - 16.0).max(0.0));
                job.wrap.max_rows = 2;
                let galley = painter.layout_job(job);
                painter.galley(pos2(error_left, footer_y - galley.size().y / 2.0), galley, theme::DANGER_TEXT);
            }
        });

        // The version list opens over the popup.
        ctx.move_to_top(LayerId::new(Order::Foreground, Id::unique(VERSION_DROPDOWN)));
        let list_open = dropdown::is_open(ctx, Id::unique(VERSION_DROPDOWN));
        if !list_open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            // Esc backs out of the delete question first.
            if self.confirming_delete {
                self.confirming_delete = false;
            } else {
                action = Action::Close;
            }
        }
        action
    }
}

/// `mb` in gigabytes for the slider's value: "4", "4.5".
fn gigabytes(mb: u32) -> String {
    if mb % 1024 == 0 { (mb / 1024).to_string() } else { format!("{:.1}", mb as f32 / 1024.0) }
}

/// The max-RAM slider: a thin pill track, gold up to a white knob (like the
/// mod switches). Click or drag anywhere on it; when focused, the arrow keys
/// step it by `MEMORY_STEP_MB` and Home / End jump to the ends.
fn memory_slider(ui: &mut egui::Ui, rect: Rect, id: Id, value: &mut u32, range: std::ops::RangeInclusive<u32>) {
    let (lo, hi) = (*range.start(), (*range.end()).max(*range.start()));
    let response = ui.interact(rect, id, Sense::click_and_drag()).on_hover_cursor(CursorIcon::PointingHand);
    let knob_r = theme::SLIDER_KNOB_RADIUS;
    let track = Rect::from_min_max(
        pos2(rect.min.x + knob_r, rect.center().y - theme::SLIDER_TRACK / 2.0),
        pos2(rect.max.x - knob_r, rect.center().y + theme::SLIDER_TRACK / 2.0),
    );
    let snap = |mb: f32| ((mb / MEMORY_STEP_MB as f32).round() as u32 * MEMORY_STEP_MB).clamp(lo, hi);
    if let Some(p) = response.interact_pointer_pos() {
        let t = ((p.x - track.min.x) / track.width()).clamp(0.0, 1.0);
        *value = snap(lo as f32 + t * (hi - lo) as f32);
    }
    if response.has_focus() {
        // Arrow keys move the slider, not the keyboard focus.
        let filter = egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() };
        ui.memory_mut(|m| m.set_focus_lock_filter(id, filter));
        let pressed = |key| ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
        if pressed(egui::Key::ArrowLeft) || pressed(egui::Key::ArrowDown) {
            *value = value.saturating_sub(MEMORY_STEP_MB).clamp(lo, hi);
        }
        if pressed(egui::Key::ArrowRight) || pressed(egui::Key::ArrowUp) {
            *value = (*value + MEMORY_STEP_MB).clamp(lo, hi);
        }
        if pressed(egui::Key::Home) {
            *value = lo;
        }
        if pressed(egui::Key::End) {
            *value = hi;
        }
    }
    response.widget_info(|| egui::WidgetInfo::slider(true, *value as f64, tr("create.memory")));

    let t = if hi > lo { (*value - lo) as f32 / (hi - lo) as f32 } else { 0.0 };
    let t = ui.ctx().animate_value_with_time(id.with("at"), t, 0.08);
    let x = egui::lerp(track.min.x..=track.max.x, t);
    let active = response.hovered() || response.dragged();
    let grow = ui.ctx().animate_bool_with_time(id.with("grow"), active, 0.12);
    let painter = ui.painter();
    let corners = theme::radius((theme::SLIDER_TRACK / 2.0) as u8);
    painter.rect_filled(track, corners, if active { theme::GLASS_PRESS } else { theme::GLASS_HOVER });
    painter.rect_filled(Rect::from_min_max(track.min, pos2(x, track.max.y)), corners, theme::PLAY);
    let knob = pos2(x, rect.center().y);
    painter.circle_filled(knob, knob_r - 1.0 + grow, egui::Color32::WHITE);
    if response.has_focus() {
        theme::focus_ring(painter, Rect::from_center_size(knob, egui::Vec2::splat(knob_r * 2.0)), knob_r as u8);
    }
}

/// A borderless button: just its label, with a soft tint on hover (Cancel).
fn text_button(ui: &mut egui::Ui, rect: Rect, id: Id, label: &str, ink: egui::Color32) -> bool {
    let response = ui.interact(rect, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let pressed = response.is_pointer_button_down_on();
    let drawn = theme::scale_about(rect, rect.center(), theme::press_scale(ui.ctx(), id, pressed));
    let painter = ui.painter();
    let corners = theme::radius(theme::FIELD_RADIUS);
    if response.hovered() {
        painter.rect_filled(drawn, corners, if pressed { theme::GLASS_PRESS } else { theme::GLASS_HOVER });
    }
    painter.text(drawn.center(), Align2::CENTER_CENTER, label, theme::text_font(15.0), ink);
    if response.has_focus() {
        theme::focus_ring(painter, drawn, theme::FIELD_RADIUS);
    }
    response.clicked()
}

/// The editor's Delete: the same button as Save, in red (owner request); faded
/// and unclickable while the profile is in use.
fn delete_button(ui: &mut egui::Ui, rect: Rect, enabled: bool) -> bool {
    if enabled {
        return crate::ui::solid_button(ui, rect, Id::unique("profile_delete"), &tr("edit.delete"), theme::STOP, egui::Color32::WHITE, theme::FIELD_RADIUS);
    }
    let painter = ui.painter();
    painter.rect_filled(rect, theme::radius(theme::FIELD_RADIUS), theme::STOP.gamma_multiply(0.4));
    painter.text(rect.center(), Align2::CENTER_CENTER, tr("edit.delete"), theme::text_font(15.0), egui::Color32::WHITE.gamma_multiply(0.5));
    false
}

/// A "+" as thick as the app's icon stroke, `half` long each way from `center`.
/// The centre is snapped to the pixel grid and the bars are drawn as boxes, so
/// both arms of each bar come out the same length (an unsnapped centre made the
/// left arm stick out a pixel).
pub fn plus_icon(painter: &egui::Painter, center: egui::Pos2, half: f32, color: egui::Color32) {
    use egui::emath::GuiRounding;
    let c = center.round_to_pixels(painter.ctx().pixels_per_point());
    let w = theme::ICON_STROKE / 2.0;
    painter.rect_filled(Rect::from_min_max(c - vec2(half, w), c + vec2(half, w)), 0.0, color);
    painter.rect_filled(Rect::from_min_max(c - vec2(w, half), c + vec2(w, half)), 0.0, color);
}
