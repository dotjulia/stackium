use std::ffi::CStr;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use addr2line::gimli::{EndianReader, RunTimeEndian};
use clap::Parser;
use debugger::DebugError;
use crate::gui::run::run_gui;
use nix::sys::ptrace;
use nix::unistd::ForkResult::{Child, Parent};
use nix::unistd::{execv, fork, getcwd, Pid};
#[cfg(feature = "web")]
use web::serve_web;

use crate::debugger::Debugger;

mod breakpoint;
mod debugger;
mod prompt;
mod gui;
#[cfg(feature = "web")]
mod web;

#[derive(Debug, Clone, clap::ValueEnum)]
enum DebugInterfaceMode {
    CLI,
    #[cfg(feature = "web")]
    Web,
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

type DebuggerType = Debugger<EndianReader<RunTimeEndian, Rc<[u8]>>>;

fn start_debuggee(prog: PathBuf) -> Result<Option<DebuggerType>, DebugError> {
    match unsafe { fork() } {
        Ok(fr) => match fr {
            Parent { child } => debugger_init(child, prog).map(|o| Some(o)),
            Child => debuggee_init(prog).map(|_| None),
        },
        Err(e) => Err(DebugError::NixError(e)),
    }
}

pub fn debugger_init(child: Pid, prog: PathBuf) -> Result<DebuggerType, DebugError> {
    println!("Child pid: {}", child);

    let bin = &fs::read(prog)?[..];
    let object_file = addr2line::object::read::File::parse(bin)?;
    let context = addr2line::Context::new(&object_file)?;

    let debugger = Debugger::new(child, context);
    debugger.waitpid()?;
    Ok(debugger)
}

fn main() -> Result<(), DebugError> {
    #[cfg(feature = "gui")]
    {
        run_gui();
        return Ok(());
    }

    let args = Args::parse();
    let debugger = start_debuggee(args.program)?.unwrap();
    match args.mode {
        DebugInterfaceMode::CLI => debugger.debug_loop(),
        #[cfg(feature = "web")]
        DebugInterfaceMode::Web => {
            serve_web(debugger);
            Ok(())
        }
    }
}
