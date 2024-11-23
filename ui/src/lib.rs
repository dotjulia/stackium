#![warn(clippy::all, rust_2018_idioms)]

mod app;
#[macro_use]
mod command;
mod breakpoint_window;
mod code_window;
mod control_window;
mod debugger_window;
mod frame_history;
mod graph_window;
mod location;
mod map_window;
mod memory_window;
mod register_window;
mod settings_window;
mod syntax_highlighting;
mod toggle;
mod variable_window;
pub use app::StackiumApp;
mod rotated_plot_text;

trait LimitStringLen {
    fn limit_string_len(&self, len: usize) -> Self;
}

impl LimitStringLen for String {
    fn limit_string_len(&self, len: usize) -> Self {
        if self.len() > len {
            format!("{}..", &self[..len])
        } else {
            self.clone()
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn start_ui() -> eframe::Result<()> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "eframe template",
        native_options,
        Box::new(|cc| Ok(Box::new(crate::StackiumApp::new(cc)))),
    )
}
