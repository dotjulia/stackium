use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, DebugMeta};
use url::Url;

use crate::command::dispatch;

enum State {
    Debugging {
        backend_url: Url,
        metadata: Promise<Result<DebugMeta, String>>,
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
                metadata: { dispatch!(backend_url, Command::DebugMeta, DebugMeta) },
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
            } => {
                egui::Window::new("Metadata").show(ctx, |ui| {
                    match metadata.ready() {
                        Some(metadata) => match metadata {
                            Ok(metadata) => {
                                ui.heading(format!("Debugging {}", metadata.binary_name));
                                ui.label(format!("{} functions", metadata.functions));
                                ui.label(format!("{} variables", metadata.vars));
                                metadata.files.iter().for_each(|file| {
                                    ui.label(file);
                                });
                            }
                            Err(message) => {
                                self.next_state = Some(State::UnrecoverableFailure {
                                    message: message.clone(),
                                });
                            }
                        },
                        None => {
                            ui.spinner();
                        }
                    };
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading(format!(
                        "Debugging {}",
                        match metadata.ready() {
                            Some(m) => match m {
                                Ok(m) => m.binary_name.clone(),
                                Err(_) => "Loading...".to_owned(),
                            },
                            None => "Loading...".to_owned(),
                        }
                    ));
                });
            }
            State::UnrecoverableFailure { message } => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Error");
                    ui.label(message.clone());
                });
            }
        }

        // egui::SidePanel::left("side_panel").show(ctx, |ui| {
        //     ui.heading("Side Panel");

        //     ui.horizontal(|ui| {
        //         ui.label("Write something: ");
        //         ui.text_edit_singleline(label);
        //     });

        //     ui.add(egui::Slider::new(value, 0.0..=10.0).text("value"));
        //     if ui.button("Increment").clicked() {
        //         *value += 1.0;
        //     }

        //     ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        //         ui.horizontal(|ui| {
        //             ui.spacing_mut().item_spacing.x = 0.0;
        //             ui.label("powered by ");
        //             ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        //             ui.label(" and ");
        //             ui.hyperlink_to(
        //                 "eframe",
        //                 "https://github.com/emilk/egui/tree/master/crates/eframe",
        //             );
        //             ui.label(".");
        //         });
        //     });
        // });
    }
}
