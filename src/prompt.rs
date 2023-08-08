use std::str::FromStr;

use dialoguer::{theme::ColorfulTheme, Completion, Input};
use serde::Deserialize;

use crate::debugger::error::DebugError;

#[derive(Deserialize)]
pub enum BreakpointPoint {
    Name(String),
    Address(u64),
}

#[derive(Deserialize)]
#[serde(tag = "Command")]
pub enum Command {
    Continue,
    Quit,
    GetRegister,
    StepInstruction,
    FindFunc(String),
    Read(u64),
    ProcessCounter,
    DebugMeta,
    DumpDwarf,
    Location,
    FindLine { line: u64, filename: String },
    StepOut,
    StepIn,
    ViewSource(usize),
    Backtrace,
    ReadVariables,
    SetBreakpoint(BreakpointPoint),
    Help(Vec<String>),
}

impl FromStr for Command {
    type Err = DebugError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.split(" ").into_iter();
        match iter
            .next()
            .ok_or(DebugError::InvalidCommand("empty command".to_string()))?
        {
            "location" => Ok(Command::Location),
            "continue" => Ok(Command::Continue),
            "quit" => Ok(Command::Quit),
            "get_registers" => Ok(Command::GetRegister),
            "step_instruction" => Ok(Command::StepInstruction),
            "pc" => Ok(Command::ProcessCounter),
            "dump_dwarf" => Ok(Command::DumpDwarf),
            "backtrace" => Ok(Command::Backtrace),
            "step_in" => Ok(Command::StepIn),
            "read_variables" => Ok(Command::ReadVariables),
            "debug_meta" => Ok(Command::DebugMeta),
            "read" => Ok(Command::Read(
                u64::from_str_radix(
                    iter.next()
                        .ok_or(DebugError::InvalidCommand(format!(
                            "read requires argument \"{}\"",
                            s
                        )))?
                        .trim_start_matches("0x"),
                    16,
                )
                .map_err(|a| DebugError::InvalidArgument(a.to_string()))?,
            )),
            "help" => Ok(Command::Help(CommandCompleter::default().commands)),
            "find_line" => Ok(Command::FindLine {
                line: iter
                    .next()
                    .ok_or(DebugError::InvalidCommand(format!(
                        "find_line requires 1st argument line \"{}\"",
                        s
                    )))?
                    .parse::<u64>()
                    .map_err(|a| DebugError::InvalidArgument(a.to_string()))?,
                filename: iter
                    .next()
                    .ok_or(DebugError::InvalidCommand(format!(
                        "find_line requires 2nd argument file \"{}\"",
                        s
                    )))?
                    .to_string(),
            }),
            "find_func" => Ok(Command::FindFunc(
                iter.next()
                    .ok_or(DebugError::InvalidCommand(format!(
                        "find_func requires argument \"{}\"",
                        s
                    )))?
                    .to_string(),
            )),
            "step_out" => Ok(Command::StepOut),
            "src" => Ok(Command::ViewSource(
                iter.next()
                    .ok_or(DebugError::InvalidCommand(format!(
                        "src requires argument \"{}\"",
                        s
                    )))?
                    .parse::<usize>()
                    .map_err(|a| DebugError::InvalidArgument(a.to_string()))?,
            )),
            "set_breakpoint" => Ok(Command::SetBreakpoint(
                match u64::from_str_radix(
                    iter.clone()
                        .next()
                        .ok_or(DebugError::InvalidCommand(format!(
                            "set_breakpoint requires argument \"{}\"",
                            s
                        )))?
                        .trim_start_matches("0x"),
                    16,
                ) {
                    Ok(a) => BreakpointPoint::Address(a),
                    Err(_) => BreakpointPoint::Name(
                        iter.next()
                            .ok_or(DebugError::InvalidCommand(format!(
                                "set_breakpoint requires argument \"{}\"",
                                s
                            )))?
                            .to_string(),
                    ),
                },
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
                "location".to_string(),
                "continue".to_string(),
                "quit".to_string(),
                "src".to_string(),
                "help".to_string(),
                "backtrace".to_string(),
                "debug_meta".to_string(),
                "read_variables".to_string(),
                "set_breakpoint".to_string(),
                "read".to_string(),
                "step_in".to_string(),
                "get_registers".to_string(),
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
