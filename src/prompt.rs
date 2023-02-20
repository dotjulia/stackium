use std::str::FromStr;

use dialoguer::{theme::ColorfulTheme, Completion, Input};

use crate::debugger::DebugError;

pub enum Command {
    Continue,
    Quit,
    SetBreakpoint(*const u8),
}

impl FromStr for Command {
    type Err = DebugError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.split(" ").into_iter();
        match iter
            .next()
            .ok_or(DebugError::InvalidCommand("empty command".to_string()))?
        {
            "continue" => Ok(Command::Continue),
            "quit" => Ok(Command::Quit),
            "set_breakpoint" => Ok(Command::SetBreakpoint(
                u64::from_str_radix(
                    iter.next()
                        .ok_or(DebugError::InvalidCommand(format!(
                            "set_breakpoint requires argument \"{}\"",
                            s
                        )))?
                        .trim_start_matches("0x"),
                    16,
                )
                .map_err(|a| DebugError::InvalidArgument(a.to_string()))?
                    as *const u8,
            )),
            _ => Err(DebugError::InvalidCommand("Unknown command".to_string())),
        }
    }
}

struct CommandCompleter {
    commands: Vec<String>,
}

impl Default for CommandCompleter {
    fn default() -> Self {
        CommandCompleter {
            commands: vec![
                "continue".to_string(),
                "quit".to_string(),
                "set_breakpoint".to_string(),
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
    Command::from_str(input).map(|_| ())
}

pub fn command_prompt() -> Result<Command, DebugError> {
    let completer = CommandCompleter::default();
    let input = Input::<String>::with_theme(&ColorfulTheme::default())
        .validate_with(command_validator)
        .completion_with(&completer)
        .with_prompt("dbg>")
        .interact_text()?;
    Command::from_str(&input)
}
