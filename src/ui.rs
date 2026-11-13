//! Placeholder window: one Start button and a status line.

use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::launch;

#[derive(Default)]
struct State {
    busy: bool,
    status: String,
}

#[derive(Default)]
struct HermesApp {
    state: Arc<Mutex<State>>,
}

impl HermesApp {
    fn start(&self, ctx: &egui::Context) {
        let state = self.state.clone();
        let ctx = ctx.clone();
        state.lock().unwrap().busy = true;
        std::thread::spawn(move || {
            let set = |s: String| {
                state.lock().unwrap().status = s;
                ctx.request_repaint();
            };
            let message = match launch::launch(&set) {
                Ok(l) => format!("Minecraft started in {} ms", l.elapsed.as_millis()),
                Err(e) => format!("Launch failed: {e:#}"),
            };
            let mut s = state.lock().unwrap();
            s.status = message;
            s.busy = false;
            ctx.request_repaint();
        });
    }
}

impl eframe::App for HermesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(12.0);
                ui.heading("Hermes Client");
                ui.label(format!("Fabric {}", launch::MC_VERSION));
                ui.add_space(12.0);
                let (busy, status) = {
                    let s = self.state.lock().unwrap();
                    (s.busy, s.status.clone())
                };
                let button = egui::Button::new("Start").min_size(egui::vec2(140.0, 36.0));
                if ui.add_enabled(!busy, button).clicked() {
                    self.start(&ctx);
                }
                ui.add_space(8.0);
                ui.label(status);
            });
        });
    }
}

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Hermes Client")
            .with_inner_size([380.0, 200.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native("Hermes Client", options, Box::new(|_cc| Ok(Box::new(HermesApp::default()))))
}
