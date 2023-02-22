use std::ffi::CStr;
use std::path::PathBuf;

use clap::Parser;
use debugger::{debugger_init, DebugError};
use nix::sys::ptrace;
use nix::unistd::ForkResult::{Child, Parent};
use nix::unistd::{execv, fork, getcwd};

mod breakpoint;
mod debugger;
mod prompt;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(index = 1)]
    program: PathBuf,
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

fn start_debuggee(prog: PathBuf) -> Result<(), DebugError> {
    match unsafe { fork() } {
        Ok(fr) => match fr {
            Parent { child } => debugger_init(child, prog),
            Child => debuggee_init(prog),
        },
        Err(e) => Err(DebugError::NixError(e)),
    }
}

fn main() -> Result<(), DebugError> {
    let args = Args::parse();
    start_debuggee(args.program)
}
