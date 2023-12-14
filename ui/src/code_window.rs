use egui::{CollapsingHeader, ComboBox, RichText, ScrollArea};
use poll_promise::Promise;
use stackium_shared::{Breakpoint, BreakpointPoint, Command, CommandOutput, Location};
use url::Url;

use crate::{
    command::dispatch_command_and_then,
    debugger_window::DebuggerWindowImpl,
    syntax_highlighting::{code_view_ui, CodeTheme},
};

#[derive(PartialEq)]
enum Selected {
    Code,
    Disassemble,
}

pub struct CodeWindow {
    backend_url: Url,
    files: Promise<Result<Vec<String>, String>>,
    selected_file: String,
    displaying_file: String,
    file: Promise<Result<String, String>>,
    breakpoints: Promise<Result<Vec<Breakpoint>, String>>,
    create_breakpoint_request: Option<Promise<Result<(), String>>>,
    location: Promise<Result<Location, String>>,
    disassembly: Promise<Result<String, String>>,
    selected_window: Selected,
    pc: Promise<Result<u64, String>>,
}

impl CodeWindow {
    pub fn new(backend_url: Url) -> Self {
        let mut s = Self {
            backend_url: backend_url.clone(),
            files: Promise::from_ready(Err(String::new())),
            selected_file: String::new(),
            file: Promise::from_ready(Err(String::new())),
            displaying_file: String::new(),
            breakpoints: Promise::from_ready(Err(String::new())),
            create_breakpoint_request: None,
            location: Promise::from_ready(Err(String::new())),
            disassembly: dispatch!(backend_url, Command::Disassemble, File),
            selected_window: Selected::Code,
            pc: Promise::from_ready(Ok(0)),
        };
        s.dirty();
        s
    }
    fn render_breakpoint(ui: &mut egui::Ui, is_on: bool) -> egui::Response {
        let desired_size = ui.spacing().icon_width_inner;
        let desired_size = egui::Vec2::new(desired_size, desired_size);
        let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
        if response.clicked() {
            response.mark_changed();
        }
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, is_on, ""));
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact_selectable(&response, is_on);
            if is_on {
                ui.painter().circle_filled(
                    rect.center(),
                    desired_size.x / 2.,
                    visuals.fg_stroke.color,
                );
            } else {
                ui.painter()
                    .circle_stroke(rect.center(), desired_size.x / 2., visuals.fg_stroke);
            }
        }
        response
    }
    fn render_disassembly(&mut self, ui: &mut egui::Ui, disassembly: String) -> bool {
        let mut dirty = false;
        ui.horizontal(|ui| {
            ui.label("Program Counter: ");
            match self.pc.ready() {
                Some(pc) => match pc {
                    Ok(pc) => ui.label(format!("{:#x?}", pc)),
                    Err(e) => ui.label(e),
                },
                None => ui.spinner(),
            }
        });
        ScrollArea::both()
            .auto_shrink([false; 2])
            .max_height(400.)
            .show_viewport(ui, |ui, _| {
                ui.set_height(30.);
                // ui.style_mut().wrap = Some(false);
                ui.vertical(|ui| {
                    ui.add_space(2. * ui.spacing().item_spacing.y);
                    for line in disassembly.lines() {
                        ui.add_space(-2. * ui.spacing().item_spacing.y);
                        ui.horizontal(|ui| {
                            let current_address =
                                line.split("\t").next().unwrap_or("").replace(":", "");
                            let current_address = current_address.trim();
                            let current_address =
                                u64::from_str_radix(&current_address, 16).unwrap_or(0);
                            match self.pc.ready() {
                                Some(pc) => match pc {
                                    Ok(pc) => {
                                        let is_current = current_address == *pc;
                                        match self.breakpoints.ready() {
                                            Some(breakpoints) => match breakpoints {
                                                Ok(breakpoints) => {
                                                    let has_breakpoint = breakpoints
                                                        .iter()
                                                        .any(|b| b.address == current_address);
                                                    if  has_breakpoint {
                                                        if Self::render_breakpoint(ui, true).clicked() {
                                                            self.create_breakpoint_request = Some(dispatch_command_and_then(self.backend_url.clone(), Command::DeleteBreakpoint(current_address), |_| {}));
                                                            dirty = true;
                                                        }
                                                    } else {
                                                        if Self::render_breakpoint(ui, false)
                                                            .clicked()
                                                        {
                                                            self.create_breakpoint_request = Some(dispatch_command_and_then(self.backend_url.clone(), Command::SetBreakpoint(BreakpointPoint::Address(current_address)), |_| {}));
                                                            dirty = true;
                                                        }
                                                    }
                                                }
                                                Err(_) => {}
                                            },
                                            None => {}
                                        };

                                        if is_current {
                                            let (rect, _) = ui.allocate_exact_size(
                                                egui::Vec2::new(7. * line.len() as f32, 15.),
                                                egui::Sense::hover(),
                                            );
                                            ui.painter().rect_filled(
                                                rect,
                                                2.,
                                                egui::Color32::LIGHT_GREEN,
                                            );
                                            ui.put(rect, |ui: &mut egui::Ui| {
                                                ui.with_layout(
                                                    egui::Layout::left_to_right(egui::Align::Min),
                                                    |ui| {
                                                        code_view_ui(
                                                            ui,
                                                            line,
                                                            &CodeTheme::from_style(ui.style()),

                                                            "asm"
                                                        )
                                                    },
                                                )
                                                .response
                                            });
                                        } else {
                                            code_view_ui(
                                                ui,
                                                &mut line.to_owned(),
                                                            &CodeTheme::from_style(ui.style()),
                                                "asm"
                                            );
                                        }
                                    }
                                    Err(_) => {
                                        ui.spinner();
                                    }
                                },
                                None => {
                                    ui.spinner();
                                }
                            };
                        });
                    }
                });
            });
        dirty
    }
    fn render_code(&mut self, ui: &mut egui::Ui, code: &String) -> bool {
        ui.add_space(2. * ui.spacing().item_spacing.y);
        let location = match self.location.ready() {
            Some(l) => match l {
                Ok(l) => Some(l),
                Err(_) => None,
            },
            None => None,
        };
        ScrollArea::both()
            .auto_shrink([false; 2])
            .max_height(400.)
            .show_viewport(ui, |ui, _| {
                for (num, line) in code.lines().enumerate() {
                    let num = num + 1;
                    ui.vertical(|ui| {
                        // how do i specify item spacing 😭
                        ui.add_space(-2. * ui.spacing().item_spacing.y);
                        ui.horizontal(|ui| {
                            match self.breakpoints.ready() {
                                Some(breakpoints) => match breakpoints {
                                    Ok(breakpoints) => {
                                        let is_on = breakpoints.iter().any(|bp| {
                                            bp.location.file == self.displaying_file
                                                && bp.location.line == num as u64
                                        });
                                        if Self::render_breakpoint(ui, is_on).clicked() {
                                            if is_on {
                                                self.create_breakpoint_request =
                                                    Some(dispatch_command_and_then(
                                                        self.backend_url.clone(),
                                                        Command::DeleteBreakpoint(
                                                            breakpoints
                                                                .iter()
                                                                .find(|b| {
                                                                    b.location.line == num as u64
                                                                        && b.location.file
                                                                            == self.displaying_file
                                                                })
                                                                .unwrap()
                                                                .address,
                                                        ),
                                                        |_| {},
                                                    ));
                                            } else {
                                                self.create_breakpoint_request =
                                                    Some(dispatch_command_and_then(
                                                        self.backend_url.clone(),
                                                        Command::SetBreakpoint(
                                                            BreakpointPoint::Location(Location {
                                                                line: num as u64,
                                                                file: self.displaying_file.clone(),
                                                                column: 0,
                                                            }),
                                                        ),
                                                        |_| {},
                                                    ));
                                            }
                                        };
                                    }
                                    Err(_) => {
                                        ui.label(
                                            RichText::new("x").color(ui.visuals().error_fg_color),
                                        );
                                    }
                                },
                                None => {
                                    Self::render_breakpoint(ui, false);
                                }
                            };
                            ui.label(num.to_string());

                            if match location {
                                Some(l) => l.line == num as u64,
                                None => false,
                            } {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::Vec2::new(6.6 * line.len() as f32, 15.),
                                    egui::Sense::hover(),
                                );
                                ui.painter()
                                    .rect_filled(rect, 2., egui::Color32::LIGHT_GREEN);
                                ui.put(rect, |ui: &mut egui::Ui| {
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Min),
                                        |ui| {
                                            code_view_ui(
                                                ui,
                                                line,
                                                &CodeTheme::from_style(ui.style()),
                                                "c",
                                            )
                                        },
                                    )
                                    .response
                                });
                            } else {
                                code_view_ui(ui, line, &CodeTheme::from_style(ui.style()), "c");
                            }
                        });
                    });
                }
            });

        let mut dirty = false;
        match &self.create_breakpoint_request {
            Some(req) => match req.ready() {
                Some(req) => match req {
                    Ok(_) => {
                        dirty = true;
                    }
                    Err(_) => {}
                },
                None => {
                    ui.spinner();
                }
            },
            None => {}
        };
        if dirty {
            self.create_breakpoint_request = None;
        }

        dirty
    }
}

