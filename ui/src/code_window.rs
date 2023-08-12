use egui::{CollapsingHeader, ComboBox};
use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput};
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
    code_theme: CodeTheme,
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
        };
        s.dirty();
        s
    }
}

impl DebuggerWindowImpl for CodeWindow {
    fn dirty(&mut self) {
        self.files = dispatch_command_and_then(
            self.backend_url.clone(),
            stackium_shared::Command::DebugMeta,
            |output| match output {
                CommandOutput::DebugMeta(meta) => meta.files,
                _ => unreachable!(),
            },
        )
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
        match self.files.ready() {
            Some(files) => match files {
                Ok(files) => {
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
        match self.file.ready() {
            Some(code) => match code {
                Ok(code) => {
                    code_view_ui(ui, code, &self.code_theme);
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
        (false, ui.label("Code window"))
    }
}
