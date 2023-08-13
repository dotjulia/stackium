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
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
        let mut dirty = false;
        let response = match &self.promise {
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
        (dirty, response)
    }
}
