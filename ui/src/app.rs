use egui::{load::SizedTexture, Align, Layout, TextureHandle};
use egui_dock::{DockArea, DockState, TabViewer};
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
    memory_window::MemoryWindow,
    register_window::RegisterWindow,
    settings_window::SettingsWindow,
    toggle::toggle_ui,
};

enum State {
    Debugging {
        backend_url: Url,
        sidebar_open: bool,
        metadata: Promise<Result<DebugMeta, String>>,
        dockable_windows: DockState<&'static str>,
        icon: Option<TextureHandle>,
        mapping: Promise<Result<(), String>>,
        restart_request: Option<Promise<Result<(), String>>>,
        tab_viewer: CustomTabViewer,
    },
    UnrecoverableFailure {
        message: String,
        restart_request: Option<Promise<Result<(), String>>>,
    },
}

impl State {
    fn construct_debugging_state(backend_url: &Url) -> Self {
        let tab_viewer = CustomTabViewer {
            dirty: false,
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
                    is_active: true,
                    body: Box::from(MemoryWindow::new(backend_url.clone())),
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
                    body: Box::from(MapWindow::new(backend_url.clone())),
                },
            ],
        };
        let mut dock_state = DockState::new(vec!["Memory"]);
        let [_, left] = dock_state.main_surface_mut().split_left(
            egui_dock::NodeIndex::root(),
            0.5,
            vec!["Code"],
        );
        let [_, bottom] = dock_state
            .main_surface_mut()
            .split_below(left, 0.7, vec!["Controls"]);
        dock_state
            .main_surface_mut()
            .split_right(bottom, 0.3, vec!["Breakpoints"]);
        Self::Debugging {
            icon: None,
            sidebar_open: true,
            backend_url: backend_url.clone(),
            metadata: { dispatch!(backend_url.clone(), Command::DebugMeta, DebugMeta) },
            mapping: { dispatch_command_and_then(backend_url.clone(), Command::Maps, |maps| {}) },
            dockable_windows: dock_state,
            tab_viewer,
            restart_request: None,
        }
    }
}

struct CustomTabViewer {
    dirty: bool,
    windows: Vec<DebuggerWindow>,
}

impl TabViewer for CustomTabViewer {
    type Tab = &'static str;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.to_string().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab_name: &mut Self::Tab) {
        let tab = self
            .windows
            .iter_mut()
            .find(|w| w.title == *tab_name)
            .unwrap();
        let dirty = tab.body.ui(ui);
        if dirty {
            self.dirty = true;
        }
    }
    fn on_close(&mut self, _tab: &mut Self::Tab) -> bool {
        self.windows
            .iter_mut()
            .find(|w| w.title == *_tab)
            .unwrap()
            .is_active = false;
        true
    }
}

// state concept does not work well with current error handling design, could be improved
pub struct StackiumApp {
    backend_url: Url,
    state: State,
    next_state: Option<State>,
}

impl StackiumApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let backend_url = Url::parse("http://localhost:8080").unwrap();
        Self {
            state: State::construct_debugging_state(&backend_url),
            backend_url,
            next_state: None,
        }
    }
}

