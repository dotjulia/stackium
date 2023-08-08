use gimli::{EvaluationResult, Reader};
use nix::{
    libc::user_regs_struct,
    sys::{
        ptrace,
        wait::{waitpid, WaitPidFlag},
    },
    unistd::Pid,
};
use object::{Object, ObjectSection};
use serde::Serialize;
use std::{ffi::c_void, fs, path::PathBuf, sync::Arc};

pub mod breakpoint;
pub mod error;
mod util;

use crate::{
    debugger::util::{get_function_meta, get_piece_addr},
    prompt::{command_prompt, Command},
    util::{dw_at_to_string, tag_to_string, Registers},
};

use self::{
    breakpoint::Breakpoint,
    error::DebugError,
    util::{find_function_from_name, get_addr_from_line, get_line_from_pc, FunctionMeta},
};

pub struct Debugger {
    pub child: Pid,
    breakpoints: Vec<Breakpoint>,
    pub program: PathBuf,
    dwarf: gimli::read::Dwarf<gimli::read::EndianReader<gimli::NativeEndian, Arc<[u8]>>>,
}

macro_rules! iter_every_entry {
    ($self:ident, $entry:ident $unit:ident | $body:block) => {
        let dwarf = &$self.dwarf;
        let mut units = dwarf.units();
        while let Ok(Some(unit_header)) = units.next() {
            let unit_opt = dwarf.unit(unit_header);
            if unit_opt.is_ok() {
                $unit = unit_opt.unwrap();
                let mut entries = $unit.entries();
                let mut entry_res = entries.next_dfs();
                while entry_res.is_ok() && entry_res.unwrap().is_some() {
                    $entry = entry_res.unwrap().unwrap().1;
                    $body
                    entry_res = entries.next_dfs();
                }
            }
        }
    };
}

macro_rules! find_entry_with_offset {
    ($offset:ident, $self:ident, $entry:ident $unit:ident | $body:block) => {
        iter_every_entry!(
            $self,
            $entry $unit | {
                if $entry.offset() == $offset {
                    $body
                }
            })
    };
}
#[derive(Debug, Serialize)]
enum TypeName {
    Name(String),
    Ref(Box<TypeName>),
}

#[derive(Debug, Default, Serialize)]
pub struct Variable {
    name: Option<String>,
    type_name: Option<TypeName>,
    value: Option<u64>,
    file: Option<String>,
    line: Option<u64>,
    addr: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct DwarfAttribute {
    name: String,
    addr: u64,
    tag: String,
    attrs: Vec<String>,
}

#[derive(Debug, Serialize)]
pub enum CommandOutput {
    Data(u64),
    Variables(Vec<Variable>),
    FunctionMeta(FunctionMeta),
    CodeWindow(Vec<(u64, String, bool)>),
    Registers(Registers),
    DebugMeta(DebugMeta),
    Location(Location),
    DwarfAttributes(Vec<DwarfAttribute>),
    Help(Vec<String>),
    Backtrace(Vec<FunctionMeta>),
    None,
}

#[derive(Debug, Serialize)]
pub struct DebugMeta {
    file_type: String,
    files: Vec<String>,
    functions: i32,
    vars: i32,
}

#[derive(Debug, Serialize)]
pub struct Location {
    line: u64,
    file: String,
    column: u64,
}

impl Debugger {
    pub fn new(child: Pid, object_file: PathBuf) -> Self {
        let load_section = |id: gimli::SectionId| -> Result<Arc<Vec<u8>>, gimli::Error> {
            let bin = fs::read(object_file.clone()).unwrap();
            let object_file = object::File::parse(&bin[..]).unwrap();
            match object_file.section_by_name(id.name()) {
                Some(section) => Ok(Arc::new(
                    section.uncompressed_data().unwrap().to_mut().clone(),
                )),
                None => Ok(Arc::new(vec![])),
            }
        };
        let dwarf_cow = gimli::Dwarf::load(&load_section).unwrap();
        let dwarf = dwarf_cow.borrow(|section| {
            gimli::EndianArcSlice::new(Arc::from(&section[..]), gimli::NativeEndian)
        });
        Debugger {
            child,
            program: object_file,
            breakpoints: Vec::new(),
            dwarf,
        }
    }

