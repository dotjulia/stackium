use std::str::FromStr;

use dialoguer::{theme::ColorfulTheme, Completion, Input};
use stackium_shared::Command;

use crate::debugger::error::DebugError;

pub struct CommandCompleter {
    pub commands: Vec<String>,
}

impl Default for CommandCompleter {
    fn default() -> Self {
        CommandCompleter {
            commands: vec![
                "get_functions".to_string(),
                "location".to_string(),
                "continue".to_string(),
                "delete_breakpoint".to_string(),
                "disassemble".to_string(),
                "quit".to_string(),
                "src".to_string(),
                "get_breakpoints".to_string(),
                "help".to_string(),
                "backtrace".to_string(),
                "debug_meta".to_string(),
                "read_variables".to_string(),
                "set_breakpoint".to_string(),
                "read".to_string(),
                "step_in".to_string(),
                "get_registers".to_string(),
                "waitpid".to_string(),
                "find_func".to_string(),
                "find_line".to_string(),
                "pc".to_string(),
                "step_out".to_string(),
                "step_instruction".to_string(),
                "dump_dwarf".to_string(),
            ],
        }
    }
}

impl Completion for CommandCompleter {
    fn get(&self, input: &str) -> Option<String> {
        let mut matches = self
            .commands
            .iter()
            .filter(|c| c.starts_with(input))
            .collect::<Vec<_>>();
        if matches.len() >= 1 {
            Some(matches.pop().unwrap().clone())
        } else {
            None
        }
    }
}

fn command_validator(input: &String) -> Result<(), DebugError> {
    Command::from_str(input)
        .map(|_| ())
        .map_err(|e| DebugError::InvalidCommand(e))
}

pub fn command_prompt() -> Result<Command, DebugError> {
    let completer = CommandCompleter::default();
    let input = Input::<String>::with_theme(&ColorfulTheme::default())
        .validate_with(command_validator)
        .completion_with(&completer)
        .with_prompt("dbg>")
        .interact_text()?;
    Command::from_str(&input).map_err(|e| DebugError::InvalidCommand(e))
}