impl eframe::App for StackiumApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(next_state) = self.next_state.take() {
            self.state = next_state;
        }
        if let State::Debugging {
            sidebar_open: _,
            backend_url: _,
            metadata: _,
            dockable_windows: _,
            tab_viewer,
            icon: _,
            mapping,
            restart_request: _,
        } = &mut self.state
        {
            if let Some(Err(_)) = mapping.ready() {
                self.next_state = Some(State::UnrecoverableFailure {
                    message: "Child process exited".to_owned(),
                    restart_request: None,
                });
                // return;
            }
            for window in tab_viewer.windows.iter_mut() {
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
                // #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        // frame.
                    }
                });

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        match &mut self.state {
            State::Debugging {
                backend_url,
                metadata,
                sidebar_open,
                dockable_windows,
                icon,
                mapping,
                tab_viewer,
                restart_request,
            } => {
                tab_viewer.dirty = false;

                if let Some(Some(Ok(p))) = restart_request.as_mut().map(|p| p.ready()) {
                    *restart_request = None;
                    tab_viewer.dirty = true;
                }

                egui::SidePanel::left("side_panel").show_animated(ctx, *sidebar_open, |ui| {
                    ui.horizontal(|ui| {
                        egui::widgets::global_theme_preference_buttons(ui);
                        if ui
                            .add(egui::Button::new("↻ Restart").fill(ui.visuals().window_fill))
                            .clicked()
                        {
                            *restart_request = Some(dispatch_command_and_then(
                                backend_url.clone(),
                                Command::RestartDebugee,
                                |_| {},
                            ));
                        }
                    });
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
                        ui.image(SizedTexture::new(
                            &mut texture.clone(),
                            egui::Vec2::new(100., 100.),
                        ));
                        ui.heading("Stackium");
                        ui.add_space(20.);
                    });
                    ui.heading("Windows");
                    for window in tab_viewer.windows.iter_mut() {
                        ui.horizontal(|ui| {
                            if ui.label(window.title).clicked() {
                                window.is_active = !window.is_active;
                            }
                            ui.with_layout(
                                Layout::left_to_right(Align::Max)
                                    .with_main_align(Align::Max)
                                    .with_main_justify(true),
                                |ui| {
                                    if toggle_ui(ui, &mut window.is_active).changed() {
                                        if window.is_active {
                                            dockable_windows.add_window(vec![window.title]);
                                        } else {
                                            let mut to_remove = None;

                                            // I see no other way of iterating over all tabs and getting all 3 (surface_index, node_index, tab_index)
                                            for (surface_index, surface) in
                                                dockable_windows.iter_surfaces().enumerate()
                                            {
                                                for (node_index, node) in
                                                    surface.iter_nodes().enumerate()
                                                {
                                                    for (tab_index, tab) in
                                                        node.iter_tabs().enumerate()
                                                    {
                                                        if tab == &window.title {
                                                            to_remove = Some((
                                                                egui_dock::SurfaceIndex(
                                                                    surface_index,
                                                                ),
                                                                egui_dock::NodeIndex(node_index),
                                                                egui_dock::TabIndex(tab_index),
                                                            ));
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                            dockable_windows.remove_tab(to_remove.unwrap());
                                        }
                                    }
                                },
                            );
                        });
                    }
                    ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                        ui.horizontal(|ui| {
                            ui.image(SizedTexture::new(texture, egui::Vec2::new(20., 20.)));
                            ui.hyperlink_to(
                                format!("Stackium {}", egui::special_emojis::GITHUB),
                                "https://github.com/dotjulia/stackium",
                            );
                            ui.label("made with ♥ by");
                            ui.hyperlink_to("dotjulia", "juli.zip")
                        });
                    });
                });

                egui::CentralPanel::default()
                    .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(0.))
                    .show(ctx, |ui| match metadata.ready() {
                        Some(m) => match m {
                            Ok(m) => {
                                if !*sidebar_open {
                                    if ui.button("Open Sidebar").clicked() {
                                        *sidebar_open = true;
                                    }
                                }

                                DockArea::new(dockable_windows)
                                    .style(egui_dock::Style::from_egui(ui.style()))
                                    .draggable_tabs(true)
                                    .show_close_buttons(true)
                                    .show_window_close_buttons(false)
                                    // .allowed_splits(self.context.allowed_splits)
                                    .show_window_collapse_buttons(true)
                                    .show_inside(ui, tab_viewer);
                                if tab_viewer.dirty {
                                    tab_viewer.dirty = false;
                                    tab_viewer.windows.iter_mut().for_each(|w| w.body.dirty());
                                    *mapping = dispatch_command_and_then(
                                        backend_url.clone(),
                                        Command::Maps,
                                        |_| {},
                                    )
                                }
                            }
                            Err(e) => {
                                self.next_state = Some(State::UnrecoverableFailure {
                                    message: e.clone(),
                                    restart_request: None,
                                });
                                ui.heading("Loading...".to_owned());
                            }
                        },
                        None => {
                            ui.heading("Loading...".to_owned());
                        }
                    });
            }
            State::UnrecoverableFailure {
                message,
                restart_request,
            } => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    match restart_request.as_mut().map(|p| p.ready()) {
                        Some(Some(Ok(p))) => {
                            *restart_request = None;
                            self.next_state =
                                Some(State::construct_debugging_state(&self.backend_url));
                            return;
                        }
                        Some(Some(Err(e))) => {
                            self.next_state = Some(State::UnrecoverableFailure {
                                message: format!("Restart failed: {}\n Please try manually restarting the debugger in the terminal.", e),
                                restart_request: None,
                            });
                            return;
                        }
                        Some(None) => {
                            ui.spinner();
                            return;
                        }
                        None => {}
                    }

                    ui.heading("Error");
                    ui.label(message.clone());
                    ui.label("Please restart the debugger".to_owned());
                    if ui
                        .add(egui::Button::new("↻ Restart Process").fill(ui.visuals().window_fill))
                        .clicked()
                    {
                        *restart_request = Some(dispatch_command_and_then(
                            self.backend_url.clone(),
                            Command::RestartDebugee,
                            |_| {},
                        ));
                    }
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