    fn dump_dwarf_attrs(&self) -> Result<Vec<DwarfAttribute>, DebugError> {
        let mut sub_entry;
        let mut unit;
        let mut output = Vec::<DwarfAttribute>::new();
        iter_every_entry!(self, sub_entry unit | {
            let mut attrs_vec = Vec::<String>::new();
            let mut attrs = sub_entry.attrs();
            while let Some(attr) = attrs.next()? {
                attrs_vec.push(format!("{}: {}", dw_at_to_string(attr.name()), match attr.string_value(&self.dwarf.debug_str) {
                    Some(s) => s.to_string().unwrap().to_string(),
                    None => match attr.udata_value() {
                        Some(u) => u.to_string(),
                        None => "??".to_owned(),
                    }
                }));
            }
            output.push(DwarfAttribute { name: unit.name.clone().unwrap().to_string().unwrap().to_string(), addr: sub_entry.offset().0 as u64, tag: tag_to_string(sub_entry.tag()), attrs: attrs_vec })
        });
        Ok(output)
    }

    fn decode_type<T: gimli::Reader<Offset = usize>>(
        &self,
        offset: gimli::AttributeValue<T>,
    ) -> Result<TypeName, DebugError> {
        if let gimli::AttributeValue::UnitRef(r) = offset {
            let mut offset_entry;
            let mut offset_unit;
            find_entry_with_offset!(r, self, offset_entry offset_unit | {
               if let Some(name) = offset_entry.attr_value(gimli::DW_AT_name)? {
                return Ok(TypeName::Name(name.string_value(&self.dwarf.debug_str).ok_or(DebugError::InvalidType)?.to_string().unwrap().to_string()));
               } else {
                return Ok(TypeName::Ref(Box::new(self.decode_type(offset_entry.attr_value(gimli::DW_AT_type)?.unwrap())?)));
               }
            });
            Err(DebugError::InvalidType)
        } else {
            Err(DebugError::InvalidType)
        }
    }

    fn get_register_from_abi(&self, reg: u16) -> Result<u64, DebugError> {
        let registers = self.get_registers()?;
        match reg {
            0 => Ok(registers.rax),
            1 => Ok(registers.rdx),
            2 => Ok(registers.rcx),
            3 => Ok(registers.rbx),
            4 => Ok(registers.rsi),
            5 => Ok(registers.rdi),
            6 => Ok(registers.rbp),
            7 => Ok(registers.rsp),
            8 => Ok(registers.r8),
            9 => Ok(registers.r9),
            10 => Ok(registers.r10),
            11 => Ok(registers.r11),
            12 => Ok(registers.r12),
            13 => Ok(registers.r13),
            14 => Ok(registers.r14),
            15 => Ok(registers.r15),
            16 => Ok(registers.rip),
            17 => Ok(registers.eflags),
            18 => Ok(registers.cs),
            19 => Ok(registers.ss),
            20 => Ok(registers.ds),
            21 => Ok(registers.es),
            22 => Ok(registers.fs),
            23 => Ok(registers.gs),
            _ => Err(DebugError::InvalidRegister),
        }
    }

    fn retrieve_pieces<T: gimli::Reader>(
        &self,
        pieces: Vec<gimli::Piece<T>>,
    ) -> Result<u64, DebugError> {
        let mut value = 0;
        for piece in pieces {
            value = value
                + match piece.location {
                    gimli::Location::Empty => todo!(),
                    gimli::Location::Register { register: _ } => todo!(),
                    gimli::Location::Address { address } => self.read(address as *mut _)?,
                    gimli::Location::Value { value: _ } => todo!(),
                    gimli::Location::Bytes { value: _ } => todo!(),
                    gimli::Location::ImplicitPointer {
                        value: _,
                        byte_offset: _,
                    } => todo!(),
                }
        }
        Ok(value)
    }

