use egui::{Align, Layout, TextureHandle};
use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, DebugMeta};
use url::Url;

use crate::{
    breakpoint_window::BreakpointWindow,
    code_window::CodeWindow,
    command::{dispatch, dispatch_command_and_then},
    control_window::ControlWindow,
    debugger_window::{DebuggerWindow, Metadata},
    graph_window::GraphWindow,
    location::LocationWindow,
    map_window::MapWindow,
    register_window::RegisterWindow,
    settings_window::SettingsWindow,
    toggle::toggle_ui,
    variable_window::VariableWindow,
};

enum State {
    Debugging {
        backend_url: Url,
        sidebar_open: bool,
        metadata: Promise<Result<DebugMeta, String>>,
        windows: Vec<DebuggerWindow>,
        icon: Option<TextureHandle>,
        mapping: Promise<Result<(), String>>,
        fullscreen: Option<&'static str>,
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
                icon: None,
                sidebar_open: true,
                backend_url: backend_url.clone(),
                metadata: { dispatch!(backend_url.clone(), Command::DebugMeta, DebugMeta) },
                mapping: {
                    dispatch_command_and_then(backend_url.clone(), Command::Maps, |maps| {})
                },
                windows: vec![
                    DebuggerWindow {
                        title: "Metadata",
                        is_active: false,
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
                        is_active: true,
                        body: Box::from(CodeWindow::new(backend_url.clone())),
                    },
                    DebuggerWindow {
                        title: "Settings",
                        is_active: false,
                        body: Box::from(SettingsWindow::new()),
                    },
                    DebuggerWindow {
                        title: "Controls",
                        is_active: true,
                        body: Box::from(ControlWindow::new(backend_url.clone())),
                    },
                    DebuggerWindow {
                        title: "Memory",
                        is_active: false,
                        body: Box::from(VariableWindow::new(backend_url.clone())),
                    },
                    DebuggerWindow {
                        title: "Graph",
                        is_active: false,
                        body: Box::from(GraphWindow::new(backend_url.clone())),
                    },
                    DebuggerWindow {
                        title: "Registers",
                        is_active: false,
                        body: Box::from(RegisterWindow::new(backend_url.clone())),
                    },
                    DebuggerWindow {
                        title: "Memory Mapping",
                        is_active: false,
                        body: Box::from(MapWindow::new(backend_url)),
                    },
                ],
                fullscreen: None,
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

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let State::Debugging {
            sidebar_open: _,
            backend_url: _,
            metadata: _,
            windows,
            icon: _,
            fullscreen: _,
            mapping,
        } = &mut self.state
        {
            if let Some(Err(_)) = mapping.ready() {
                self.next_state = Some(State::UnrecoverableFailure {
                    message: "Child process exited".to_owned(),
                });
                return;
            }
            for window in windows {
                window.body.update(ctx, frame);
            }
        }

        egui::TopBottomPanel::bottom("debug warning").show(ctx, |ui| {
            egui::warn_if_debug_build(ui);
        });

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        frame.close();
                    }
                });
            });
        });

        match &mut self.state {
            State::Debugging {
                backend_url,
                metadata,
                sidebar_open,
                windows,
                icon,
                fullscreen,
                mapping,
            } => {
                egui::SidePanel::left("side_pabel").show_animated(ctx, *sidebar_open, |ui| {
                    let texture = icon.get_or_insert_with(|| {
                        let icon = include_bytes!("../assets/icon-1024.png");
                        let image = match load_image_from_memory(icon) {
                            Ok(image) => image,
                            Err(_) => egui::ColorImage::example(),
                        };
                        ui.ctx()
                            .load_texture("icon-1024", image, Default::default())
                    });
                    ui.with_layout(Layout::top_down(Align::Max), |ui| {
                        if ui.button("X").clicked() {
                            *sidebar_open = false;
                        }
                    });

                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.add_space(10.);
                        ui.image(&mut texture.clone(), egui::Vec2::new(100., 100.));
                        ui.heading("Stackium");
                        ui.add_space(20.);
                    });
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
                            ui.image(texture, egui::Vec2::new(20., 20.));
                            ui.hyperlink_to(
                                format!("Stackium {}", egui::special_emojis::GITHUB),
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
                            if !*sidebar_open {
                                if ui.button("Open Sidebar").clicked() {
                                    *sidebar_open = true;
                                }
                            }

                            let mut is_dirty = false;
                            let mut close_fullscreen = false;
                            if let Some(fullscreen) = fullscreen {
                                if ui.button("Exit Fullscreen").clicked() {
                                    close_fullscreen = true;
                                }
                                let window =
                                    windows.iter_mut().find(|w| &w.title == fullscreen).unwrap();
                                window.body.ui(ui);
                            } else {
                                ui.heading(format!("Debugging {}", m.binary_name));
                                ui.label(format!("Number of functions: {}", m.functions));
                                ui.label(format!("Number of variables: {}", m.vars));
                                ui.label(format!("Files: {}", m.files.join(", ")));
                                for window in windows.iter_mut() {
                                    if window.is_active {
                                        egui::Window::new(window.title)
                                            .open(&mut window.is_active)
                                            .show(ctx, |ui| {
                                                ui.with_layout(
                                                    Layout::top_down(Align::Max),
                                                    |ui| {
                                                        if ui.button("Fullscreen").clicked() {
                                                            *fullscreen = Some(window.title);
                                                        }
                                                    },
                                                );
                                                let (dirty, res) = window.body.ui(ui);
                                                if dirty {
                                                    is_dirty = true;
                                                }
                                                res
                                            });
                                    }
                                }
                            }
                            if close_fullscreen {
                                *fullscreen = None;
                            }
                            if is_dirty {
                                windows.iter_mut().for_each(|w| w.body.dirty());
                                *mapping = dispatch_command_and_then(
                                    backend_url.clone(),
                                    Command::Maps,
                                    |maps| {},
                                )
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

fn load_image_from_memory(image_data: &[u8]) -> Result<egui::ColorImage, image::ImageError> {
    let image = image::load_from_memory(image_data)?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}
