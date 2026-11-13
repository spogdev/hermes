//! Main launcher window (design v1, see `app-design.md`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{
    self, Align2, Context, CursorIcon, FontId, Id, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions,
    ViewportCommand, pos2, vec2,
};

use crate::dropdown::{self, Direction};
use crate::lang::{tr, tr_args};
use crate::launch::{self, Step};
use crate::profiles::Profiles;
use crate::theme::{self, place};
use crate::window_shape;

/// Developer options for visual checks (`--screenshot <png>`, `--open-dropdown account|profile`,
/// `--state downloading|checking|failed`, `--focus play|account`).
#[derive(Default)]
pub struct DevOptions {
    pub screenshot: Option<PathBuf>,
    pub open_dropdown: Option<String>,
    pub state: Option<String>,
    pub focus: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum PlayLook {
    Ready,
    Busy,
    Failed,
}

enum Phase {
    Idle,
    Working(Option<Step>),
    Running(Child),
    Failed(String),
}

struct HermesApp {
    phase: Arc<Mutex<Phase>>,
    logo: TextureHandle,
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
}

const ACCOUNT_DROPDOWN: &str = "account_dropdown";
const PROFILE_DROPDOWN: &str = "profile_dropdown";

impl HermesApp {
    fn new(ctx: &Context, dev: DevOptions) -> Self {
        match dev.open_dropdown.as_deref() {
            Some("account") => dropdown::set_open(ctx, Id::unique(ACCOUNT_DROPDOWN), true),
            Some("profile") => dropdown::set_open(ctx, Id::unique(PROFILE_DROPDOWN), true),
            _ => {}
        }
        theme::apply_style(ctx);
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
            _ => {}
        }
        let focus = match dev.focus.as_deref() {
            Some("play") => Some(Id::unique("play")),
            Some("account") => Some(dropdown::pill_id(Id::unique(ACCOUNT_DROPDOWN))),
            _ => None,
        };
        let profile_icons = root.as_deref().map(|r| load_profile_icons(ctx, r, &profiles)).unwrap_or_default();
        HermesApp {
            focus,
            profile_icons,
            phase: Arc::new(Mutex::new(phase)),
            logo: load_png(ctx, "logo", include_bytes!("../assets/images/logo_placeholder.png")),
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

    fn profile_dropdown(&mut self, ctx: &Context, rect: Rect) {
        let items: Vec<dropdown::Item> = self
            .profiles
            .profiles
            .iter()
            .map(|p| dropdown::Item {
                label: p.info.name.clone().unwrap_or_else(|| tr("launcher.profile.unnamed")),
                icon: self.profile_icons.get(&p.id).map(|t| dropdown::Icon::Image(t.id())),
            })
            .collect();
        let selected = self.profiles.selected_index();
        // Creating profiles from the launcher comes later (owner will add it).
        if let Some(i) = dropdown::show(ctx, Id::unique(PROFILE_DROPDOWN), rect, Direction::Up, &items, selected) {
            let id = self.profiles.profiles[i].id.clone();
            let saved = match &self.root {
                Some(root) => self.profiles.select(root, &id),
                None => Ok(()),
            };
            if let Err(e) = saved {
                *self.phase.lock().unwrap() = Phase::Failed(format!("{e:#}"));
            }
        }
    }

    /// Clips the window to the outline of the main window plus the overhanging
    /// sidebar (rebuilt if the physical size changes, e.g. DPI) and returns the
    /// visible window area to lay the UI out in.
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
                vec![to_px(main.expand(theme::EDGE_MARGIN)), to_px(sidebar.expand(theme::EDGE_MARGIN))]
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
                Ok(launched) => Phase::Running(launched.child),
                Err(e) => Phase::Failed(format!("{e:#}")),
            };
            *phase.lock().unwrap() = next;
            ctx.request_repaint();
        });
    }

    /// Returns to idle when the game exits; a crash is shown as a failure.
    fn poll_game(&self, ctx: &Context) {
        let mut phase = self.phase.lock().unwrap();
        if let Phase::Running(child) = &mut *phase {
            match child.try_wait() {
                Ok(None) => ctx.request_repaint_after(Duration::from_secs(1)),
                Ok(Some(status)) if status.success() => *phase = Phase::Idle,
                Ok(Some(status)) => *phase = Phase::Failed(format!("Minecraft exited with {status}")),
                Err(e) => *phase = Phase::Failed(e.to_string()),
            }
        }
    }

    fn title_bar(&self, ui: &mut egui::Ui, bar: Rect) {
        let ctx = ui.ctx().clone();
        let drag = ui.interact(bar, Id::unique("title_bar"), Sense::click_and_drag());
        if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        ui.painter().text(
            pos2(bar.min.x + 12.0, bar.center().y + 2.0),
            Align2::LEFT_CENTER,
            tr("launcher.title"),
            FontId::proportional(13.0),
            theme::TEXT_DIM,
        );

        let size = vec2(32.0, 24.0);
        let close = Rect::from_center_size(pos2(bar.max.x - 30.0, bar.center().y + 2.0), size);
        let minimise = close.translate(vec2(-(size.x + 6.0), 0.0));
        if bar_button(ui, close, "launcher.window.close", true, |p, c, color| {
            let d = 4.5;
            p.line_segment([c + vec2(-d, -d), c + vec2(d, d)], Stroke::new(theme::ICON_STROKE, color));
            p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], Stroke::new(theme::ICON_STROKE, color));
        }) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if bar_button(ui, minimise, "launcher.window.minimise", false, |p, c, color| {
            p.line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], Stroke::new(theme::ICON_STROKE, color));
        }) {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
    }

    fn play_button(&self, ui: &mut egui::Ui, rect: Rect) {
        let (label, progress, look, error) = match &*self.phase.lock().unwrap() {
            Phase::Idle => (tr("launcher.play"), None, PlayLook::Ready, None),
            Phase::Working(step) => match step {
                None | Some(Step::Metadata) => (tr("launcher.status.metadata"), None, PlayLook::Busy, None),
                Some(Step::Checking) => (tr("launcher.status.checking"), None, PlayLook::Busy, None),
                Some(Step::Downloading { done, total }) => (
                    tr_args("launcher.status.downloading", &[&done.to_string(), &total.to_string()]),
                    Some(*done as f32 / (*total).max(1) as f32),
                    PlayLook::Busy,
                    None,
                ),
                Some(Step::PreparingJava) => (tr("launcher.status.preparing_java"), None, PlayLook::Busy, None),
                Some(Step::Starting | Step::JavaCacheSkipped(_)) => (tr("launcher.status.starting"), None, PlayLook::Busy, None),
            },
            Phase::Running(_) => (tr("launcher.status.running"), None, PlayLook::Busy, None),
            Phase::Failed(e) => (tr("launcher.status.failed"), None, PlayLook::Failed, Some(e.clone())),
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
            // Working or running: a muted green that still reads as "Play", with light text.
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
        let mut job = egui::text::LayoutJob::simple_singleline(label.clone(), FontId::proportional(rect.height() * 0.38), text_color);
        job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 2.0 * theme::CONTROL_RADIUS as f32 - 8.0);
        let galley = painter.layout_job(job);
        // Lift the label slightly when the progress track sits under it.
        let center = rect.center() - vec2(0.0, if progress.is_some() { 2.0 } else { 0.0 });
        painter.galley(Align2::CENTER_CENTER.anchor_size(center, galley.size()).min, galley, text_color);
        if response.has_focus() {
            theme::focus_ring(painter, rect, theme::CONTROL_RADIUS);
        }

        if let Some(error) = error {
            response = response.on_hover_text(tr_args("launcher.status.failed.details", &[&error]));
        }
        if enabled && response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
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
        if self.frames == 45 {
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
    ctx.load_texture(name, color, TextureOptions::LINEAR)
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
        let stroke = Stroke::new(theme::STROKE_WIDTH, theme::OUTLINE);
        let window_radius = theme::radius(theme::WINDOW_RADIUS);
        painter.rect_filled(window, window_radius, theme::BG);
        painter.rect_stroke(window, window_radius, stroke, StrokeKind::Inside);

        // Sidebar: full height and a little bigger than the main window, so it overhangs it.
        painter.rect_filled(sidebar, window_radius, theme::PANEL);
        painter.rect_stroke(sidebar, window_radius, stroke, StrokeKind::Inside);

        // Custom bar across the top of the main window, right of the sidebar.
        let bar = Rect::from_min_max(pos2(sidebar.max.x, window.min.y), pos2(window.max.x, window.min.y + theme::BAR_HEIGHT));
        self.title_bar(ui, bar);
        // The template's main column sits below the bar (x positions include the sidebar).
        let body = Rect::from_min_max(pos2(window.min.x, bar.max.y), window.max);

        // Logo: template size, centred in the sidebar with equal side and top margins.
        let logo_area = place(window, 10.0, 12.0, 46.0, 40.0);
        let logo_size = logo_area.width().min(logo_area.height());
        let margin = (sidebar.width() - logo_size) / 2.0;
        let logo = Rect::from_min_size(sidebar.min + egui::Vec2::splat(margin), egui::Vec2::splat(logo_size));
        painter.image(self.logo.id(), logo, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);

        // Main content area: contents not designed yet.
        let content = place(body, 78.0, 78.0, 1248.0, 579.0);
        painter.rect_filled(content, theme::radius(theme::PANEL_RADIUS), theme::PANEL);
        painter.rect_stroke(content, theme::radius(theme::PANEL_RADIUS), stroke, StrokeKind::Inside);

        self.play_button(ui, place(body, 1070.0, 675.0, 256.0, 58.0));
        // Profile selector: left of Play, same size, opens upward (owner request, no mock-up yet).
        self.profile_dropdown(&ctx, place(body, 796.0, 675.0, 256.0, 58.0));

        let items: Vec<dropdown::Item> = self
            .accounts
            .iter()
            .map(|name| dropdown::Item { label: name.clone(), icon: Some(dropdown::Icon::Color(theme::HEAD_PLACEHOLDER)) })
            .collect();
        let account = place(body, 1095.0, 13.0, 231.0, 50.0);
        if let Some(i) = dropdown::show(&ctx, Id::unique(ACCOUNT_DROPDOWN), account, Direction::Down, &items, self.selected_account) {
            self.selected_account = i;
        }

        self.screenshot_tick(&ctx);
    }
}

pub fn run(dev: DevOptions) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
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