    fn read_variables(&self) -> Result<Vec<Variable>, DebugError> {
        let mut sub_entry;
        let mut unit;
        let mut variables = Vec::new();
        iter_every_entry!(self, sub_entry unit | {
            if sub_entry.tag() == gimli::DW_TAG_variable {
                let mut var = Variable::default();
                if let Some(location) = sub_entry.attr_value(gimli::DW_AT_location)? {
                    let location = location.exprloc_value().unwrap();
                    let mut evaluation = location.evaluation(unit.encoding());
                    let mut result = evaluation.evaluate().unwrap();
                    while result != EvaluationResult::Complete {
                        match result {
                            EvaluationResult::Complete => panic!(),
                            EvaluationResult::RequiresMemory { address: _, size: _, space: _, base_type: _ } => todo!(),
                            EvaluationResult::RequiresRegister { register, base_type: _ } => {
                                let value = self.get_register_from_abi(register.0)?;
                                result = evaluation.resume_with_register(gimli::Value::U64(value))?;
                            },
                            EvaluationResult::RequiresFrameBase => {
                                let base_pointer = self.get_registers()?.rbp;
                                result = evaluation.resume_with_frame_base(base_pointer)?;

                            },
                            EvaluationResult::RequiresTls(_) => todo!(),
                            EvaluationResult::RequiresCallFrameCfa => todo!(),
                            EvaluationResult::RequiresAtLocation(_) => todo!(),
                            EvaluationResult::RequiresEntryValue(_) => todo!(),
                            EvaluationResult::RequiresParameterRef(_) => todo!(),
                            EvaluationResult::RequiresRelocatedAddress(_) => todo!(),
                            EvaluationResult::RequiresIndexedAddress { index, relocate: _ } => {
                                let addr = self.dwarf.debug_addr.get_address(unit.header.address_size(), unit.addr_base, index)?;
                                result = evaluation.resume_with_indexed_address(addr)?;

                            },
                            EvaluationResult::RequiresBaseType(_) => todo!(),
                        }
                    }
                    let pieces = evaluation.result();
                    var.addr = get_piece_addr(&pieces[0]);
                    var.value = self.retrieve_pieces(pieces).ok();
                }
                var.type_name = self.decode_type(sub_entry.attr(gimli::DW_AT_type)?.unwrap().value()).ok();

                if let Some(name) = sub_entry.attr(gimli::DW_AT_name)? {
                    if let Some(name) = name.string_value(&self.dwarf.debug_str) {
                        let name = name.to_string()?;
                        var.name = Some(name.to_string());
                    }
                }
                if let Some(file) = sub_entry.attr(gimli::DW_AT_decl_file)? {
                    if let Some(file) = file.string_value(&self.dwarf.debug_str) {
                        var.file = file.to_string().ok().map(|s| s.to_string());
                    }
                }
                if let Some(line) = sub_entry.attr(gimli::DW_AT_decl_line)? {
                    if let Some(line) = line.udata_value() {
                        var.line = Some(line as u64);
                    }
                }
                variables.push(var);
            }
        });
        Ok(variables)
    }

    fn get_func_from_addr(&self, addr: u64) -> Result<FunctionMeta, DebugError> {
        let mut meta;
        let mut entry;
        let mut unit;
        iter_every_entry!(
            self,
            entry unit | {
                if entry.tag() == gimli::DW_TAG_subprogram {
                    meta = get_function_meta(&entry, &self.dwarf)?;
                    if let (Some(low_pc), Some(high_pc)) = (meta.low_pc, meta.high_pc) {
                        if addr >= low_pc && addr <= low_pc + high_pc {
                            return Ok(meta);
                        }
                    }
                }
            }
        );
        Err(DebugError::FunctionNotFound)
    }

    fn backtrace(&self) -> Result<Vec<FunctionMeta>, DebugError> {
        let mut bt = Vec::<FunctionMeta>::new();
        let pc = self.get_pc()?;
        let mut func_meta = self.get_func_from_addr(pc)?;
        bt.push(func_meta.clone());
        let mut frame_pointer = self.get_registers()?.rbp;
        let mut return_addr = self.read((frame_pointer + 8) as *mut _)?;
        let mut max_depth = 20;
        while func_meta.name != Some("main".to_string()) {
            max_depth -= 1;
            if max_depth == 0 {
                break;
            }
            let func_meta_res = self.get_func_from_addr(return_addr);
            if func_meta_res.is_ok() {
                func_meta = func_meta_res.unwrap();
                bt.push(func_meta.clone());
                frame_pointer = self.read(frame_pointer as *mut _)?;
                return_addr = self.read((frame_pointer + 8) as *mut _)?;
            } else {
                bt.push(FunctionMeta {
                    name: None,
                    low_pc: None,
                    high_pc: None,
                    return_addr: None,
                });
            }
        }
        Ok(bt)
    }

    fn print_current_location(
        &self,
        window: usize,
    ) -> Result<Vec<(u64, String, bool)>, DebugError> {
        let regs = self.get_registers().unwrap();
        let pc = regs.rip;
        let line = get_line_from_pc(&self.dwarf, pc)?;
        let mut lines = Vec::new();
        let file = fs::read_to_string(line.file).unwrap();
        for (index, line_str) in file.lines().enumerate() {
            if index as u64 >= line.line - window as u64
                && index as u64 <= line.line + window as u64
            {
                lines.push((
                    index as u64,
                    line_str.to_string(),
                    index as u64 == line.line,
                ));
            }
        }
        Ok(lines)
    }

