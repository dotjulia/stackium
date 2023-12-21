//! # Stackium
//! A debugger for students to learn pointer and memory layout more intuitively
//! ## Running the debugger
//! Usage: `stackium [OPTIONS] <PROGRAM>`
//! ```
//! Arguments: <PROGRAM> - the binary file to debug
//!
//! Options:
//! * -m, --mode <MODE> [default: cli] [possible values: cli, web]
//! * -h, --help        Print help
//! * -V, --version     Print version
//! ```
//! Launch with `-m web` to expose the API on port `8080`.
//! Have a look at the [crate::prompt::Command] struct for documentation on the API or
//! inspect the JSON Schema on `/schema` (or in the [schema.json](./schema.json)) or `/response_schema`.
use std::ffi::CStr;
use std::path::PathBuf;

use clap::Parser;
use debugger::error::DebugError;
use nix::sys::ptrace;
use nix::unistd::ForkResult::{Child, Parent};
use nix::unistd::{execv, fork, getcwd, Pid};
#[cfg(feature = "web")]
use web::start_webserver;

use crate::debugger::Debugger;

mod debugger;
mod prompt;
mod util;
mod variables;
#[cfg(feature = "web")]
mod web;

#[derive(Debug, Clone, clap::ValueEnum)]
enum DebugInterfaceMode {
    CLI,
    #[cfg(feature = "web")]
    Web,
    #[cfg(feature = "web")]
    Gui,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(index = 1)]
    program: PathBuf,
    #[clap(short, long, default_value = "cli")]
    mode: DebugInterfaceMode,
}

fn debuggee_init(prog: PathBuf) -> Result<(), DebugError> {
    match ptrace::traceme() {
        Ok(_) => (),
        Err(e) => {
            println!("Error in ptrace::traceme: {}", e);
            return Err(DebugError::NixError(e));
        }
    }

    // I think ASLR can't be disabled under macOS
    #[cfg(target_os = "linux")]
    nix::sys::personality::set(nix::sys::personality::Persona::ADDR_NO_RANDOMIZE)?;

    println!(
        "Child running in {:?}",
        getcwd().map_err(|e| DebugError::NixError(e))?
    );
    let path = format!("{}\0", prog.display());
    let path = CStr::from_bytes_with_nul(path.as_bytes()).unwrap();
    match execv(path, &[path]) {
        Ok(e) => {
            println!("Execv returned: {}", e);
            Ok(())
        }
        Err(e) => {
            println!("Error in execv: {}", nix::Error::from(e));
            Err(DebugError::NixError(e))
        }
    }
}

fn start_debuggee<'a>(prog: PathBuf) -> Result<Option<Debugger>, DebugError> {
    match unsafe { fork() } {
        Ok(fr) => match fr {
            Parent { child } => debugger_init(child, prog).map(|o| Some(o)),
            Child => debuggee_init(prog).map(|_| None),
        },
        Err(e) => Err(DebugError::NixError(e)),
    }
}

pub fn debugger_init<'a>(child: Pid, prog: PathBuf) -> Result<Debugger, DebugError> {
    println!("Child pid: {}", child);

    let debugger = Debugger::new(child, prog);
    debugger.waitpid()?;
    Ok(debugger)
}

fn main() -> Result<(), DebugError> {
    let args = Args::parse();
    let debugger = start_debuggee(args.program)?.unwrap();
    match args.mode {
        DebugInterfaceMode::CLI => debugger.debug_loop(),
        #[cfg(feature = "web")]
        DebugInterfaceMode::Web => start_webserver(debugger),
        #[cfg(feature = "web")]
        DebugInterfaceMode::Gui => match unsafe { fork() } {
            Ok(fr) => match fr {
                Parent { child: _ } => start_webserver(debugger),
                Child => {
                    match stackium_ui::start_ui() {
                        Ok(_) => {}
                        Err(e) => {
                            println!("{:?}", e);
                            panic!();
                        }
                    }
                    Ok(())
                }
            },
            Err(e) => Err(DebugError::NixError(e)),
        },
    }
}
