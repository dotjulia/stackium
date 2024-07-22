use gimli::{EvaluationResult, Reader};
use nix::{
    sys::{
        ptrace,
        wait::{waitpid, WaitPidFlag},
    },
    unistd::Pid,
};
use object::{Object, ObjectSection};
use stackium_shared::{
    Breakpoint, BreakpointPoint, Command, CommandOutput, DataType, DebugMeta, DwarfAttribute,
    FunctionMeta, Location, MemoryMap, Registers, TypeName, Variable,
};
use std::{ffi::c_void, fs, path::PathBuf, rc::Rc, sync::Arc};

pub mod breakpoint;
pub mod error;
pub mod registers;
mod util;

#[cfg(debug_assertions)]
macro_rules! debug_println {
    ($($x:tt)*) => { println!($($x)*) }
}

#[cfg(not(debug_assertions))]
macro_rules! debug_println {
    ($($x:tt)*) => {{}};
}

use crate::{
    debugger::{
        registers::FromUserRegsStruct,
        util::{get_function_meta, get_piece_addr},
    },
    prompt::{command_prompt, CommandCompleter},
    util::{dw_at_to_string, tag_to_string},
};

use self::{
    breakpoint::DebuggerBreakpoint,
    error::DebugError,
    util::{find_function_from_name, get_addr_from_line, get_functions, get_line_from_pc},
};

