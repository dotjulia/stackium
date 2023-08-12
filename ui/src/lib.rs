#![warn(clippy::all, rust_2018_idioms)]

mod app;
#[macro_use]
mod command;
mod breakpoint_window;
mod code_window;
mod debugger_window;
mod location;
mod syntax_highlighting;
mod toggle;
pub use app::StackiumApp;
