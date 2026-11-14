//! Main launcher window (design v1, see `app-design.md`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{
    self, Align2, Context, CursorIcon, Id, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions,
    ViewportCommand, pos2, vec2,
};

use crate::dropdown;
use crate::lang::{tr, tr_args};
use crate::launch::{self, Step};
use crate::profiles::Profiles;
use crate::theme;
use crate::{game_window, window_shape};

/// Developer options for visual checks (`--screenshot <png>`, `--open-dropdown account`,
/// `--state downloading|checking|failed`, `--focus play|account`).
#[derive(Default)]
pub struct DevOptions {
    pub screenshot: Option<PathBuf>,
    pub open_dropdown: Option<String>,
    pub state: Option<String>,
    pub focus: Option<String>,
    pub open_sidebar: bool,
    /// Frame to capture with `--screenshot` (default 45, once animations have settled).
    pub screenshot_frame: Option<u32>,
}

#[derive(Clone, Copy, PartialEq)]
enum PlayLook {
    Ready,
    Busy,
    Stop,
    Failed,
}

enum Phase {
    Idle,
    Working(Option<Step>),
    /// The game is running. `stopping`: the player asked it to close.
    Running { child: Child, stopping: bool },
    Failed(String),
}

struct HermesApp {
    phase: Arc<Mutex<Phase>>,
    logo: TextureHandle,
    nav_icons: NavIcons,
    accounts: Vec<String>,
    selected_account: usize,
    /// Data folder; `None` if it couldn't be created (the error is shown on Play).
    root: Option<PathBuf>,
    profiles: Profiles,
    /// Loaded profile icons by profile id (profiles without a readable PNG have none).
    profile_icons: HashMap<String, TextureHandle>,
    /// Physical window size the rounded window region was last built for.
    region_size: Option<[i32; 2]>,
    /// Size (points) of the drawing area Windows actually shows; see `window_shape`.
    visible: Option<egui::Vec2>,
    screenshot: Option<PathBuf>,
    frames: u32,
    /// Developer focus target, applied once the widgets exist (areas skip their first frame).
    focus: Option<Id>,
    /// Developer option: keep the sidebar expanded.
    force_sidebar: bool,
    screenshot_frame: u32,
}

const ACCOUNT_DROPDOWN: &str = "account_dropdown";

impl HermesApp {
    fn new(ctx: &Context, dev: DevOptions) -> Self {
        match dev.open_dropdown.as_deref() {
            Some("account") => dropdown::set_open(ctx, Id::unique(ACCOUNT_DROPDOWN), true),
            _ => {}
        }
        theme::apply_style(ctx);
        theme::install_fonts(ctx);
        let loaded = launch::Paths::new().and_then(|paths| Ok((Profiles::load(&paths.root)?, paths.root)));
        let (profiles, root, mut phase) = match loaded {
            Ok((profiles, root)) => (profiles, Some(root), Phase::Idle),
            // No data folder: no profiles to show, and Play shows the error.
            Err(e) => (Profiles { selected: String::new(), profiles: Vec::new() }, None, Phase::Failed(format!("{e:#}"))),
        };
        match dev.state.as_deref() {
            Some("downloading") => phase = Phase::Working(Some(Step::Downloading { done: 2100, total: 5247 })),
            Some("checking") => phase = Phase::Working(Some(Step::Checking)),
            Some("failed") => phase = Phase::Failed("example error".into()),
            // A harmless short-lived process stands in for the game.
            Some("running") => {
                if let Ok(child) = std::process::Command::new("ping").args(["-n", "20", "127.0.0.1"]).stdout(std::process::Stdio::null()).spawn() {
                    phase = Phase::Running { child, stopping: false };
                }
            }
            _ => {}
        }
        let focus = match dev.focus.as_deref() {
            Some("play") => Some(Id::unique("play")),
            Some("account") => Some(dropdown::pill_id(Id::unique(ACCOUNT_DROPDOWN))),
            _ => None,
        };
        let profile_icons = root.as_deref().map(|r| load_profile_icons(ctx, r, &profiles)).unwrap_or_default();
        HermesApp {
            force_sidebar: dev.open_sidebar,
            screenshot_frame: dev.screenshot_frame.unwrap_or(45),
            focus,
            profile_icons,
            phase: Arc::new(Mutex::new(phase)),
            logo: load_png(ctx, "logo", include_bytes!("../assets/images/logo.png")),
            nav_icons: NavIcons {
                home: load_png(ctx, "icon_home", include_bytes!("../assets/images/icon_home.png")),
                home_filled: load_png(ctx, "icon_home_filled", include_bytes!("../assets/images/icon_home_filled.png")),
                settings: load_png(ctx, "icon_settings", include_bytes!("../assets/images/icon_settings.png")),
                settings_filled: load_png(ctx, "icon_settings_filled", include_bytes!("../assets/images/icon_settings_filled.png")),
            },
            // Offline mode has one account; Microsoft accounts will fill this list.
            accounts: vec![launch::PLAYER_NAME.to_string()],
            selected_account: 0,
            root,
            profiles,
            region_size: None,
            visible: None,
            screenshot: dev.screenshot,
            frames: 0,
        }
    }

