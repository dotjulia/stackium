use egui::RichText;

use crate::{debugger_window::DebuggerWindowImpl, frame_history::FrameHistory};

pub struct SettingsWindow {
    frame_history: FrameHistory,
    run_mode: RunMode,
}

#[derive(PartialEq)]
enum RunMode {
    Reactive,
    Continuous,
}

impl SettingsWindow {
    pub fn new() -> Self {
        Self {
            frame_history: FrameHistory::default(),
            run_mode: RunMode::Reactive,
        }
    }
}

impl DebuggerWindowImpl for SettingsWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.frame_history
            .on_new_frame(ctx.input(|i| i.time), frame.info().cpu_usage);
        match self.run_mode {
            RunMode::Reactive => {}
            RunMode::Continuous => {
                // request repaint immediately
                ctx.request_repaint();
            }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.collapsing("Debug Info", |ui| {
            ui.horizontal(|ui| {
                let run_mode = &mut self.run_mode;
                ui.label("Mode:");
                ui.radio_value(run_mode, RunMode::Reactive, "Reactive")
                    .on_hover_text(
                        "Repaint when there are animations or input (e.g. mouse movement)",
                    );
                ui.radio_value(run_mode, RunMode::Continuous, "Continuous")
                    .on_hover_text("Repaint everything each frame");
            });
            if self.run_mode == RunMode::Continuous {
                ui.label(
                    RichText::new(format!(
                        "⚠ Repainting the UI each frame. FPS: {:.1}",
                        self.frame_history.fps()
                    ))
                    .color(ui.visuals().warn_fg_color),
                );
            } else {
                ui.label("Only running UI code when there are animations or input.");
            }
            self.frame_history.ui(ui);
        });
        ui.separator();
        let ctx = ui.ctx().clone();
        ctx.settings_ui(ui);
        false
    }
}