    fn debug_meta(&self) -> Result<DebugMeta, DebugError> {
        let mut entry;
        let mut unit;
        let mut vars = 0;
        let mut functions = 0;
        let mut files = Vec::new();
        iter_every_entry!(self, entry unit | {
            if entry.tag() == gimli::DW_TAG_variable {
                vars += 1;
            } else if entry.tag() == gimli::DW_TAG_subprogram {
                functions += 1;
            }
            let name = unit.name.clone();
            if let Some(name) = name {
                if let Ok(name) = name.to_string() {
                    let name = name.to_string();
                    if !files.contains(&name) {
                        files.push(name);
                    }
                }
            }
        });
        Ok(DebugMeta {
            file_type: format!("{:?}", self.dwarf.file_type),
            functions,
            vars,
            files,
        })
    }

    pub fn process_command(&mut self, command: Command) -> Result<CommandOutput, DebugError> {
        match command {
            Command::DebugMeta => Ok(CommandOutput::DebugMeta(self.debug_meta()?)),
            Command::DumpDwarf => Ok(CommandOutput::DwarfAttributes(self.dump_dwarf_attrs()?)),
            Command::Help(commands) => Ok(CommandOutput::Help(commands)),
            Command::Backtrace => Ok(CommandOutput::Backtrace(self.backtrace()?)),
            Command::ReadVariables => Ok(CommandOutput::Variables(self.read_variables()?)),
            Command::Read(addr) => Ok(CommandOutput::Data(self.read(addr as *mut _)?)),
            Command::Continue => {
                self.continue_exec()?;
                Ok(CommandOutput::None)
            }
            Command::Quit => std::process::exit(0),
            Command::StepOut => self.step_out().map(|_| CommandOutput::None),
            Command::FindLine{ line, filename } => {
                let addr = get_addr_from_line(&self.dwarf, line, filename)?;
                Ok(CommandOutput::Data(addr))
            }
            Command::FindFunc(name) => {
                let func = find_function_from_name(&self.dwarf, name);
                Ok(CommandOutput::FunctionMeta(func?))
            }
            Command::StepIn => self.step_in().map(|_| CommandOutput::None),
            Command::StepInstruction => self.step_instruction().map(|_| CommandOutput::None),
            Command::ProcessCounter => {
                let regs = self.get_registers()?;
                Ok(CommandOutput::Data(regs.rip))
            }
            Command::SetBreakpoint(a) => match a {
                crate::prompt::BreakpointPoint::Name(name) => {
                    let func = find_function_from_name(&self.dwarf, name)?;
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
                    Ok(CommandOutput::None)
                }
                crate::prompt::BreakpointPoint::Address(addr) => {
                    println!("Setting breakpoint at address: {:?}", addr);
                    let mut breakpoint = Breakpoint::new(self.child, addr as *const u8)?;
                    breakpoint.enable(self.child)?;
                    self.breakpoints.push(breakpoint);
                    Ok(CommandOutput::None)
                }
            },
            Command::ViewSource(window) => self
                .print_current_location(window)
                .map(|l| CommandOutput::CodeWindow(l)),
            Command::GetRegister => {
                let regs = self.get_registers()?;
                Ok(CommandOutput::Registers(regs.into()))
            }
            Command::Location => Ok(CommandOutput::Location(get_line_from_pc(
                &self.dwarf,
                self.get_pc()?,
            )?)),
        }
    }

    pub fn debug_loop(mut self) -> Result<(), DebugError> {
        loop {
            let input = command_prompt()?;
            println!("{:#?}", self.process_command(input));
        }
    }

    #[allow(dead_code)]
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
            .filter(|(_, b)| b.address as u64 == ra)
            .map(|(i, _)| i)
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

    fn step_in(&mut self) -> Result<(), DebugError> {
        let line = get_line_from_pc(&self.dwarf, self.get_pc()?)?.line;
        while get_line_from_pc(&self.dwarf, self.get_pc()?)?.line == line {
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
            .filter(|(_, b)| b.address as u64 == pc)
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

    pub fn waitpid(&self) -> Result<(), DebugError> {
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
