use egui::{Align, Layout};
use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, DebugMeta};
use url::Url;

use crate::{
    breakpoint_window::BreakpointWindow,
    code_window::CodeWindow,
    command::dispatch,
    debugger_window::{DebuggerWindow, Metadata},
    location::LocationWindow,
    toggle::toggle_ui,
};

enum State {
    Debugging {
        backend_url: Url,
        metadata: Promise<Result<DebugMeta, String>>,
        windows: Vec<DebuggerWindow>,
    },
    UnrecoverableFailure {
        message: String,
    },
}

pub struct StackiumApp {
    state: State,
    next_state: Option<State>,
}

impl StackiumApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let backend_url = Url::parse("http://localhost:8080").unwrap();
        Self {
            next_state: None,
            state: State::Debugging {
                backend_url: backend_url.clone(),
                metadata: { dispatch!(backend_url.clone(), Command::DebugMeta, DebugMeta) },
                windows: vec![
                    DebuggerWindow {
                        title: "Metadata",
                        is_active: true,
                        body: Box::from(Metadata::new(backend_url.clone())),
                    },
                    DebuggerWindow {
                        title: "Location",
                        is_active: false,
                        body: Box::from(LocationWindow::new(backend_url.clone())),
                    },
                    DebuggerWindow {
                        title: "Breakpoints",
                        is_active: true,
                        body: Box::from(BreakpointWindow::new(backend_url.clone())),
                    },
                    DebuggerWindow {
                        title: "Code",
                        is_active: false,
                        body: Box::from(CodeWindow::new(backend_url)),
                    },
                ],
            },
        }
    }
}

impl eframe::App for StackiumApp {
    fn post_rendering(&mut self, _window_size_px: [u32; 2], _frame: &eframe::Frame) {
        if let Some(next_state) = self.next_state.take() {
            self.state = next_state;
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("debug warning").show(ctx, |ui| {
            egui::warn_if_debug_build(ui);
        });

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
            });
        });

        match &mut self.state {
            State::Debugging {
                backend_url: _,
                metadata,
                windows,
            } => {
                egui::SidePanel::left("side_pabel").show(ctx, |ui| {
                    ui.heading("Windows");
                    for window in windows.iter_mut() {
                        ui.horizontal(|ui| {
                            if ui.label(window.title).clicked() {
                                window.is_active = !window.is_active;
                            }
                            ui.with_layout(
                                Layout::left_to_right(Align::Max)
                                    .with_main_align(Align::Max)
                                    .with_main_justify(true),
                                |ui| {
                                    toggle_ui(ui, &mut window.is_active);
                                },
                            );
                        });
                    }
                    ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                        ui.horizontal(|ui| {
                            ui.hyperlink_to(
                                format!("{} Stackium", egui::special_emojis::GITHUB),
                                "https://github.com/dotjulia/stackium",
                            );
                            ui.label("made with ♥ by");
                            ui.hyperlink_to("dotjulia", "juli.zip")
                        });
                    });
                });

                egui::CentralPanel::default().show(ctx, |ui| match metadata.ready() {
                    Some(m) => match m {
                        Ok(m) => {
                            ui.heading(format!("Debugging {}", m.binary_name));
                            ui.label(format!("Number of functions: {}", m.functions));
                            ui.label(format!("Number of variables: {}", m.vars));
                            ui.label(format!("Files: {}", m.files.join(", ")));
                            let mut is_dirty = false;
                            for window in windows.iter_mut() {
                                if window.is_active {
                                    egui::Window::new(window.title).show(ctx, |ui| {
                                        let (dirty, res) = window.body.ui(ui);
                                        if dirty {
                                            is_dirty = true;
                                        }
                                        res
                                    });
                                }
                            }
                            if is_dirty {
                                windows.iter_mut().for_each(|w| w.body.dirty());
                            }
                        }
                        Err(e) => {
                            self.next_state =
                                Some(State::UnrecoverableFailure { message: e.clone() });
                            ui.heading("Loading...".to_owned());
                        }
                    },
                    None => {
                        ui.heading("Loading...".to_owned());
                    }
                });
            }
            State::UnrecoverableFailure { message } => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Error");
                    ui.label(message.clone());
                });
            }
        }
    }
}
