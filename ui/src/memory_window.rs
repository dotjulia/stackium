use std::ops::Range;

use egui::{
    plot::{PlotPoint, PlotUi, Text},
    RichText,
};
use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, DiscoveredVariable, Registers};
use url::Url;

use crate::{command::dispatch_command_and_then, debugger_window::DebuggerWindowImpl};

pub struct MemoryWindow {
    backend_url: Url,
    variables: Promise<Result<Vec<DiscoveredVariable>, String>>,
    registers: Promise<Result<Registers, String>>,
}

impl MemoryWindow {
    pub fn new(backend_url: Url) -> Self {
        let mut ret = Self {
            backend_url,
            variables: Promise::from_ready(Err(String::new())),
            registers: Promise::from_ready(Err(String::new())),
        };
        ret.dirty();
        ret
    }
}

fn render_variable(
    variable: &DiscoveredVariable,
    ui: &mut PlotUi,
    stack_range: Range<u64>,
    addr_spacing: f32,
) {
    if variable.address >= stack_range.start && variable.address < stack_range.end {
        let position = PlotPoint::new(
            0,
            (variable.address - stack_range.start) as f32 * addr_spacing,
        );
        ui.text(Text::new(
            position,
            RichText::new(format!("{:x}", variable.address)).font(egui::FontId {
                size: 13.0,
                family: egui::FontFamily::Monospace,
            }),
        ));
    }
}

impl DebuggerWindowImpl for MemoryWindow {
    fn dirty(&mut self) {
        self.variables = dispatch!(
            self.backend_url.clone(),
            Command::DiscoverVariables,
            DiscoveredVariables
        );
        self.registers = dispatch!(self.backend_url.clone(), Command::GetRegister, Registers);
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
        if let (Some(Ok(variables)), Some(Ok(registers))) =
            (self.variables.ready(), self.registers.ready())
        {
            let addr_spacing = 10.0f32;
            egui::plot::Plot::new("Memory")
                .height(600f32)
                .show_axes([false, false])
                .allow_zoom(false)
                .show(ui, |ui| {
                    let stack_range = registers.rsp..registers.rbp;
                    for (index, addr) in stack_range.enumerate() {
                        let position = PlotPoint::new(0, index as f32 * addr_spacing);
                        ui.text(Text::new(
                            position,
                            RichText::new(format!("{:x}", addr)).font(egui::FontId {
                                size: 13.0,
                                family: egui::FontFamily::Monospace,
                            }),
                        ));
                    }
                });
        } else {
            ui.spinner();
        }
        (false, ui.label("test"))
    }
}
