use iced::{
    widget::{container, text},
    Element, Length, Sandbox, Settings,
};

use crate::{debugger::DebugError, start_debuggee, DebuggerType};

use super::{
    debug_view::{DebugViewMessage, DebugViewState},
    file_picker::{FilePicker, FilePickerMessage},
};

enum View {
    Start(FilePicker),
    Debug(DebugViewState),
}

struct AppState {
    view: View,
}

#[derive(Debug, Clone)]
pub enum AppMessage {
    FilePickerEvent(FilePickerMessage),
    DebugViewEvent(DebugViewMessage),
}

fn load_binary(bin: String) -> Result<Option<DebuggerType>, DebugError> {
    start_debuggee(bin.into())
}

impl Sandbox for AppState {
    type Message = AppMessage;

    fn new() -> Self {
        Self {
            view: View::Start(FilePicker::new()),
        }
    }

    fn title(&self) -> String {
        String::from("Stackium")
    }

    fn update(&mut self, message: Self::Message) {
        match message {
            AppMessage::FilePickerEvent(e) => match &mut self.view {
                View::Start(f) => match e {
                    FilePickerMessage::Load(bin) => match load_binary(bin) {
                        Ok(d) => self.view = View::Debug(DebugViewState::new(d.unwrap())),
                        Err(e) => todo!(),
                    },
                    _ => match f.update(e).map(|f| AppMessage::FilePickerEvent(f)) {
                        Some(m) => self.update(m),
                        None => {}
                    },
                },
                _ => {}
            },
            AppMessage::DebugViewEvent(dm) => match &mut self.view {
                View::Debug(d) => d.update(dm),
                _ => {}
            },
        }
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        match &self.view {
            View::Start(file_picker) => file_picker.view().map(|f| AppMessage::FilePickerEvent(f)),
            View::Debug(d) => d.view().map(|d| AppMessage::DebugViewEvent(d)),
        }

        // container(text("Test"))
        //     .padding(20)
        //     .height(Length::Fill)
        //     .center_y()
        //     .into()
    }
}

pub fn run_gui() {
    AppState::run(Settings::default()).unwrap_or_else(|e| {
        println!("Failed creating window: {:?}", e);
    });
}