    /// Selects the profile Launch starts and remembers it.
    fn select_profile(&mut self, id: &str) {
        let saved = match &self.root {
            Some(root) => self.profiles.select(root, id),
            None => Ok(()),
        };
        if let Err(e) = saved {
            *self.phase.lock().unwrap() = Phase::Failed(format!("{e:#}"));
        }
    }

    /// Home screen (the only screen so far). The right pane lists the pinned
    /// profiles; clicking one selects it for Launch. The selected profile's row
    /// has the bright selected outline. Profiles are pinned in their
    /// `profile.json` (`"pinned": true`) until the launcher has a way to pin them.
    fn home(&mut self, ui: &mut egui::Ui, pane: Rect) {
        let clip = pane.shrink(theme::STROKE_WIDTH);
        let painter = ui.painter().with_clip_rect(clip);
        let width = pane.width() - 2.0 * theme::GAP;
        let mut clicked = None;
        let pinned = self.profiles.profiles.iter().filter(|p| p.info.pinned);
        for (i, profile) in pinned.enumerate() {
            let y = pane.min.y + theme::GAP + i as f32 * (theme::PINNED_ROW_HEIGHT + theme::PINNED_ROW_GAP);
            let row = Rect::from_min_size(pos2(pane.min.x + theme::GAP, y), vec2(width, theme::PINNED_ROW_HEIGHT));
            if row.min.y >= clip.max.y {
                break;
            }
            let label = profile.info.name.clone().unwrap_or_else(|| tr("profile.unnamed"));
            let selected = profile.id == self.profiles.selected;
            let id = Id::unique(("pinned_profile", &profile.id));
            let response = ui.interact(row.intersect(clip), id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListBoxOption, true, selected, &label));
            let pressed = response.is_pointer_button_down_on();
            let row = theme::scale_about(row, row.center(), theme::press_scale(ui.ctx(), id, pressed));

            // Same look as the dropdowns: glass tint, solid outline that turns
            // the bright Launch blue for the selected profile.
            let radius = theme::radius(theme::CONTROL_RADIUS);
            painter.rect_filled(row, radius, theme::GLASS_TINT);
            if response.hovered() {
                painter.rect_filled(row, radius, if pressed { theme::GLASS_PRESS } else { theme::GLASS_HOVER });
            }
            let emphasis = theme::selected_amount(ui.ctx(), id, selected);
            theme::selectable_outline(&painter, row, radius, emphasis, 0.0, theme::Edge::Solid);
            if response.has_focus() {
                theme::focus_ring(&painter, row, theme::CONTROL_RADIUS);
            }

            let pad = (row.height() - theme::PINNED_ICON_SIZE) / 2.0;
            let mut text_x = row.min.x + pad + 4.0;
            if let Some(texture) = self.profile_icons.get(&profile.id) {
                let icon = Rect::from_min_size(pos2(row.min.x + pad + 4.0, row.min.y + pad), egui::Vec2::splat(theme::PINNED_ICON_SIZE));
                let shape = egui::epaint::RectShape::filled(icon, theme::radius(theme::SMALL_RADIUS), egui::Color32::WHITE)
                    .with_texture(texture.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)));
                painter.add(shape);
                text_x = icon.max.x + 12.0;
            }
            let mut job = egui::text::LayoutJob::simple_singleline(label, theme::text_font(16.0), theme::TEXT);
            job.wrap = egui::text::TextWrapping::truncate_at_width((row.max.x - 16.0 - text_x).max(0.0));
            let galley = painter.layout_job(job);
            painter.galley(Align2::LEFT_CENTER.anchor_size(pos2(text_x, row.center().y), galley.size()).min, galley, theme::TEXT);

            if response.clicked() {
                clicked = Some(profile.id.clone());
            }
        }
        if let Some(id) = clicked {
            self.select_profile(&id);
        }
    }

    /// Clips the window to the window outline plus everything the hovered sidebar
    /// can grow into (rebuilt if the physical size changes, e.g. DPI) and returns
    /// the visible window area to lay the UI out in. Areas the collapsed sidebar
    /// doesn't cover are fully transparent.
    fn clip_window(&mut self, ctx: &Context, frame: &eframe::Frame) -> Rect {
        let ppp = ctx.pixels_per_point();
        let content = ctx.content_rect();
        let size = content.size() * ppp;
        let size = [size.x.round() as i32, size.y.round() as i32];
        if self.region_size != Some(size) {
            // The clip sits in the transparent edge margin around the painted shapes.
            let radius = ((theme::WINDOW_RADIUS as f32 + theme::EDGE_MARGIN) * ppp).round() as i32;
            let to_px = |r: Rect| window_shape::Shape {
                x: ((r.min.x - content.min.x) * ppp).round() as i32,
                y: ((r.min.y - content.min.y) * ppp).round() as i32,
                width: (r.width() * ppp).round() as i32,
                height: (r.height() * ppp).round() as i32,
                radius,
            };
            self.visible = window_shape::clip(frame, |v| {
                let area = Rect::from_min_size(content.min, vec2(v.width as f32, v.height as f32) / ppp);
                let (main, sidebar) = theme::frame_layout(area);
                vec![to_px(main.expand(theme::EDGE_MARGIN)), to_px(theme::sidebar_reach(sidebar).expand(theme::EDGE_MARGIN))]
            })
            .map(|v| vec2(v.width as f32, v.height as f32) / ppp);
            self.region_size = Some(size);
        }
        Rect::from_min_size(content.min, self.visible.unwrap_or(content.size()))
    }

    fn start(&self, ctx: &Context) {
        let Some(root) = &self.root else { return };
        let game_dir = Profiles::dir(root, &self.profiles.selected);
        *self.phase.lock().unwrap() = Phase::Working(None);
        let phase = self.phase.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let report = |step: Step| {
                if !matches!(step, Step::JavaCacheSkipped(_)) {
                    *phase.lock().unwrap() = Phase::Working(Some(step));
                    ctx.request_repaint();
                }
            };
            let next = match launch::launch(&game_dir, &report) {
                Ok(launched) => Phase::Running { child: launched.child, stopping: false },
                Err(e) => Phase::Failed(format!("{e:#}")),
            };
            *phase.lock().unwrap() = next;
            ctx.request_repaint();
        });
    }

    /// Returns to idle when the game exits; a crash is shown as a failure.
    fn poll_game(&self, ctx: &Context) {
        let mut phase = self.phase.lock().unwrap();
        if let Phase::Running { child, stopping } = &mut *phase {
            match child.try_wait() {
                Ok(None) => ctx.request_repaint_after(Duration::from_secs(1)),
                // Exiting after Stop is expected, whatever the exit code.
                Ok(Some(_)) if *stopping => *phase = Phase::Idle,
                Ok(Some(status)) if status.success() => *phase = Phase::Idle,
                Ok(Some(status)) => *phase = Phase::Failed(format!("Minecraft exited with {status}")),
                Err(e) => *phase = Phase::Failed(e.to_string()),
            }
        }
    }

    /// Stop: asks the game's window to close (it saves and exits like its own
    /// close button). If it has no window yet, or Stop is pressed again, the
    /// process is ended.
    fn stop_game(&self) {
        let mut phase = self.phase.lock().unwrap();
        if let Phase::Running { child, stopping } = &mut *phase {
            if *stopping || !game_window::request_close(child.id()) {
                let _ = child.kill();
            }
            *stopping = true;
        }
    }

    /// Full-height sidebar inside the window. While hovered it opens over the
    /// window and slightly past its top, bottom and left edges, the logo grows a
    /// little, and the "Hermes" wordmark appears beside it. Other contents aren't
    /// designed yet.
    ///
    /// Motion: everything moves on the app's one spring (same physics as buttons
    /// and dropdowns), layered so it reads as soft material rather than a box:
    /// the width leads; the growth past the edges and the logo chase it on their
    /// own springs, so they trail a beat; the leading (right) corners round out
    /// with speed; the wordmark waits until the panel is past halfway, then
    /// slides in, and on closing it leaves first.
    fn sidebar(&self, ctx: &Context, collapsed: Rect) {
        let id = Id::unique("sidebar");
        // `open` is the direction the sidebar is moving in. It stays open while the
        // pointer is anywhere over the expanded panel, and it only changes direction
        // once the width has settled: opening always fully opens, closing fully closes.
        let open = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        let hover_area = if open { theme::sidebar_rect(collapsed, 1.0, 1.0) } else { collapsed };
        // (The debug `--open-sidebar` option opens it from frame 10, once frame timing is steady.)
        let forced = self.force_sidebar && self.frames >= 10;
        // A fast move heading out of the open sidebar closes it straight away,
        // before the pointer has actually left, so it doesn't lag behind the hand.
        let velocity = ctx.input(|i| i.pointer.velocity());
        let wants_open = forced
            || ctx
                .pointer_hover_pos()
                .is_some_and(|p| hover_area.contains(p) && !(open && theme::heading_out(hover_area, p, velocity)));
        // Leaving the window closes it at once, even mid-opening (the one exception
        // to finishing a move before reversing).
        let left_window = !forced && ctx.input(|i| i.pointer.hover_pos().is_none());
        let open = if left_window {
            false
        } else if theme::spring_peek(ctx, id).settled {
            wants_open
        } else {
            open
        };
        ctx.data_mut(|d| d.insert_temp(id, open));

        // The panel bounces a little (calmer than buttons); the logo and all text
        // settle without overshoot, so nothing inside the sidebar wobbles.
        let width = theme::spring_motion_with(ctx, id, if open { 1.0 } else { 0.0 }, theme::Bounce::Calm);
        let outset = theme::spring_motion_with(ctx, id.with("outset"), width.value, theme::Bounce::Calm);
        let grow = theme::spring_motion_with(ctx, id.with("logo"), width.value.clamp(0.0, 1.0), theme::Bounce::None);
        let reveal_target = if open && width.value > 0.55 { 1.0 } else { 0.0 };
        let word = theme::spring_motion_with(ctx, id.with("word"), reveal_target, theme::Bounce::None);

        // Opening overshoots slightly; closing never goes smaller than collapsed,
        // which would uncover the window edge it hides.
        let t = width.value.max(0.0);
        let shown = t.min(1.0);
        let rect = theme::sidebar_rect(collapsed, t, outset.value.max(0.0));
        // The leading edge softens while moving: its corners round out with speed.
        let soften = (width.velocity.abs() * 4.0).min(26.0) as u8;
        let corners = egui::CornerRadius {
            ne: theme::WINDOW_RADIUS + soften,
            se: theme::WINDOW_RADIUS + soften,
            ..theme::radius(theme::WINDOW_RADIUS)
        };
        // The logo keeps its place at the top and grows a little, a beat behind the panel.
        let pad = (theme::SIDEBAR_WIDTH - theme::LOGO_SIZE) / 2.0;
        let size = egui::emath::lerp(theme::LOGO_SIZE..=theme::LOGO_SIZE_EXPANDED, grow.value.max(0.0));
        let logo = Rect::from_min_size(rect.min + egui::Vec2::splat(pad), egui::Vec2::splat(size));
        egui::Area::new(id).order(egui::Order::Middle).fixed_pos(rect.min).show(ctx, |ui| {
            ui.allocate_rect(rect, Sense::hover());
            let painter = ui.painter();
            // Selected while open: the outline thickens and shimmers like a dropdown's.
            theme::glass(&painter.with_clip_rect(rect.expand(24.0)), rect, corners, shown, 0, shown, theme::Edge::Diamond);
            painter.image(self.logo.id(), logo, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
            let reveal = word.value.clamp(0.0, 1.0);
            if reveal > 0.0 {
                // "Hermes" in Pacifico, its letters as tall as the logo: slides in from
                // just behind the logo while fading in, clipped to the panel.
                let probe = painter.layout_no_wrap("Hermes".into(), theme::wordmark_font(100.0), theme::TEXT);
                let font_size = 100.0 * size / probe.mesh_bounds.height().max(1.0);
                let color = theme::TEXT.gamma_multiply(reveal);
                let galley = painter.layout_no_wrap("Hermes".into(), theme::wordmark_font(font_size), color);
                let ink = galley.mesh_bounds;
                let slide = 10.0 * (1.0 - word.value);
                let pos = pos2(logo.max.x + 12.0 - slide - ink.min.x, logo.center().y - ink.center().y);
                painter.with_clip_rect(rect.shrink(theme::STROKE_WIDTH)).galley(pos, galley, color);
            }

            // Navigation: Home under the logo, Settings at the very bottom. Pages for
            // them aren't designed yet, so they don't navigate anywhere yet.
            let pad_top = (theme::SIDEBAR_WIDTH - theme::LOGO_SIZE) / 2.0;
            let home_y = rect.min.y + pad_top + theme::LOGO_SIZE + 18.0;
            let settings_y = rect.max.y - theme::NAV_INSET - theme::NAV_ROW_HEIGHT;
            let icons = &self.nav_icons;
            let row = |id, y, key, icon, filled| NavRow { id, y, label_key: key, icon, filled };
            nav_row(ui, rect, open, word.value, row(Id::unique("nav_home"), home_y, "sidebar.home", &icons.home, &icons.home_filled));
            nav_row(ui, rect, open, word.value, row(Id::unique("nav_settings"), settings_y, "sidebar.settings", &icons.settings, &icons.settings_filled));
        });
    }

    fn title_bar(&mut self, ui: &mut egui::Ui, bar: Rect) {
        let ctx = ui.ctx().clone();
        let size = vec2(40.0, 32.0);
        let close = Rect::from_center_size(pos2(bar.max.x - 32.0, bar.center().y + 1.0), size);
        let minimise = close.translate(vec2(-(size.x + 6.0), 0.0));
        // Account switcher: in the bar, left of the minimise button.
        let account = Rect::from_min_size(
            pos2(minimise.min.x - 12.0 - theme::ACCOUNT_SIZE.x, bar.center().y + 1.0 - theme::ACCOUNT_SIZE.y / 2.0),
            theme::ACCOUNT_SIZE,
        );
        // The window drags from the bar but never from its controls: the drag zone
        // stops just left of the account switcher, so pressing a control can't start a move.
        let drag_zone = Rect::from_min_max(bar.min, pos2(account.min.x - 6.0, bar.max.y));
        let drag = ui.interact(drag_zone, Id::unique("title_bar"), Sense::click_and_drag());
        if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        // Faint divider along the bottom of the bar the window buttons sit in,
        // stopping short of the window's outline on the right.
        let y = bar.max.y - 0.5;
        ui.painter().line_segment(
            [pos2(bar.min.x, y), pos2(bar.max.x - theme::STROKE_WIDTH, y)],
            Stroke::new(1.0, theme::OUTLINE),
        );

        if bar_button(ui, close, "window.close", true, |p, c, color| {
            let d = 5.5;
            p.line_segment([c + vec2(-d, -d), c + vec2(d, d)], Stroke::new(theme::ICON_STROKE, color));
            p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], Stroke::new(theme::ICON_STROKE, color));
        }) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if bar_button(ui, minimise, "window.minimise", false, |p, c, color| {
            p.line_segment([c + vec2(-6.0, 0.0), c + vec2(6.0, 0.0)], Stroke::new(theme::ICON_STROKE, color));
        }) {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }

        let items: Vec<dropdown::Item> = self
            .accounts
            .iter()
            .map(|name| dropdown::Item { label: name.clone(), icon: Some(theme::HEAD_PLACEHOLDER) })
            .collect();
        if let Some(i) = dropdown::show(&ctx, Id::unique(ACCOUNT_DROPDOWN), account, &items, self.selected_account) {
            self.selected_account = i;
        }
    }

    fn play_button(&self, ui: &mut egui::Ui, rect: Rect) {
        let (label, progress, look, error) = match &*self.phase.lock().unwrap() {
            Phase::Idle => (tr("play"), None, PlayLook::Ready, None),
            Phase::Working(step) => match step {
                None | Some(Step::Metadata) => (tr("status.metadata"), None, PlayLook::Busy, None),
                Some(Step::Checking) => (tr("status.checking"), None, PlayLook::Busy, None),
                Some(Step::Downloading { done, total }) => (
                    tr_args("status.downloading", &[&done.to_string(), &total.to_string()]),
                    Some(*done as f32 / (*total).max(1) as f32),
                    PlayLook::Busy,
                    None,
                ),
                Some(Step::PreparingJava) => (tr("status.preparing_java"), None, PlayLook::Busy, None),
                Some(Step::Starting | Step::JavaCacheSkipped(_)) => (tr("status.starting"), None, PlayLook::Busy, None),
            },
            Phase::Running { .. } => (tr("status.stop"), None, PlayLook::Stop, None),
            Phase::Failed(e) => (tr("status.failed"), None, PlayLook::Failed, Some(e.clone())),
        };
        let enabled = look != PlayLook::Busy;

        let mut response = ui.interact(rect, Id::unique("play"), if enabled { Sense::click() } else { Sense::hover() });
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &label));
        let painter = ui.painter();
        let radius = theme::radius(theme::CONTROL_RADIUS);
        let pressed = enabled && response.is_pointer_button_down_on();
        // Shrinks while pressed and springs back on release; only the drawing scales.
        let rect = theme::scale_about(rect, rect.center(), theme::press_scale(ui.ctx(), response.id, pressed));
        let text_color = match look {
            PlayLook::Ready => {
                let fill = if pressed {
                    theme::PLAY.lerp_to_gamma(theme::BG, 0.15)
                } else if response.hovered() {
                    theme::PLAY.lerp_to_gamma(egui::Color32::WHITE, 0.12)
                } else {
                    theme::PLAY
                };
                painter.rect_filled(rect, radius, fill);
                theme::PLAY_TEXT
            }
            // Running: red Stop button.
            PlayLook::Stop => {
                let fill = if pressed {
                    theme::STOP.lerp_to_gamma(theme::BG, 0.2)
                } else if response.hovered() {
                    theme::STOP.lerp_to_gamma(egui::Color32::WHITE, 0.08)
                } else {
                    theme::STOP
                };
                painter.rect_filled(rect, radius, fill);
                egui::Color32::WHITE
            }
            // Working: a muted blue that still reads as "Play", with light text.
            PlayLook::Busy => {
                painter.rect_filled(rect, radius, theme::PLAY.lerp_to_gamma(theme::PANEL, 0.6));
                theme::TEXT
            }
            // Failed: an outlined control in the error colour, clickable to retry.
            PlayLook::Failed => {
                painter.rect_filled(rect, radius, theme::PANEL);
                if pressed || response.hovered() {
                    painter.rect_filled(rect, radius, if pressed { theme::GLASS_PRESS } else { theme::GLASS_HOVER });
                }
                painter.rect_stroke(rect, radius, Stroke::new(theme::STROKE_WIDTH, theme::DANGER), StrokeKind::Inside);
                theme::DANGER_TEXT
            }
        };
        if let Some(fraction) = progress {
            // Thin track along the bottom edge, so the label always sits on one colour.
            let inset = theme::CONTROL_RADIUS as f32;
            let track = Rect::from_min_max(pos2(rect.min.x + inset, rect.max.y - 7.0), pos2(rect.max.x - inset, rect.max.y - 4.0));
            painter.rect_filled(track, theme::radius(2), theme::PLAY.lerp_to_gamma(theme::PANEL, 0.85));
            let done = Rect::from_min_max(track.min, pos2(track.min.x + track.width() * fraction.clamp(0.0, 1.0), track.max.y));
            painter.rect_filled(done, theme::radius(2), theme::PLAY);
        }
        // Long labels (and longer translations) truncate with an ellipsis inside the button.
        let mut job = egui::text::LayoutJob::simple_singleline(label.clone(), theme::text_font(rect.height() * 0.38), text_color);
        job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 2.0 * theme::CONTROL_RADIUS as f32 - 8.0);
        let galley = painter.layout_job(job);
        // Lift the label slightly when the progress track sits under it.
        let center = rect.center() - vec2(0.0, if progress.is_some() { 2.0 } else { 0.0 });
        painter.galley(Align2::CENTER_CENTER.anchor_size(center, galley.size()).min, galley, text_color);
        if response.has_focus() {
            theme::focus_ring(painter, rect, theme::CONTROL_RADIUS);
        }

        if let Some(error) = error {
            response = response.on_hover_text(tr_args("status.failed.details", &[&error]));
        }
        if enabled && response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            if look == PlayLook::Stop {
                self.stop_game();
                return;
            }
            self.start(ui.ctx());
        }
    }

    /// Developer screenshot: waits for animations to settle, saves the frame, closes.
    fn screenshot_tick(&mut self, ctx: &Context) {
        let Some(path) = &self.screenshot else { return };
        self.frames += 1;
        ctx.request_repaint();
        if self.frames == 5 {
            if let Some(id) = self.focus.take() {
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
        if self.frames == self.screenshot_frame {
            let path = path.clone();
            let ctx2 = ctx.clone();
            ctx.request_screenshot(move |image| {
                let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_srgba_unmultiplied()).collect();
                let [w, h] = image.size;
                if let Err(e) = image::save_buffer(&path, &rgba, w as u32, h as u32, image::ColorType::Rgba8) {
                    eprintln!("screenshot failed: {e}");
                }
                ctx2.send_viewport_cmd(ViewportCommand::Close);
            });
        }
    }
}