impl DebuggerWindowImpl for CodeWindow {
    fn update(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.displaying_file != self.selected_file {
            self.displaying_file = self.selected_file.clone();
            self.file = dispatch_command_and_then(
                self.backend_url.clone(),
                Command::GetFile(self.selected_file.clone()),
                |output| match output {
                    CommandOutput::File(file) => file,
                    _ => unreachable!(),
                },
            );
        }
    }
    fn dirty(&mut self) {
        self.files = dispatch_command_and_then(
            self.backend_url.clone(),
            stackium_shared::Command::DebugMeta,
            |output| match output {
                CommandOutput::DebugMeta(meta) => meta.files,
                _ => unreachable!(),
            },
        );
        self.breakpoints = dispatch!(
            self.backend_url.clone(),
            Command::GetBreakpoints,
            Breakpoints
        );
        self.location = dispatch!(self.backend_url.clone(), Command::Location, Location);
        self.pc = dispatch_command_and_then(
            self.backend_url.clone(),
            Command::ProgramCounter,
            |o| match o {
                CommandOutput::Data(o) => o,
                _ => unreachable!(),
            },
        );
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.selected_window, Selected::Code, "Code");
            ui.selectable_value(
                &mut self.selected_window,
                Selected::Disassemble,
                "Disassemble",
            );
        });
        let mut dirty = false;
        if self.selected_window == Selected::Code {
            match self.files.ready() {
                Some(files) => match files {
                    Ok(files) => {
                        if files.len() > 0 && self.selected_file.len() == 0 {
                            self.selected_file = files.first().unwrap().clone();
                        }
                        ComboBox::from_label("File")
                            .selected_text(self.selected_file.clone())
                            .show_ui(ui, |ui| {
                                for file in files {
                                    ui.selectable_value(
                                        &mut self.selected_file,
                                        file.clone(),
                                        file,
                                    );
                                }
                            });
                    }
                    Err(err) => {
                        ui.label(err);
                    }
                },
                None => {
                    ui.spinner();
                }
            }
            match self.file.ready() {
                Some(code) => match code {
                    Ok(code) => {
                        let code = code.clone();
                        dirty = self.render_code(ui, &code);
                    }
                    Err(err) => {
                        ui.label(err);
                    }
                },
                None => {
                    ui.spinner();
                }
            }
        } else {
            match self.disassembly.ready() {
                Some(disassembly) => match disassembly {
                    Ok(disassembly) => {
                        let disassembly = disassembly.clone();
                        dirty = self.render_disassembly(ui, disassembly);
                    }
                    Err(err) => {
                        ui.label(err);
                    }
                },
                None => {
                    ui.spinner();
                }
            }
        }

        // CollapsingHeader::new("Theme").show(ui, |ui| {
        // self.code_theme.ui(ui);
        // });
        (dirty, ui.label("Code window"))
    }
}
