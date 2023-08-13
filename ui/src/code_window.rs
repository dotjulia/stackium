use egui::{CollapsingHeader, ComboBox, RichText};
use poll_promise::Promise;
use stackium_shared::{Breakpoint, BreakpointPoint, Command, CommandOutput, Location};
use url::Url;

use crate::{
    command::dispatch_command_and_then,
    debugger_window::DebuggerWindowImpl,
    syntax_highlighting::{code_view_ui, CodeTheme},
};

pub struct CodeWindow {
    backend_url: Url,
    files: Promise<Result<Vec<String>, String>>,
    selected_file: String,
    displaying_file: String,
    file: Promise<Result<String, String>>,
    breakpoints: Promise<Result<Vec<Breakpoint>, String>>,
    code_theme: CodeTheme,
    create_breakpoint_request: Option<Promise<Result<(), String>>>,
}

impl CodeWindow {
    pub fn new(backend_url: Url) -> Self {
        let mut s = Self {
            backend_url: backend_url.clone(),
            files: Promise::from_ready(Err(String::new())),
            selected_file: String::new(),
            file: Promise::from_ready(Err(String::new())),
            displaying_file: String::new(),
            code_theme: Default::default(),
            breakpoints: Promise::from_ready(Err(String::new())),
            create_breakpoint_request: None,
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
    fn render_code(&mut self, ui: &mut egui::Ui, code: &String) -> bool {
        ui.add_space(2. * ui.spacing().item_spacing.y);
        for (num, line) in code.lines().enumerate() {
            let num = num + 1;
            ui.vertical(|ui| {
                // how do i specify item spacing 😭
                ui.add_space(-2. * ui.spacing().item_spacing.y);
                ui.horizontal(|ui| {
                    match self.breakpoints.ready() {
                        Some(breakpoints) => match breakpoints {
                            Ok(breakpoints) => {
                                if Self::render_breakpoint(
                                    ui,
                                    breakpoints.iter().any(|bp| {
                                        bp.location.file == self.displaying_file
                                            && bp.location.line == num as u64
                                    }),
                                )
                                .clicked()
                                {
                                    self.create_breakpoint_request =
                                        Some(dispatch_command_and_then(
                                            self.backend_url.clone(),
                                            Command::SetBreakpoint(BreakpointPoint::Location(
                                                Location {
                                                    line: num as u64,
                                                    file: self.displaying_file.clone(),
                                                    column: 0,
                                                },
                                            )),
                                            |_| {},
                                        ));
                                };
                            }
                            Err(_) => {
                                ui.label(RichText::new("x").color(ui.visuals().error_fg_color));
                            }
                        },
                        None => {
                            ui.spinner();
                        }
                    };
                    ui.label(num.to_string());
                    code_view_ui(ui, line, &self.code_theme);
                });
            });
        }
        false
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
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
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
                                ui.selectable_value(&mut self.selected_file, file.clone(), file);
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
        let mut dirty = false;
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

        CollapsingHeader::new("Theme").show(ui, |ui| {
            self.code_theme.ui(ui);
        });
        (dirty, ui.label("Code window"))
    }
}
