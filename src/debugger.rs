use std::{borrow, ffi::c_void, fmt::Display, fs, num::NonZeroU64, path::PathBuf};

use addr2line::gimli::{self, DebuggingInformationEntry};
use nix::{
    libc::user_regs_struct,
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
    FunctionNotFound,
    IoError(std::io::Error),
    GimliError(gimli::Error),
    ObjectError(addr2line::object::Error),
    BreakpointInvalidState,
    NoBreakpointFound,
    InvalidPC(u64),
    InvalidCommand(String),
    InvalidArgument(String),
}

impl From<addr2line::object::Error> for DebugError {
    fn from(e: addr2line::object::Error) -> Self {
        DebugError::ObjectError(e)
    }
}

impl From<gimli::Error> for DebugError {
    fn from(e: gimli::Error) -> Self {
        DebugError::GimliError(e)
    }
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

pub struct Debugger<R: gimli::Reader> {
    pub child: Pid,
    breakpoints: Vec<Breakpoint>,
    context: addr2line::Context<R>,
}

#[derive(Debug)]
struct FunctionMeta {
    name: Option<String>,
    low_pc: Option<u64>,
    high_pc: Option<u64>,
    return_addr: Option<u64>,
}

fn get_function_meta<R: gimli::Reader>(
    entry: &DebuggingInformationEntry<R, R::Offset>,
    dwarf: &gimli::Dwarf<R>,
) -> Result<FunctionMeta, DebugError> {
    let mut name: Option<String> = None;
    let mut attrs = entry.attrs();
    let mut low_pc = None;
    let mut high_pc = None;
    let mut return_addr = None;
    while let Some(attr) = attrs.next()? {
        match attr.name() {
            gimli::DW_AT_name => {
                if let gimli::AttributeValue::DebugStrRef(offset) = attr.value() {
                    if let Ok(s) = dwarf.debug_str.get_str(offset)?.to_string() {
                        name = Some(s.to_string());
                    }
                }
            }
            gimli::DW_AT_low_pc => {
                if let gimli::AttributeValue::Addr(addr) = attr.value() {
                    low_pc = Some(addr);
                }
            }
            gimli::DW_AT_high_pc => {
                if let gimli::AttributeValue::Udata(data) = attr.value() {
                    high_pc = Some(data);
                }
            }
            gimli::DW_AT_return_addr => {
                if let gimli::AttributeValue::Udata(addr) = attr.value() {
                    return_addr = Some(addr);
                }
            }
            _ => {}
        }
    }
    Ok(FunctionMeta {
        name,
        return_addr,
        low_pc,
        high_pc,
    })
}

impl<R: gimli::Reader> Debugger<R> {
    fn new(child: Pid, context: addr2line::Context<R>) -> Self {
        Debugger::<R> {
            child,
            context,
            breakpoints: Vec::new(),
        }
    }

    fn get_line_from_pc(&self, pc: u64) -> Result<addr2line::Location, DebugError> {
        Ok(self
            .context
            .find_location(pc)?
            .ok_or(DebugError::InvalidPC(pc))?)
    }

