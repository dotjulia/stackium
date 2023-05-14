use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use iced::{
    widget::{text, Column},
    Element,
};

use crate::{prompt::Command, DebuggerType};

use super::command_line::{CommandLineMessage, CommandLineState};

#[derive(Debug, Clone)]
pub enum DebugViewMessage {
    CommandLine(CommandLineMessage),
}

pub struct DebugViewState {
    command_line: CommandLineState,
    debugger: Arc<Cell<DebuggerType>>,
}

impl DebugViewState {
    pub fn update(&mut self, message: DebugViewMessage) {
        match message {
            DebugViewMessage::CommandLine(s) => self.command_line.update(s),
        };
    }

    pub fn new(debugger: DebuggerType) -> Self {
        let d = Arc::from(Cell::from(debugger));
        Self {
            debugger: d.clone(),
            command_line: CommandLineState::new(d),
        }
    }

    pub fn view(&self) -> Element<'_, DebugViewMessage> {
        Column::with_children(vec![
            text(self.debugger.get_mut().child).into(),
            self.command_line
                .view()
                .map(|f| DebugViewMessage::CommandLine(f))
                .into(),
        ])
        .into()
    }
}