/// Outline and filled versions of the sidebar's navigation icons.
struct NavIcons {
    home: TextureHandle,
    home_filled: TextureHandle,
    settings: TextureHandle,
    settings_filled: TextureHandle,
}

/// One sidebar navigation row.
struct NavRow<'a> {
    id: Id,
    /// Top of the row inside the sidebar.
    y: f32,
    label_key: &'a str,
    icon: &'a TextureHandle,
    filled: &'a TextureHandle,
}

/// Draws a navigation row inside the sidebar `panel`: the icon (outline, filled
/// while hovered), its label beside it (shown as the sidebar opens, on the same
/// reveal as the wordmark, `reveal` 0..1+), and, only while hovered and the
/// sidebar is `open` (not closed or closing), a frosted-glass selector of fixed
/// length (the full width of the open sidebar) around both.
fn nav_row(ui: &mut egui::Ui, panel: Rect, open: bool, reveal: f32, nav: NavRow) {
    let NavRow { id, y, label_key, icon, filled } = nav;
    let row = Rect::from_min_size(pos2(panel.min.x + theme::NAV_INSET, y), vec2(theme::NAV_ROW_WIDTH, theme::NAV_ROW_HEIGHT));
    let clip = panel.shrink(theme::STROKE_WIDTH);
    let label = tr(label_key);
    // Only the part of the row inside the sidebar responds to the pointer.
    let visible = row.intersect(clip);
    let response = ui.interact(visible, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    let hovered = response.hovered();
    let painter = ui.painter().with_clip_rect(clip);
    // Pressing shrinks the icon, label and selector together, like every other
    // button, around the middle of the part that's visible.
    let scale = theme::press_scale(ui.ctx(), id, response.is_pointer_button_down_on());
    let origin = visible.center();
    let shrink = |r: Rect| theme::scale_about(r, origin, scale);

    let selector = ui.ctx().animate_bool_with_time(id.with("selector"), hovered && open, 0.12);
    theme::frosted(&painter, shrink(row), theme::SMALL_RADIUS + 3, selector);

    // The icon sits centred in the collapsed sidebar, so it never moves sideways.
    let center = pos2(panel.min.x + theme::SIDEBAR_WIDTH / 2.0, row.center().y);
    let icon_rect = Rect::from_center_size(center, egui::Vec2::splat(theme::NAV_ICON_SIZE));
    let texture = if hovered { filled } else { icon };
    painter.image(texture.id(), shrink(icon_rect), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);

    let shown = reveal.clamp(0.0, 1.0);
    if shown > 0.0 {
        let color = theme::TEXT.gamma_multiply(shown);
        let slide = 10.0 * (1.0 - reveal);
        let anchor = pos2(icon_rect.max.x + 14.0 - slide, row.center().y);
        let anchor = origin + (anchor - origin) * scale;
        painter.text(anchor, Align2::LEFT_CENTER, label, theme::text_font(theme::NAV_TEXT_SIZE * scale), color);
    }
    if response.has_focus() {
        theme::focus_ring(&painter, row.intersect(clip), theme::SMALL_RADIUS + 3);
    }
}

/// A title-bar control. `danger` turns the hover background red (close).
/// `name_key` is the language key used as its accessible name.
fn bar_button(
    ui: &mut egui::Ui,
    rect: Rect,
    name_key: &str,
    danger: bool,
    icon: impl FnOnce(&egui::Painter, egui::Pos2, egui::Color32),
) -> bool {
    let response = ui.interact(rect, Id::unique(("bar_button", name_key)), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr(name_key)));
    let pressed = response.is_pointer_button_down_on();
    let rect = theme::scale_about(rect, rect.center(), theme::press_scale(ui.ctx(), response.id, pressed));
    let active = pressed || response.hovered();
    if active {
        let fill = match (danger, pressed) {
            (true, false) => theme::DANGER,
            (true, true) => theme::DANGER.lerp_to_gamma(theme::BG, 0.2),
            (false, false) => theme::GLASS_HOVER,
            (false, true) => theme::GLASS_PRESS,
        };
        ui.painter().rect_filled(rect, theme::radius(theme::SMALL_RADIUS), fill);
    }
    if response.has_focus() {
        theme::focus_ring(ui.painter(), rect, theme::SMALL_RADIUS);
    }
    let color = if active && danger { egui::Color32::WHITE } else { theme::TEXT };
    icon(ui.painter(), rect.center(), color);
    response.clicked()
}