    fn get_addr_from_line(
        &self,
        line_to_find: u64,
        file_to_search: String,
    ) -> Result<u64, DebugError> {
        let dwarf = self.context.dwarf();
        let mut units = dwarf.units();
        while let Ok(Some(unit_header)) = units.next() {
            if let Ok(unit) = dwarf.unit(unit_header) {
                if let Some(line_program) = unit.line_program {
                    let mut rows = line_program.rows();
                    while let Ok(Some((header, row))) = rows.next_row() {
                        if let Some(file) = row.file(header) {
                            if let Some(filename) = file.path_name().string_value(&dwarf.debug_str)
                            {
                                if filename.to_string()? == file_to_search
                                    && row.line() == NonZeroU64::new(line_to_find)
                                {
                                    return Ok(row.address());
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(DebugError::FunctionNotFound)
    }

    fn get_func_from_addr(&self, addr: u64) -> Result<FunctionMeta, DebugError> {
        let dwarf = self.context.dwarf();
        let mut units = dwarf.units();
        while let Ok(Some(unit_header)) = units.next() {
            if let Ok(unit) = dwarf.unit(unit_header) {
                let mut entries = unit.entries();
                while let Ok(Some((_, entry))) = entries.next_dfs() {
                    if gimli::DW_TAG_subprogram == entry.tag() {
                        let func_meta = get_function_meta(&entry, &dwarf)?;
                        if let (Some(low_pc), Some(high_pc)) = (func_meta.low_pc, func_meta.high_pc)
                        {
                            if addr >= low_pc && addr <= low_pc + high_pc {
                                return Ok(func_meta);
                            }
                        }
                    }
                }
            }
        }
        Err(DebugError::FunctionNotFound)
    }

    fn backtrace(&self) -> Result<(), DebugError> {
        let print_meta = |func_meta: &FunctionMeta| {
            if let Some(name) = &func_meta.name {
                println!("{}()", name);
            }
        };
        let pc = self.get_pc()?;
        let mut func_meta = self.get_func_from_addr(pc)?;
        print_meta(&func_meta);
        let mut frame_pointer = self.get_registers()?.rbp;
        let mut return_addr = self.read((frame_pointer + 8) as *mut _)?;
        let mut max_depth = 20;
        while func_meta.name != Some("main".to_string()) {
            if --max_depth == 0 {
                break;
            }
            let func_meta_res = self.get_func_from_addr(return_addr);
            if func_meta_res.is_ok() {
                func_meta = func_meta_res.unwrap();
                print_meta(&func_meta);
                frame_pointer = self.read(frame_pointer as *mut _)?;
                return_addr = self.read((frame_pointer + 8) as *mut _)?;
            } else {
                println!("Unknown function");
            }
        }
        Ok(())
    }

    fn print_current_location(&self, window: usize) -> Result<(), DebugError> {
        let regs = self.get_registers().unwrap();
        let pc = regs.rip;
        let line = self.get_line_from_pc(pc)?;
        println!(
            "Current location: {}:{}",
            line.file.unwrap(),
            line.line.unwrap()
        );
        let file = fs::read_to_string(line.file.unwrap()).unwrap();
        for (index, line_str) in file.lines().enumerate() {
            if index as u32 >= line.line.unwrap() - window as u32
                && index as u32 <= line.line.unwrap() + window as u32
            {
                println!(
                    "{: >4}: {}{}",
                    index,
                    line_str,
                    if index as u32 == line.line.unwrap() {
                        " <---"
                    } else {
                        ""
                    }
                );
            }
        }
        Ok(())
    }

    fn debug_loop(mut self) -> Result<(), DebugError> {
        loop {
            let input = command_prompt()?;
            match input {
                Command::Help(commands) => {
                    println!("Available commands: ");
                    for command in commands {
                        println!("{}", command);
                    }
                }
                Command::Backtrace => self.backtrace()?,
                Command::Read(addr) => {
                    let val = self.read(addr as *mut _)?;
                    println!("RBP: {:#x}", self.get_registers()?.rbp);
                    println!("Value at {:#x}: {:#x}", addr, val);
                }
                Command::Continue => self.continue_exec()?,
                Command::Quit => break,
                Command::StepOut => self.step_out()?,
                Command::FindLine(line, file) => {
                    let addr = self.get_addr_from_line(line, file)?;
                    println!("Address: {:#x}", addr);
                }
                Command::FindFunc(name) => {
                    let func = self.find_function_from_name(name);
                    println!("{:?}", func);
                }
                Command::StepIn => self.step_in()?,
                Command::StepInstruction => self.step_instruction()?,
                Command::ProcessCounter => {
                    let regs = self.get_registers()?;
                    println!("Process counter: {:#x}", regs.rip);
                }
                Command::ViewSource(window) => match self.print_current_location(window) {
                    Ok(_) => {}
                    Err(e) => println!("Couldn't inspect current location: {:?}", e),
                },
                Command::GetRegister => {
                    let regs = self.get_registers()?;
                    println!("Registers: {:?}", regs);
                }
                Command::SetBreakpoint(a) => match a {
                    crate::prompt::BreakpointPoint::name(name) => {
                        let func = self.find_function_from_name(name)?;
                        if let Some(addr) = func.low_pc {
                            println!(
                                "Setting breakpoint at function: {:?} {:#x}",
                                func.name, addr,
                            );
                            let mut breakpoint = Breakpoint::new(self.child, addr as *const u8)?;
                            breakpoint.enable(self.child)?;
                            self.breakpoints.push(breakpoint);
                        } else {
                            println!("Couldn't find function: {:?}", func.name);
                        }
                    }
                    crate::prompt::BreakpointPoint::address(addr) => {
                        println!("Setting breakpoint at address: {:?}", addr);
                        let mut breakpoint = Breakpoint::new(self.child, addr)?;
                        breakpoint.enable(self.child)?;
                        self.breakpoints.push(breakpoint);
                    }
                },
            }
        }
        Ok(())
    }

    fn write(&self, addr: *mut c_void, data: u64) -> Result<(), DebugError> {
        match unsafe { ptrace::write(self.child, addr, data as *mut _) } {
            Ok(_) => Ok(()),
            Err(e) => Err(DebugError::NixError(e)),
        }
    }

    fn read(&self, addr: *mut c_void) -> Result<u64, DebugError> {
        match ptrace::read(self.child, addr) {
            Ok(d) => Ok(d as u64),
            Err(e) => Err(DebugError::NixError(e)),
        }
    }

    fn get_pc(&self) -> Result<u64, DebugError> {
        let regs = self.get_registers()?;
        Ok(regs.rip)
    }

    fn set_pc(&self, pc: u64) -> Result<(), DebugError> {
        let mut regs = self.get_registers()?;
        regs.rip = pc;
        self.set_registers(regs)
    }

    fn step_instruction(&mut self) -> Result<(), DebugError> {
        let pc = self.get_pc()?;
        if self.breakpoints.iter().any(|b| b.address as u64 == pc) {
            self.step_breakpoint()?;
        } else {
            ptrace::step(self.child, None)?;
            self.waitpid()?;
        }
        Ok(())
    }

    fn step_out(&mut self) -> Result<(), DebugError> {
        let fp = self.get_registers()?.rbp;
        let ra = self.read((fp + 8) as *mut c_void)?;
        let bp: Vec<_> = self
            .breakpoints
            .iter()
            .enumerate()
            .filter(|(i, b)| b.address as u64 == ra)
            .map(|(i, b)| i)
            .collect();
        if bp.len() == 0 {
            let mut breakpoint = Breakpoint::new(self.child, ra as *const u8)?;
            breakpoint.enable(self.child)?;
            self.continue_exec()?;
            breakpoint.disable(self.child)?;
            Ok(())
        } else if bp.len() == 1 {
            let index = bp[0];
            self.breakpoints[index].enable(self.child)?;
            self.continue_exec()?;
            self.breakpoints[index].disable(self.child)?;
            Ok(())
        } else {
            Err(DebugError::BreakpointInvalidState)
        }
    }

    fn find_function_from_name(&self, name_to_find: String) -> Result<FunctionMeta, DebugError> {
        let mut units = self.context.dwarf().units();
        while let Some(unit_header) = units.next()? {
            let unit = self.context.dwarf().unit(unit_header)?;
            let mut cursor = unit.entries();
            while let Some((_, entry)) = cursor.next_dfs()? {
                if entry.tag() != gimli::DW_TAG_subprogram {
                    continue;
                }
                if let Ok(Some(name)) = entry.attr(gimli::DW_AT_name) {
                    if let Some(name) = name.string_value(&self.context.dwarf().debug_str) {
                        if let Ok(name) = name.to_string() {
                            if name == name_to_find {
                                return get_function_meta(entry, self.context.dwarf());
                            }
                        }
                    }
                }
            }
        }
        Err(DebugError::FunctionNotFound)
    }

    fn step_in(&mut self) -> Result<(), DebugError> {
        let line = match self.get_line_from_pc(self.get_pc()?) {
            Ok(line) => line.line,
            Err(_) => None,
        };
        while match self.get_line_from_pc(self.get_pc()?) {
            Ok(line) => line.line,
            Err(_) => None,
        } == line
        {
            self.step_instruction()?;
        }
        Ok(())
    }

    fn step_breakpoint(&mut self) -> Result<(), DebugError> {
        let pc = self.get_pc()?;
        let breakpoint_indices: Vec<_> = self
            .breakpoints
            .iter()
            .enumerate()
            .filter(|(i, b)| b.address as u64 == pc)
            .map(|(i, _)| i)
            .collect();
        if breakpoint_indices.len() == 1 {
            let index = breakpoint_indices[0];
            self.set_pc(pc)?;
            self.breakpoints[index].disable(self.child)?;
            ptrace::step(self.child, None)?;
            self.waitpid()?;
            self.breakpoints[index].enable(self.child)?;
            Ok(())
        } else if breakpoint_indices.len() == 0 {
            Err(DebugError::NoBreakpointFound)
        } else {
            Err(DebugError::BreakpointInvalidState)
        }
    }

    fn get_registers(&self) -> Result<user_regs_struct, DebugError> {
        match ptrace::getregs(self.child) {
            Ok(r) => Ok(r),
            Err(e) => Err(DebugError::NixError(e)),
        }
    }

    fn set_registers(&self, reg: user_regs_struct) -> Result<(), DebugError> {
        match ptrace::setregs(self.child, reg) {
            Ok(_) => Ok(()),
            Err(e) => Err(DebugError::NixError(e)),
        }
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
                    match signal {
                        nix::sys::signal::Signal::SIGTRAP => {
                            let siginfo = nix::sys::ptrace::getsiginfo(pid)?;
                            // I think nix doesn't have a constant for this
                            if siginfo.si_code == 128 {
                                println!("Hit breakpoint!");

                                // step back one instruction
                                self.set_pc(self.get_pc()? - 1)?;
                            } else {
                                println!(
                                    "Child {} stopped with SIGTRAP and code {}",
                                    pid, siginfo.si_code
                                );
                            }
                        }
                        _ => {
                            println!("Child {} stopped with signal: {}", pid, signal);
                        }
                    }
                    Ok(())
                }
                nix::sys::wait::WaitStatus::Continued(pid) => {
                    println!("Child {} continued", pid);
                    Ok(())
                }
                #[cfg(target_os = "linux")]
                nix::sys::wait::WaitStatus::StillAlive => {
                    println!("Child is still alive");
                    Ok(())
                }
                #[cfg(target_os = "linux")]
                nix::sys::wait::WaitStatus::PtraceEvent(pid, signal, int) => {
                    println!(
                        "Child {} ptrace event with signal: {} and int: {}",
                        pid, signal, int
                    );
                    Ok(())
                }
                #[cfg(target_os = "linux")]
                nix::sys::wait::WaitStatus::PtraceSyscall(pid) => {
                    println!("Child {} ptrace syscall", pid);
                    Ok(())
                }
            },
            Err(e) => Err(DebugError::NixError(e)),
        }
    }

    fn continue_exec(&mut self) -> Result<(), DebugError> {
        match self.step_breakpoint() {
            Ok(_) => (),
            Err(DebugError::NoBreakpointFound) => {
                println!("Warning: continuing execution from non-breakpoint");
            }
            Err(e) => return Err(e),
        }
        ptrace::cont(self.child, None).map_err(|e| DebugError::NixError(e))?;
        self.waitpid()
    }
}

pub fn debugger_init(child: Pid, prog: PathBuf) -> Result<(), DebugError> {
    println!("Child pid: {}", child);

    let bin = &fs::read(prog)?[..];
    let object_file = addr2line::object::read::File::parse(bin)?;
    let context = addr2line::Context::new(&object_file)?;

    let debugger = Debugger::new(child, context);
    debugger.waitpid()?;
    debugger.debug_loop()?;
    Ok(())
}
