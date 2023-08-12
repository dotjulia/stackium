use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, Location};
use url::Url;

use crate::debugger_window::DebuggerWindowImpl;

pub struct LocationWindow {
    location: Promise<Result<Location, String>>,
}

impl LocationWindow {
    pub fn new(backend_url: Url) -> Self {
        Self {
            location: dispatch!(backend_url, Command::Location, Location),
        }
    }
}

impl DebuggerWindowImpl for LocationWindow {
    fn ui(&mut self, ui: &mut egui::Ui) -> egui::Response {
        match self.location.ready() {
            Some(location) => match location {
                Ok(location) => {
                    ui.label(format!("Current file: {}", location.file));
                    ui.label(format!("Location: {}:{}", location.line, location.column))
                }
                Err(err) => {
                    if err.contains("NoSource") {
                        ui.label("No source code available for the current state of the process");
                        ui.label("Try setting breakpoints or continuing the execution.")
                    } else {
                        ui.label(err)
                    }
                }
            },
            None => ui.spinner(),
        }
    }
}
