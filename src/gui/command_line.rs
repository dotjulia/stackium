use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    str::FromStr,
    sync::Arc,
};

use iced::{
    widget::{container, text, text_input},
    Element, Length,
};

use crate::{prompt::Command, DebuggerType};

pub struct CommandLineState {
    value: String,
    debugger: Arc<Cell<DebuggerType>>,
    last_output: String,
}

#[derive(Debug, Clone)]
pub enum CommandLineMessage {
    OnChange(String),
    Submit,
}

impl CommandLineState {
    pub fn new(debugger: Arc<Cell<DebuggerType>>) -> Self {
        Self {
            value: String::new(),
            last_output: String::new(),
            debugger,
        }
    }

    pub fn update(&mut self, message: CommandLineMessage) {
        match message {
            CommandLineMessage::OnChange(v) => {
                self.value = v;
            }
            CommandLineMessage::Submit => {
                if let Ok(command) = self.value.parse() {
                    let command: Command = command;
                    match self.debugger.get_mut().process_command(command) {
                        Ok(o) => self.last_output = format!("{:?}", o),
                        Err(_) => todo!(),
                    };
                }
            }
        }
    }

    pub fn view(&self) -> Element<'_, CommandLineMessage> {
        container(
            text_input("command", self.value.as_str())
                .on_input(CommandLineMessage::OnChange)
                .on_submit(CommandLineMessage::Submit),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}
