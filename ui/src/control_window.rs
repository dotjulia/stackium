use egui::RichText;
use poll_promise::Promise;
use stackium_shared::Command;
use url::Url;

use crate::{command::dispatch_command_and_then, debugger_window::DebuggerWindowImpl};

pub struct ControlWindow {
    promise: Option<Promise<Result<(), String>>>,
    backend_url: Url,
    warning: Option<String>,
}

impl ControlWindow {
    pub fn new(backend_url: Url) -> Self {
        Self {
            promise: None,
            backend_url,
            warning: None,
        }
    }
}

impl DebuggerWindowImpl for ControlWindow {
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        match &self.promise {
            Some(promise) => match promise.ready() {
                Some(result) => match result {
                    Ok(_) => {
                        dirty = true;
                        self.promise = None;
                        ui.spinner()
                    }
                    Err(err) => {
                        self.warning = Some(err.clone());
                        ui.spinner()
                    }
                },
                None => ui.spinner(),
            },
            None => {
                let r = ui.button("Continue");
                // if ui.button("Step Over").clicked() {
                //     self.promise = Some(dispatch_command_and_then(
                //         self.backend_url.clone(),
                //         Command::StepOut,
                //         |_| {},
                //     ));
                // }

                // if ui.button("Step In").clicked() {
                //     self.promise = Some(dispatch_command_and_then(
                //         self.backend_url.clone(),
                //         Command::StepIn,
                //         |_| {},
                //     ));
                // }

                if ui.button("Step Instruction").clicked() {
                    self.promise = Some(dispatch_command_and_then(
                        self.backend_url.clone(),
                        Command::StepInstruction,
                        |_| {},
                    ));
                }

                if r.clicked() {
                    self.promise = Some(dispatch_command_and_then(
                        self.backend_url.clone(),
                        Command::Continue,
                        |_| {},
                    ));
                }
                r
            }
        };
        if let Some(warning) = &self.warning {
            ui.label(RichText::new(format!("⚠ {}", warning)).color(ui.visuals().warn_fg_color));
        }
        dirty
    }
}
