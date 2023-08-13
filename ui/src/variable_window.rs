use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, Variable};
use url::Url;

use crate::debugger_window::DebuggerWindowImpl;

pub struct VariableWindow {
    variables: Promise<Result<Vec<Variable>, String>>,
    backend_url: Url,
}

impl VariableWindow {
    pub fn new(backend_url: Url) -> Self {
        let mut s = Self {
            variables: Promise::from_ready(Err(String::new())),
            backend_url,
        };
        s.dirty();
        s
    }
}

impl DebuggerWindowImpl for VariableWindow {
    fn dirty(&mut self) {
        self.variables = dispatch!(self.backend_url.clone(), Command::ReadVariables, Variables);
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
        let res = match self.variables.ready() {
            Some(variables) => match variables {
                Ok(variables) => {
                    for variable in variables.iter() {
                        ui.horizontal(|ui| {
                            ui.label(variable.name.clone().unwrap_or("undefined".to_owned()));
                            ui.label(format!("{:?}", variable.type_name));
                            ui.label(format!("{:#x?}", variable.value.unwrap_or(0)));
                            ui.label(format!("{:#x?}", variable.addr.unwrap_or(0)));
                        });
                        ui.separator();
                    }
                    ui.separator()
                }
                Err(err) => ui.label(err),
            },
            None => ui.spinner(),
        };
        (false, res)
    }
}
