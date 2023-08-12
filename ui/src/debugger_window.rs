use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, DebugMeta};
use url::Url;

pub struct DebuggerWindow {
    pub title: &'static str,
    pub is_active: bool,
    pub body: Box<dyn DebuggerWindowImpl>,
}

pub trait DebuggerWindowImpl {
    /// The bool in the return value indicates whether the
    /// widget changed the debug state significantly
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response);
    fn dirty(&mut self) {}
}

pub struct Metadata {
    metadata: Promise<Result<DebugMeta, String>>,
}

impl Metadata {
    pub fn new(backend_url: Url) -> Self {
        Self {
            metadata: { dispatch!(backend_url, Command::DebugMeta, DebugMeta) },
        }
    }
}

impl DebuggerWindowImpl for Metadata {
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
        match self.metadata.ready() {
            Some(metadata) => match metadata {
                Ok(metadata) => {
                    ui.heading(format!("Debugging {}", metadata.binary_name));
                    ui.label(format!("{} functions", metadata.functions));

                    metadata.files.iter().for_each(|file| {
                        ui.label(file);
                    });
                    (false, ui.label(format!("{} variables", metadata.vars)))
                }
                Err(message) => (false, ui.label("Error")),
            },
            None => (false, ui.spinner()),
        }
    }
}