type ConcreteReader = gimli::read::EndianReader<gimli::NativeEndian, Arc<[u8]>>;
pub struct Debugger {
    pub child: Pid,
    breakpoints: Vec<Breakpoint>,
    pub program: PathBuf,
    dwarf: gimli::read::Dwarf<ConcreteReader>,
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

fn unit_offset<T: gimli::Reader>(
    offset: gimli::AttributeValue<T>,
) -> Option<<T as gimli::Reader>::Offset> {
    if let gimli::AttributeValue::UnitRef(r) = offset {
        Some(r.0)
    } else {
        None
    }
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
        let mut iter = dwarf.debug_info.units();
        while let Some(unit) = iter.next().unwrap() {
            let version = unit.version();
            debug_println!("Dwarf Version = {}", version);
            if version != 4 {
                eprintln!("Stackium currently only supports binaries built with dwarf debug version 4. Please compile with the \x1b[1;33m-gdwarf-4\x1b[0m flag!");
                panic!();
            }
        }
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
        known_types: DataType,
    ) -> Result<DataType, DebugError> {
        if let gimli::AttributeValue::UnitRef(r) = offset {
            let mut unit_iter = self.dwarf.units();
            while let Ok(Some(unit_header)) = unit_iter.next() {
                let abbrevs = self.dwarf.abbreviations(&unit_header)?;
                let mut tree = unit_header.entries_tree(&abbrevs, None)?;
                let root = tree.root()?;
                fn process_tree(
                    debugger: &Debugger,
                    node: gimli::EntriesTreeNode<ConcreteReader>,
                    unit_header: &gimli::UnitHeader<ConcreteReader>,
                    find_offset: gimli::UnitOffset<<ConcreteReader as gimli::Reader>::Offset>,
                    mut known_types: DataType,
                ) -> Result<Option<DataType>, DebugError> {
                    let dwarf = &debugger.dwarf;
                    if node.entry().offset() == find_offset {
                        match node.entry().tag() {
                            gimli::DW_TAG_base_type => {
                                if let (Ok(Some(name)), Ok(Some(byte_size))) = (
                                    node.entry().attr(gimli::DW_AT_name),
                                    node.entry().attr(gimli::DW_AT_byte_size),
                                ) {
                                    known_types.0.push((
                                        find_offset.0,
                                        TypeName::Name {
                                            name: name
                                                .string_value(&dwarf.debug_str)
                                                .unwrap()
                                                .to_string()
                                                .unwrap()
                                                .to_string(),
                                            byte_size: byte_size.udata_value().unwrap() as usize,
                                        },
                                    ));
                                    return Ok(Some(known_types));
                                } else {
                                    debug_println!("Failed getting type name");
                                }
                            }
                            gimli::DW_TAG_const_type => {
                                if let Ok(Some(type_field)) = node.entry().attr(gimli::DW_AT_type) {
                                    known_types =
                                        debugger.decode_type(type_field.value(), known_types)?;
                                    return Ok(Some(known_types));
                                }
                            }
                            gimli::DW_TAG_typedef => {
                                if let Ok(Some(type_field)) = node.entry().attr(gimli::DW_AT_type) {
                                    known_types =
                                        debugger.decode_type(type_field.value(), known_types)?;
                                    return Ok(Some(known_types));
                                } else {
                                    let name = if let Ok(Some(name)) =
                                        node.entry().attr(gimli::DW_AT_name)
                                    {
                                        name.string_value(&dwarf.debug_str)
                                            .unwrap()
                                            .to_string()
                                            .unwrap()
                                            .to_string()
                                    } else {
                                        String::new()
                                    };
                                    known_types.0.push((
                                        find_offset.0,
                                        TypeName::Name { name, byte_size: 0 },
                                    ));
                                    return Ok(Some(known_types));
                                }
                            }
                            gimli::DW_TAG_pointer_type => {
                                if let Ok(Some(type_field)) = node.entry().attr(gimli::DW_AT_type) {
                                    //TODO: Find fix for recursive types
                                    // debug_println!(
                                    //     "Resolving pointer for type {:?}",
                                    //     unit_offset(type_field.value())
                                    // );
                                    // debug_println!("Known types: {:?}", known_types);
                                    let index = known_types.0.iter().position(|e| {
                                        e.0 == unit_offset(type_field.value()).unwrap()
                                    });
                                    if let Some(index) = index {
                                        let mut ret_vec = known_types.clone();
                                        debug_println!("Type already known");
                                        ret_vec.0.push((
                                            unit_offset(type_field.value()).unwrap(),
                                            TypeName::Ref { index: Some(index) },
                                        ));
                                        return Ok(Some(ret_vec));
                                    }
                                    // [Current Types] + Ptr Type + [Types]
                                    let next_index = known_types.0.len() + 1;
                                    known_types.0.push((
                                        find_offset.0,
                                        TypeName::Ref {
                                            index: Some(next_index),
                                        },
                                    ));
                                    let sub_type = debugger
                                        .decode_type(type_field.value(), known_types.clone())?;
                                    known_types.0 = sub_type.0;
                                    return Ok(Some(known_types));
                                } else {
                                    known_types
                                        .0
                                        .push((find_offset.0, TypeName::Ref { index: None }));
                                    return Ok(Some(known_types));
                                }
                            }
                            gimli::DW_TAG_array_type => {
                                if let Ok(Some(type_field)) = node.entry().attr(gimli::DW_AT_type) {
                                    let mut children_iter = node.children();
                                    let mut lengths = vec![];
                                    while let Ok(Some(child)) = children_iter.next() {
                                        if let Ok(Some(count)) =
                                            child.entry().attr(gimli::DW_AT_count)
                                        {
                                            lengths.push(count.udata_value().unwrap() as usize);
                                        } else {
                                            debug_println!(
                                                "Found child entry but failed getting count"
                                            );
                                        }
                                    }
                                    known_types.0.push((
                                        find_offset.0,
                                        TypeName::Name {
                                            name: String::new(),
                                            byte_size: 0,
                                        },
                                    ));
                                    let arr_index = known_types.0.len() - 1;

                                    let sub_type = if let Some(sub_type) =
                                        known_types.0.iter().position(|t| {
                                            t.0 == unit_offset(type_field.value()).unwrap()
                                        }) {
                                        sub_type
                                    } else {
                                        let sub_type = debugger
                                            .decode_type(type_field.value(), known_types.clone())?;
                                        let i = known_types.0.len();
                                        known_types.0 = sub_type.0;
                                        i
                                    };

                                    known_types.0[arr_index] = (
                                        find_offset.0,
                                        TypeName::Arr {
                                            arr_type: sub_type,
                                            count: lengths,
                                        },
                                    );
                                    return Ok(Some(known_types));
                                } else {
                                    debug_println!("Failed getting array type");
                                }
                            }
                            gimli::DW_TAG_structure_type => {
                                let (name, byte_size) = (
                                    node.entry().attr(gimli::DW_AT_name)?,
                                    node.entry().attr(gimli::DW_AT_byte_size)?,
                                );
                                let name = if let Some(name) = name {
                                    name.string_value(&dwarf.debug_str)
                                        .unwrap()
                                        .to_string()
                                        .unwrap()
                                        .to_string()
                                } else {
                                    "unnamed struct".to_owned()
                                };
                                let byte_size = if let Some(byte_size) = byte_size {
                                    byte_size.udata_value().unwrap()
                                } else {
                                    0
                                };
                                // Push Structure first in case of self referential struct
                                debug_println!("Decoding struct: {} {:?}", &name, known_types);

                                known_types.0.push((
                                    find_offset.0,
                                    TypeName::ProductType {
                                        name: name.clone(),
                                        members: vec![],
                                        byte_size: byte_size as usize,
                                    },
                                ));
                                let struct_index = known_types.0.len() - 1;
                                let mut children_iter = node.children();
                                let mut types: Vec<(String, usize, usize)> = vec![];
                                while let Ok(Some(child)) = children_iter.next() {
                                    if let (
                                        Ok(Some(name)),
                                        Ok(Some(typeoffset)),
                                        Ok(Some(byteoffset)),
                                    ) = (
                                        child.entry().attr(gimli::DW_AT_name),
                                        child.entry().attr(gimli::DW_AT_type),
                                        child.entry().attr(gimli::DW_AT_data_member_location),
                                    ) {
                                        let name = name
                                            .string_value(&dwarf.debug_str)
                                            .unwrap()
                                            .to_string()
                                            .unwrap()
                                            .to_string();
                                        let index = if let Some(index) =
                                            known_types.0.iter().position(|t| {
                                                t.0 == unit_offset(typeoffset.value()).unwrap()
                                            }) {
                                            index
                                        } else {
                                            let membertype = debugger.decode_type(
                                                typeoffset.value(),
                                                known_types.clone(),
                                            )?;
                                            let i = known_types.0.len();
                                            known_types.0 = membertype.0;
                                            i
                                        };
                                        let byteoffset = byteoffset.udata_value().unwrap();
                                        types.push((name, index, byteoffset as usize));
                                    } else {
                                        debug_println!("Failed to decode member type");
                                    }
                                }
                                known_types.0[struct_index] = (
                                    find_offset.0,
                                    TypeName::ProductType {
                                        name,
                                        members: types,
                                        byte_size: byte_size as usize,
                                    },
                                );
                                return Ok(Some(known_types));
                            }
                            _ => {
                                debug_println!(
                                    "Invalid entry: {:?}, offset: {:?}",
                                    node.entry().tag(),
                                    node.entry().offset()
                                );
                                return Err(DebugError::InvalidType);
                            }
                        }
                        debug_println!(
                            "Failed parsing entry: {:?}, offset: {:?}",
                            node.entry().tag(),
                            node.entry().offset()
                        );
                        return Err(DebugError::InvalidType);
                    }
                    let mut children = node.children();
                    while let Some(child) = children.next()? {
                        match process_tree(
                            debugger,
                            child,
                            unit_header,
                            find_offset,
                            known_types.clone(),
                        )? {
                            Some(t) => {
                                return Ok(Some(t));
                            }
                            None => (),
                        }
                    }
                    Ok(None)
                }
                if let Some(t) = process_tree(self, root, &unit_header, r, known_types.clone())? {
                    return Ok(t);
                }
            }
            debug_println!("Didn't find header");
            Err(DebugError::InvalidType)
        } else {
            debug_println!("Invalid offset type");
            Err(DebugError::InvalidType)
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

    pub fn read_variables(&self) -> Result<Vec<Variable>, DebugError> {
        let mut sub_entry;
        let mut unit;
        let mut variables = Vec::new();
        let mut curr_high_pc = 0u64;
        let mut curr_low_pc = 0u64;
        iter_every_entry!(self, sub_entry unit | {
            // debug_println!("{:#?}", tag_to_string(sub_entry.tag()));
            if sub_entry.tag() == gimli::DW_TAG_subprogram || sub_entry.tag() == gimli::DW_TAG_lexical_block{

                if let Ok(Some(lpc)) = sub_entry.attr_value(gimli::DW_AT_low_pc) {
                    match lpc {
                        gimli::AttributeValue::Addr(addr) => {
                            curr_low_pc = addr;
                        },
                        _ => { debug_println!("unexpected low pc value: {:#?}", lpc); }
                    }
                }

                if let Ok(Some(hpc)) = sub_entry.attr_value(gimli::DW_AT_high_pc) {
                    curr_high_pc = curr_low_pc + hpc.udata_value().unwrap_or(0);
                }

            }
            if sub_entry.tag() == gimli::DW_TAG_variable || sub_entry.tag() == gimli::DW_TAG_formal_parameter {
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
                                let base_pointer = Registers::from_regs(self.get_registers()?).base_pointer;
                                result = evaluation.resume_with_frame_base(base_pointer)?;

                            },
                            EvaluationResult::RequiresTls(_) => todo!(),
                            EvaluationResult::RequiresCallFrameCfa => todo!(),
                            EvaluationResult::RequiresAtLocation(_) => todo!(),
                            EvaluationResult::RequiresEntryValue(_) => todo!(),
                            EvaluationResult::RequiresParameterRef(_) => todo!(),
                            EvaluationResult::RequiresRelocatedAddress(addr) => {
                                // let mut iter = self.dwarf.debug_info.units();
                                // while let Ok(Some(header)) = iter.next() {
                                    // let unit = self.dwarf.unit(header);
                                // }
                                // todo!()
                                result = evaluation.resume_with_relocated_address(addr)?;
                            },
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
                var.type_name = self.decode_type(sub_entry.attr(gimli::DW_AT_type)?.unwrap().value(), DataType(vec![])).ok();

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
                var.high_pc = curr_high_pc;
                var.low_pc = curr_low_pc;
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
        let mut frame_pointer = Registers::from_regs(self.get_registers()?).base_pointer;
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
        let pc = Registers::from_regs(self.get_registers()?).instruction_pointer;
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
            binary_name: self.program.to_str().unwrap().to_owned(),
            file_type: format!("{:?}", self.dwarf.file_type),
            functions,
            vars,
            files,
        })
    }

    pub fn get_maps(&self) -> Result<Vec<MemoryMap>, DebugError> {
        let maps = std::fs::read_to_string(format!("/proc/{}/maps", self.child))?;
        let lines = maps.lines();
        use regex::Regex;
        let re = Regex::new(
                    r"^([0-9a-fA-F]+)-([0-9a-fA-F]+) (r|-)(w|-)(x|-)(p|s) ([0-9a-fA-f]+) [0-9a-fA-F]+:[0-9a-fA-F]+ [0-9]+ *(.+)?"
                )
                .unwrap();
        let mut maps: Vec<MemoryMap> = Vec::new();
        for line in lines {
            let captures = re.captures(line).unwrap();
            maps.push(MemoryMap {
                from: u64::from_str_radix(&captures[1], 16).unwrap(),
                to: u64::from_str_radix(&captures[2], 16).unwrap(),
                read: &captures[3] == "r",
                write: &captures[4] == "w",
                execute: &captures[5] == "x",
                shared: &captures[6] == "s",
                offset: u64::from_str_radix(&captures[7], 16).unwrap(),
                mapped: captures.get(8).map_or("", |m| m.as_str()).to_owned(),
            });
        }
        Ok(maps)
    }

    pub fn process_command(&mut self, command: Command) -> Result<CommandOutput, DebugError> {
        match command {
            Command::Maps => Ok(CommandOutput::Maps(self.get_maps()?)),
            Command::Disassemble => Ok(CommandOutput::File(
                std::str::from_utf8(
                    &std::process::Command::new("objdump")
                        .arg("--disassemble")
                        .arg(self.program.clone().into_os_string())
                        .output()?
                        .stdout,
                )?
                .to_string(),
            )),
            Command::ReadMemory(addr, size) => {
                Ok(CommandOutput::Memory(self.read_memory(addr, size)?))
            }
            Command::GetFunctions => Ok(CommandOutput::Functions(get_functions(&self.dwarf)?)),
            Command::WaitPid => {
                self.waitpid_flag(Some(WaitPidFlag::WNOHANG))?;
                Ok(CommandOutput::None)
            }
            Command::GetFile(filename) => Ok(CommandOutput::File(fs::read_to_string(filename)?)),
            Command::GetBreakpoints => Ok(CommandOutput::Breakpoints(self.breakpoints.clone())),
            Command::DebugMeta => Ok(CommandOutput::DebugMeta(self.debug_meta()?)),
            Command::DumpDwarf => Ok(CommandOutput::DwarfAttributes(self.dump_dwarf_attrs()?)),
            Command::Help => Ok(CommandOutput::Help(CommandCompleter::default().commands)),
            Command::Backtrace => Ok(CommandOutput::Backtrace(self.backtrace()?)),
            Command::ReadVariables => Ok(CommandOutput::Variables(self.read_variables()?)),
            Command::DiscoverVariables => Ok(CommandOutput::DiscoveredVariables(
                self.discover_variables()?,
            )),
            Command::Read(addr) => Ok(CommandOutput::Data(self.read(addr as *mut _)?)),
            Command::Continue => {
                self.continue_exec()?;
                Ok(CommandOutput::None)
            }
            Command::Quit => std::process::exit(0),
            Command::StepOut => self.step_out().map(|_| CommandOutput::None),
            Command::FindLine { line, filename } => {
                let addr = get_addr_from_line(&self.dwarf, line, filename)?;
                Ok(CommandOutput::Data(addr))
            }
            Command::FindFunc(name) => {
                let func = find_function_from_name(&self.dwarf, name);
                Ok(CommandOutput::FunctionMeta(func?))
            }
            Command::StepIn => self.step_in().map(|_| CommandOutput::None),
            Command::StepInstruction => self.step_instruction().map(|_| CommandOutput::None),
            Command::ProgramCounter => Ok(CommandOutput::Data(
                Registers::from_regs(self.get_registers()?).instruction_pointer,
            )),
            Command::SetBreakpoint(a) => match a {
                BreakpointPoint::Name(name) => {
                    debug_println!("Name: '{}'", &name);
                    let func = find_function_from_name(&self.dwarf, name)?;
                    if let Some(addr) = func.low_pc {
                        debug_println!(
                            "Setting breakpoint at function: {:?} {:#x} for {:?}",
                            func.name,
                            addr,
                            self.child
                        );
                        if self.breakpoints.iter().any(|b| b.address == addr) {
                            return Err(DebugError::BreakpointInvalidState);
                        }
                        let mut breakpoint =
                            Breakpoint::new(&self.dwarf, self.child, addr as *const u8)?;
                        breakpoint.enable(self.child)?;
                        self.breakpoints.push(breakpoint);
                    } else {
                        debug_println!("Couldn't find function: {:?}", func.name);
                    }
                    Ok(CommandOutput::None)
                }
                BreakpointPoint::Address(addr) => {
                    debug_println!("Setting breakpoint at address: {:?}", addr);

                    if self.breakpoints.iter().any(|b| b.address == addr) {
                        return Err(DebugError::BreakpointInvalidState);
                    }
                    let mut breakpoint =
                        Breakpoint::new(&self.dwarf, self.child, addr as *const u8)?;
                    breakpoint.enable(self.child)?;
                    self.breakpoints.push(breakpoint);
                    Ok(CommandOutput::None)
                }
                BreakpointPoint::Location(location) => {
                    debug_println!("Setting a breakpoint at location: {:?}", location);
                    let addr = get_addr_from_line(&self.dwarf, location.line, location.file)?;

                    if self.breakpoints.iter().any(|b| b.address == addr) {
                        return Err(DebugError::BreakpointInvalidState);
                    }
                    let mut breakpoint =
                        Breakpoint::new(&self.dwarf, self.child, addr as *const u8)?;
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
                Ok(CommandOutput::Registers(Registers::from_regs(regs)))
            }
            Command::Location => Ok(CommandOutput::Location(get_line_from_pc(
                &self.dwarf,
                self.get_pc()?,
            )?)),
            Command::DeleteBreakpoint(address) => {
                match self
                    .breakpoints
                    .iter_mut()
                    .find(|breakpoint| breakpoint.address == address)
                {
                    Some(breakpoint) => {
                        breakpoint.disable(self.child)?;
                        self.breakpoints = self
                            .breakpoints
                            .iter()
                            .filter(|b| b.address != address)
                            .map(|b| b.clone())
                            .collect();
                        Ok(CommandOutput::None)
                    }
                    None => Err(DebugError::FunctionNotFound),
                }
            }
        }
    }

    pub fn debug_loop(mut self) -> Result<(), DebugError> {
        loop {
            let input = command_prompt()?;
            println!("{:#?}", self.process_command(input));
        }
    }

    // #[allow(dead_code)]
    // fn write(&self, addr: *mut c_void, data: u64) -> Result<(), DebugError> {
    //     match unsafe { ptrace::write(self.child, addr, data as *mut _) } {
    //         Ok(_) => Ok(()),
    //         Err(e) => Err(DebugError::NixError(e)),
    //     }
    // }

    pub fn read(&self, addr: *mut c_void) -> Result<u64, DebugError> {
        match ptrace::read(self.child, addr) {
            Ok(d) => Ok(d as u64),
            Err(e) => Err(DebugError::NixError(e)),
        }
    }

    pub fn read_memory(&self, addr: u64, len: u64) -> Result<Vec<u8>, DebugError> {
        let mut values = vec![];
        // debug_println!("Reading @ {:#x} : {}", addr, len);
        for i in 0..len {
            let v = ptrace::read(self.child, (addr + i as u64) as *mut c_void)?;
            values.push((v & 0xFF) as u8);
        }
        Ok(values)
    }

    fn get_pc(&self) -> Result<u64, DebugError> {
        Ok(Registers::from_regs(self.get_registers()?).instruction_pointer)
    }

    fn set_pc(&self, pc: u64) -> Result<(), DebugError> {
        let mut regs = self.get_registers()?;
        #[cfg(target_arch = "x86_64")]
        {
            regs.rip = pc;
        }
        #[cfg(target_arch = "aarch64")]
        {
            regs.pc = pc;
        }
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
        let fp = Registers::from_regs(self.get_registers()?).base_pointer;
        let ra = self.read((fp + 8) as *mut c_void)?;
        let bp: Vec<_> = self
            .breakpoints
            .iter()
            .enumerate()
            .filter(|(_, b)| b.address as u64 == ra)
            .map(|(i, _)| i)
            .collect();
        if bp.len() == 0 {
            let mut breakpoint = Breakpoint::new(&self.dwarf, self.child, ra as *const u8)?;
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
    pub fn waitpid(&self) -> Result<(), DebugError> {
        self.waitpid_flag(Some(WaitPidFlag::WUNTRACED))
    }

    pub fn waitpid_flag(&self, flags: Option<WaitPidFlag>) -> Result<(), DebugError> {
        match waitpid(self.child, flags) {
            Ok(s) => match s {
                nix::sys::wait::WaitStatus::Exited(pid, status) => {
                    debug_println!("Child {} exited with status: {}", pid, status);
                    Ok(())
                }
                nix::sys::wait::WaitStatus::Signaled(pid, status, coredump) => {
                    debug_println!(
                        "Child {} signaled with status: {:?} and coredump: {}",
                        pid,
                        status,
                        coredump
                    );
                    Ok(())
                }
                nix::sys::wait::WaitStatus::Stopped(pid, signal) => {
                    match signal {
                        nix::sys::signal::Signal::SIGTRAP => {
                            let siginfo = nix::sys::ptrace::getsiginfo(pid)?;
                            // I think nix doesn't have a constant for this
                            if siginfo.si_code == 128 {
                                debug_println!("Hit breakpoint!");

                                // step back one instruction
                                self.set_pc(self.get_pc()? - 1)?;
                            } else {
                                debug_println!(
                                    "Child {} stopped with {:?} and code {}",
                                    pid,
                                    siginfo,
                                    siginfo.si_code
                                );
                            }
                        }
                        nix::sys::signal::Signal::SIGSEGV => {
                            println!("Segmentation fault!");

                            match ptrace::kill(self.child) {
                                Ok(a) => debug_println!("Killed child: {:?}", a),
                                Err(e) => debug_println!("Failed to kill child: {:?}", e),
                            }
                        }
                        _ => {
                            debug_println!("Child {} stopped with signal: {:?}", pid, signal);
                        }
                    }
                    Ok(())
                }
                nix::sys::wait::WaitStatus::Continued(pid) => {
                    debug_println!("Child {} continued", pid);
                    Ok(())
                }
                #[cfg(target_os = "linux")]
                nix::sys::wait::WaitStatus::StillAlive => {
                    debug_println!("Child is still alive");
                    Ok(())
                }
                #[cfg(target_os = "linux")]
                nix::sys::wait::WaitStatus::PtraceEvent(pid, signal, int) => {
                    debug_println!(
                        "Child {} ptrace event with signal: {:?} and int: {}",
                        pid,
                        signal,
                        int
                    );
                    Ok(())
                }
                #[cfg(target_os = "linux")]
                nix::sys::wait::WaitStatus::PtraceSyscall(pid) => {
                    debug_println!("Child {} ptrace syscall", pid);
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
                debug_println!("Warning: continuing execution from non-breakpoint");
            }
            Err(e) => return Err(e),
        }
        ptrace::cont(self.child, None).map_err(|e| DebugError::NixError(e))?;
        self.waitpid()
    }
}
