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
                    egui_extras::TableBuilder::new(ui)
                        .striped(true)
                        .column(egui_extras::Column::auto().at_least(80.).resizable(true))
                        .column(egui_extras::Column::auto().at_least(100.).resizable(true))
                        .column(egui_extras::Column::remainder())
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.heading("Name");
                            });
                            header.col(|ui| {
                                ui.heading("Address");
                            });
                            header.col(|ui| {
                                ui.heading("Content");
                            });
                        })
                        .body(|mut body| {
                            let mut sorted_variables = variables.clone();

                            sorted_variables
                                .sort_by(|b, a| a.addr.unwrap_or(0).cmp(&b.addr.unwrap_or(0)));
                            for variable in sorted_variables.iter() {
                                if let (Some(address), Some(value)) =
                                    (variable.addr, variable.value)
                                {
                                    body.row(30.0, |mut row| {
                                        row.col(|ui| {
                                            ui.label(format!(
                                                "{}: {}",
                                                variable
                                                    .name
                                                    .clone()
                                                    .unwrap_or("unknown".to_owned()),
                                                variable
                                                    .type_name
                                                    .clone()
                                                    .unwrap_or(stackium_shared::TypeName::Name(
                                                        "??".to_owned()
                                                    ))
                                                    .to_string()
                                            ));
                                        });
                                        row.col(|ui| {
                                            ui.label(format!("{:#x}", address));
                                        });
                                        row.col(|ui| {
                                            ui.label(format!("{:#x}", value));
                                        });
                                    });
                                }
                            }
                        });
                    ui.separator()
                }
                Err(err) => ui.label(err),
            },
            None => ui.spinner(),
        };
        (false, res)
    }
}