/// Loads each profile's icon (a PNG named in its `profile.json`); unreadable icons are skipped.
fn load_profile_icons(ctx: &Context, root: &std::path::Path, profiles: &Profiles) -> HashMap<String, TextureHandle> {
    profiles
        .profiles
        .iter()
        .filter_map(|p| {
            let bytes = std::fs::read(Profiles::icon_path(root, p)?).ok()?;
            image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?;
            Some((p.id.clone(), load_png(ctx, &format!("profile-icon-{}", p.id), &bytes)))
        })
        .collect()
}

fn load_png(ctx: &Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .expect("bundled PNG decodes")
        .to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    // Mipmaps keep large source images smooth when drawn small (logo, profile icons).
    let options = TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..TextureOptions::LINEAR };
    ctx.load_texture(name, color, options)
}

impl eframe::App for HermesApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Transparent: the anti-aliased rounded edges blend with what is behind.
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let area = self.clip_window(&ctx, frame);
        self.poll_game(&ctx);
        let (window, sidebar) = theme::frame_layout(area);

        let painter = ui.painter().clone();
        painter.rect_filled(window, theme::radius(theme::WINDOW_RADIUS), theme::BG);
        theme::outline(&painter, window, theme::WINDOW_RADIUS, theme::Edge::Diamond);

        // Main content pane: the home screen's pinned profiles.
        let body = theme::body_layout(window, sidebar);
        painter.rect_filled(body.content, theme::radius(theme::PANEL_RADIUS), theme::PANEL);
        theme::outline(&painter, body.content, theme::PANEL_RADIUS, theme::Edge::Solid);
        self.home(ui, body.content);

        self.sidebar(&ctx, sidebar);

        // Custom bar across the top of the window, right of the sidebar.
        let bar = Rect::from_min_max(pos2(sidebar.max.x, window.min.y), pos2(window.max.x, window.min.y + theme::BAR_HEIGHT));
        self.title_bar(ui, bar);

        self.play_button(ui, body.play);

        self.screenshot_tick(&ctx);
    }
}

/// Window and taskbar icon: the logo on a dark blue circle (the exe file gets the
/// same artwork from `app_icon.ico`, see build.rs).
fn app_icon() -> egui::IconData {
    let image = image::load_from_memory_with_format(include_bytes!("../assets/images/app_icon.png"), image::ImageFormat::Png)
        .expect("bundled PNG decodes")
        .to_rgba8();
    egui::IconData { width: image.width(), height: image.height(), rgba: image.into_raw() }
}

pub fn run(dev: DevOptions) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(app_icon())
            .with_inner_size(theme::window_size())
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_maximize_button(false),
        ..Default::default()
    };
    eframe::run_native(
        "Hermes Client",
        options,
        Box::new(|cc| Ok(Box::new(HermesApp::new(&cc.egui_ctx, dev)))),
    )
}
