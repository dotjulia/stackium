use std::fmt::Display;

use nix::{
    sys::{
        ptrace,
        wait::{waitpid, WaitPidFlag},
    },
    unistd::Pid,
};

use crate::{
    breakpoint::Breakpoint,
    prompt::{command_prompt, Command},
};

#[derive(Debug)]
pub enum DebugError {
    NixError(nix::Error),
    IoError(std::io::Error),
    BreakpointInvalidState,
    InvalidCommand(String),
    InvalidArgument(String),
}

impl From<nix::Error> for DebugError {
    fn from(e: nix::Error) -> Self {
        DebugError::NixError(e)
    }
}

impl From<std::io::Error> for DebugError {
    fn from(e: std::io::Error) -> Self {
        DebugError::IoError(e)
    }
}

impl Display for DebugError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format!("{:?}", self).fmt(f)
    }
}

pub struct Debugger {
    pub child: Pid,
    breakpoints: Vec<Breakpoint>,
}

impl Debugger {
    fn new(child: Pid) -> Self {
        Debugger {
            child,
            breakpoints: Vec::new(),
        }
    }

    fn debug_loop(mut self) -> Result<(), DebugError> {
        loop {
            let input = command_prompt()?;
            match input {
                Command::Continue => continue_exec(self.child)?,
                Command::Quit => break,
                Command::SetBreakpoint(a) => {
                    println!("Setting breakpoint at address: {:?}", a);
                    let mut breakpoint = Breakpoint::new(self.child, a)?;
                    breakpoint.enable(self.child)?;
                    self.breakpoints.push(breakpoint);
                }
            }
        }
        Ok(())
    }

    fn waitpid(&self) -> Result<(), DebugError> {
        match waitpid(self.child, Some(WaitPidFlag::WUNTRACED)) {
            Ok(s) => match s {
                nix::sys::wait::WaitStatus::Exited(pid, status) => {
                    println!("Child {} exited with status: {}", pid, status);
                    Ok(())
                }
                nix::sys::wait::WaitStatus::Signaled(pid, status, coredump) => {
                    println!(
                        "Child {} signaled with status: {} and coredump: {}",
                        pid, status, coredump
                    );
                    Ok(())
                }
                nix::sys::wait::WaitStatus::Stopped(pid, signal) => {
                    println!("Child {} stopped with signal: {}", pid, signal);
                    Ok(())
                }
                nix::sys::wait::WaitStatus::Continued(pid) => {
                    println!("Child {} continued", pid);
                    Ok(())
                }
                nix::sys::wait::WaitStatus::StillAlive => todo!(),
            },
            Err(e) => Err(DebugError::NixError(e)),
        }
    }
}

fn continue_exec(pid: Pid) -> Result<(), DebugError> {
    ptrace::cont(pid, None).map_err(|e| DebugError::NixError(e))?;
    waitpid(pid, None)
        .map_err(|e| DebugError::NixError(e))
        .map(|_| ())
}

pub fn debugger_init(child: Pid) -> Result<(), DebugError> {
    println!("Child pid: {}", child);
    let debugger = Debugger::new(child);
    debugger.waitpid()?;
    debugger.debug_loop()?;
    Ok(())
}
