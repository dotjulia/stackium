use egui::{ComboBox, RichText};
use poll_promise::Promise;
use stackium_shared::{Breakpoint, BreakpointPoint, Command, CommandOutput};
use url::Url;

use crate::{command::dispatch_command_and_then, debugger_window::DebuggerWindowImpl};

#[derive(PartialEq)]
enum Selection {
    Address,
    Function,
}

impl std::fmt::Debug for Selection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Selection::Address => f.write_str("Address"),
            Selection::Function => f.write_str("Function"),
        }
    }
}

pub struct BreakpointWindow {
    breakpoints: Promise<Result<Vec<Breakpoint>, String>>,
    selected: Selection,
    selection_input: String,
    backend_url: Url,
    warning: Option<String>,
    adding_breakpoint_req: Option<Promise<Result<(), String>>>,
}

impl BreakpointWindow {
    pub fn new(backend_url: Url) -> Self {
        Self {
            breakpoints: dispatch!(backend_url.clone(), Command::GetBreakpoints, Breakpoints),
            selected: Selection::Function,
            selection_input: "main".to_owned(),
            backend_url,
            warning: None,
            adding_breakpoint_req: None,
        }
    }
}

impl DebuggerWindowImpl for BreakpointWindow {
    fn dirty(&mut self) {
        self.breakpoints = dispatch!(
            self.backend_url.clone(),
            Command::GetBreakpoints,
            Breakpoints
        );
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
        let mut is_dirty = false;
        match self.breakpoints.ready() {
            Some(breakpoints) => match breakpoints {
                Ok(breakpoints) => {
                    ui.heading("Breakpoints");
                    for breakpoint in breakpoints.iter() {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "{} {}:{} @ {:#x}",
                                breakpoint.location.file,
                                breakpoint.location.line,
                                breakpoint.location.column,
                                breakpoint.address
                            ));
                            if ui
                                .button(if breakpoint.enabled {
                                    "disable"
                                } else {
                                    "enable"
                                })
                                .clicked()
                            {
                                self.adding_breakpoint_req = Some(dispatch_command_and_then(
                                    self.backend_url.clone(),
                                    Command::DeleteBreakpoint(breakpoint.address),
                                    |_| {},
                                ));
                            }
                        });
                    }
                    ui.add_space(10.);
                }
                Err(err) => {
                    ui.label(err);
                }
            },
            None => {
                ui.spinner();
            }
        };
        ui.horizontal(|ui| {
            ComboBox::new("Address or Function", "")
                .selected_text(format!("{:?}", self.selected))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.selected, Selection::Address, "Address");
                    ui.selectable_value(&mut self.selected, Selection::Function, "Function");
                });
            ui.text_edit_singleline(&mut self.selection_input);

            if let Some(req) = &mut self.adding_breakpoint_req {
                match req.ready() {
                    Some(res) => {
                        is_dirty = true;
                        match res {
                            Ok(_) => self.adding_breakpoint_req = None,
                            Err(err) => {
                                self.warning = Some(err.clone());
                            }
                        }
                    }
                    None => {
                        ui.spinner();
                    }
                }
            }
            if ui.button("add").clicked() {
                let bp = match self.selected {
                    Selection::Address => {
                        if self.selection_input.starts_with("0x") {
                            let wo_pre = self.selection_input.trim_start_matches("0x");
                            let addr = u64::from_str_radix(wo_pre, 16);
                            match addr {
                                Ok(addr) => Some(BreakpointPoint::Address(addr)),
                                Err(_) => None,
                            }
                        } else {
                            match u64::from_str_radix(&self.selection_input, 10) {
                                Ok(addr) => Some(BreakpointPoint::Address(addr)),
                                Err(_) => None,
                            }
                        }
                    }
                    Selection::Function => {
                        Some(BreakpointPoint::Name(self.selection_input.clone()))
                    }
                };
                if let Some(bp) = bp {
                    self.warning = None;
                    self.adding_breakpoint_req = Some(dispatch_command_and_then(
                        self.backend_url.clone(),
                        Command::SetBreakpoint(bp),
                        |_| (),
                    ));
                } else {
                    self.warning = Some("Failed parsing number".to_owned());
                }
            }
        });
        match self.selected {
            Selection::Address => {
                if self.selection_input.starts_with("0x") {
                    ui.label(
                        RichText::new("⚠ parsing address as hex")
                            .small()
                            .color(ui.visuals().warn_fg_color),
                    );
                } else {
                    ui.label(
                        RichText::new("⚠ parsing address as dec")
                            .small()
                            .color(ui.visuals().warn_fg_color),
                    );
                }
            }
            Selection::Function => {}
        };
        let ret = if let Some(warning) = &self.warning {
            ui.label(
                RichText::new(format!("⚠ {}", warning))
                    .small()
                    .color(ui.visuals().warn_fg_color),
            )
        } else {
            ui.label("")
        };
        (is_dirty, ret)
    }
}
