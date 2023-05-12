use dioxus::prelude::*;

use crate::{
    debugger::DebugError, gui::{file_picker::load_binary_view, debugging::debugging_view}, start_debuggee, DebuggerType,
};

fn load_binary(bin: String) -> Result<Option<DebuggerType>, DebugError> {
    start_debuggee(bin.into())
}

enum AppState {
    LoadBinary,
    Debugging(DebuggerType),
    Error(String),
}

fn app_start<'a>(cx: Scope<'a>) -> Element<'a> {
    let state = use_state(cx, || AppState::LoadBinary);
    match state.get() {
        AppState::LoadBinary => cx.render(rsx! {
            load_binary_view {
                  load: cx.event_handler(|e| {
                    let retv = load_binary(e);
                    match retv {
                        Ok(debugger) => match debugger {
                            Some(debugger) => state.set(AppState::Debugging(debugger)),
                            None => state.set(AppState::Error("unknown error".to_owned())),
                        },
                        Err(err) => state.set(AppState::Error(err.to_string())),
                    }
                }),
            }
        }),
        AppState::Error(err) => cx.render(rsx! {
            div {
                style: "width: 100%; height: 100%; position: absolute; top: 0; left: 0; padding: 0; margin: 0; display: flex; justify-content: center; align-items: center; overflow: hidden;",
                "{err}"
                br {}
                button {
                    onclick: |_| state.set(AppState::LoadBinary),
                    "Try again",
                }
            }
        }),
        AppState::Debugging(_) => cx.render(rsx! {
            debugging_view {
                
            }
        }),
    }
}

pub fn run_gui() {
    dioxus_desktop::launch(app_start);
}
