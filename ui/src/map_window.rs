use egui::RichText;
use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, MemoryMap};
use url::Url;

use crate::debugger_window::DebuggerWindowImpl;

pub struct MapWindow {
    mapping: Promise<Result<Vec<MemoryMap>, String>>,
    backend_url: Url,
}

impl MapWindow {
    pub fn new(backend_url: Url) -> Self {
        let mut ret = Self {
            mapping: Promise::from_ready(Err(String::new())),
            backend_url,
        };
        ret.dirty();
        ret
    }
}

impl DebuggerWindowImpl for MapWindow {
    fn dirty(&mut self) {
        self.mapping = dispatch!(self.backend_url.clone(), Command::Maps, Maps);
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.vertical(|ui| match self.mapping.ready() {
            Some(mapping) => match mapping {
                Ok(mapping) => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for i in (0..mapping.len()).rev() {
                            let map = &mapping[i];
                            let connected = if i > 0 {
                                mapping[i - 1].to == map.from
                            } else {
                                false
                            };
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.monospace(format!("{:#018x}", map.to));
                                    ui.monospace("...");
                                    if !connected {
                                        ui.monospace(format!("{:#018x}", map.from));
                                    }
                                });
                                let b = |a, s| if a { s } else { "-" };
                                ui.monospace(format!(
                                    "{}/{}/{}",
                                    b(map.read, "r"),
                                    b(map.write, "w"),
                                    b(map.execute, "x")
                                ));
                                ui.label(&map.mapped);
                            });
                            if !connected {
                                ui.separator();
                            }
                        }
                    });
                }
                Err(e) => {
                    ui.label(e);
                }
            },
            None => {
                ui.spinner();
            }
        });
        false
    }
}
