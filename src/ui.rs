//! Main launcher window (design v1, see `app-design.md`).

use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{
    self, Align2, Context, CursorIcon, FontId, Id, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions,
    ViewportCommand, pos2, vec2,
};

use crate::lang::{tr, tr_args};
use crate::launch::{self, Step};
use crate::theme::{self, place};
use crate::dropdown;

/// Developer options for visual checks (`--screenshot <png>`, `--open-dropdown`).
#[derive(Default)]
pub struct DevOptions {
    pub screenshot: Option<PathBuf>,
    pub open_dropdown: bool,
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
    screenshot: Option<PathBuf>,
    frames: u32,
}

const ACCOUNT_DROPDOWN: &str = "account_dropdown";

impl HermesApp {
    fn new(ctx: &Context, dev: DevOptions) -> Self {
        if dev.open_dropdown {
            dropdown::set_open(ctx, Id::unique(ACCOUNT_DROPDOWN), true);
        }
        HermesApp {
            phase: Arc::new(Mutex::new(Phase::Idle)),
            logo: load_png(ctx, "logo", include_bytes!("../assets/images/logo_placeholder.png")),
            // Offline mode has one account; Microsoft accounts will fill this list.
            accounts: vec![launch::PLAYER_NAME.to_string()],
            selected_account: 0,
            screenshot: dev.screenshot,
            frames: 0,
        }
    }

    fn start(&self, ctx: &Context) {
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
            let next = match launch::launch(&report) {
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
            pos2(bar.min.x + 22.0, bar.center().y + 2.0),
            Align2::LEFT_CENTER,
            tr("launcher.title"),
            FontId::proportional(13.0),
            theme::TEXT_DIM,
        );

        let size = vec2(32.0, 24.0);
        let close = Rect::from_center_size(pos2(bar.max.x - 30.0, bar.center().y + 2.0), size);
        let minimise = close.translate(vec2(-(size.x + 6.0), 0.0));
        if bar_button(ui, close, "close", true, |p, c, color| {
            let d = 4.5;
            p.line_segment([c + vec2(-d, -d), c + vec2(d, d)], Stroke::new(1.6, color));
            p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], Stroke::new(1.6, color));
        }) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if bar_button(ui, minimise, "minimise", false, |p, c, color| {
            p.line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], Stroke::new(1.6, color));
        }) {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
    }

    fn play_button(&self, ui: &mut egui::Ui, rect: Rect) {
        let (label, progress, enabled, error) = match &*self.phase.lock().unwrap() {
            Phase::Idle => (tr("launcher.play"), None, true, None),
            Phase::Working(step) => match step {
                None | Some(Step::Metadata) => (tr("launcher.status.metadata"), None, false, None),
                Some(Step::Checking) => (tr("launcher.status.checking"), None, false, None),
                Some(Step::Downloading { done, total }) => (
                    tr_args("launcher.status.downloading", &[&done.to_string(), &total.to_string()]),
                    Some(*done as f32 / (*total).max(1) as f32),
                    false,
                    None,
                ),
                Some(Step::PreparingJava) => (tr("launcher.status.preparing_java"), None, false, None),
                Some(Step::Starting | Step::JavaCacheSkipped(_)) => (tr("launcher.status.starting"), None, false, None),
            },
            Phase::Running(_) => (tr("launcher.status.running"), None, false, None),
            Phase::Failed(e) => (tr("launcher.status.failed"), None, true, Some(e.clone())),
        };

        let mut response = ui.interact(rect, Id::unique("play"), if enabled { Sense::click() } else { Sense::hover() });
        let painter = ui.painter();
        let radius = theme::radius(theme::PANEL_RADIUS);
        let base = if !enabled {
            theme::PLAY.gamma_multiply(0.45)
        } else if response.hovered() {
            theme::PLAY.gamma_multiply(1.12)
        } else {
            theme::PLAY
        };
        painter.rect_filled(rect, radius, base);
        if let Some(fraction) = progress {
            let filled = Rect::from_min_max(rect.min, pos2(rect.min.x + rect.width() * fraction, rect.max.y));
            painter.with_clip_rect(filled).rect_filled(rect, radius, theme::PLAY);
        }
        painter.text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(rect.height() * 0.38), theme::PLAY_TEXT);

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
fn bar_button(
    ui: &mut egui::Ui,
    rect: Rect,
    id: &str,
    danger: bool,
    icon: impl FnOnce(&egui::Painter, egui::Pos2, egui::Color32),
) -> bool {
    let response = ui.interact(rect, Id::unique(("bar_button", id)), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, theme::radius(8), if danger { theme::DANGER } else { theme::GLASS_HOVER });
    }
    let color = if response.hovered() && danger { egui::Color32::WHITE } else { theme::TEXT };
    icon(ui.painter(), rect.center(), color);
    response.clicked()
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
        [0.0; 4] // transparent, so the rounded window corners show what is behind
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_game(&ctx);

        let window = ctx.content_rect();
        let painter = ui.painter().clone();
        let stroke = Stroke::new(theme::STROKE_WIDTH, theme::OUTLINE);
        painter.rect_filled(window, theme::radius(theme::WINDOW_RADIUS), theme::BG);
        painter.rect_stroke(window, theme::radius(theme::WINDOW_RADIUS), stroke, StrokeKind::Inside);

        let bar = Rect::from_min_size(window.min, vec2(window.width(), theme::BAR_HEIGHT));
        self.title_bar(ui, bar);
        let body = Rect::from_min_max(pos2(window.min.x, bar.max.y), window.max);

        // Sidebar: its bottom-left corner follows the window's rounding.
        let sidebar = place(body, 0.0, 0.0, 66.0, 749.0);
        let sidebar_radius = egui::CornerRadius {
            sw: theme::WINDOW_RADIUS,
            ..theme::radius(theme::PANEL_RADIUS)
        };
        painter.rect_filled(sidebar, sidebar_radius, theme::PANEL);
        painter.rect_stroke(sidebar, sidebar_radius, stroke, StrokeKind::Inside);

        let logo_area = place(body, 10.0, 12.0, 46.0, 40.0);
        let logo = Rect::from_center_size(logo_area.center(), egui::Vec2::splat(logo_area.width().min(logo_area.height())));
        painter.image(self.logo.id(), logo, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);

        // Main content area: contents not designed yet.
        let content = place(body, 78.0, 78.0, 1248.0, 579.0);
        painter.rect_filled(content, theme::radius(theme::PANEL_RADIUS), theme::PANEL);
        painter.rect_stroke(content, theme::radius(theme::PANEL_RADIUS), stroke, StrokeKind::Inside);

        self.play_button(ui, place(body, 1070.0, 675.0, 256.0, 58.0));

        let items: Vec<dropdown::Item> = self
            .accounts
            .iter()
            .map(|name| dropdown::Item { label: name, icon: theme::HEAD_PLACEHOLDER })
            .collect();
        dropdown::show(&ctx, Id::unique(ACCOUNT_DROPDOWN), place(body, 1095.0, 13.0, 231.0, 50.0), &items, &mut self.selected_account);

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
