use std::{ffi::c_void, fmt::Display, fs, num::NonZeroU64, path::PathBuf, sync::Arc};

use addr2line::gimli::{
    self, AttributeValue, DebuggingInformationEntry, Evaluation, EvaluationResult,
};
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
    UnitNotFound,
    InvalidType,
    IoError(std::io::Error),
    GimliError(gimli::Error),
    ObjectError(addr2line::object::Error),
    BreakpointInvalidState,
    InvalidRegister,
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
    #[allow(dead_code)]
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

macro_rules! iter_every_entry {
    ($self:ident, $entry:ident $unit:ident | $body:block) => {
        let dwarf = $self.context.dwarf();
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

// fn get_entry_at_offset(&self, offset: R::Offset) -> Result<gimli::Unit<R>, DebugError> {
//     let mut sub_entry;
//     let mut unit;
//     iter_every_entry!(self, sub_entry unit | {
//         if sub_entry.offset().0 == offset {
//             return Ok(sub_entry);
//         }
//     });
//     Err(DebugError::UnitNotFound)
// }
fn tag_to_string(tag: gimli::DwTag) -> String {
    match tag {
        gimli::DW_TAG_array_type => "DW_TAG_array_type",
        gimli::DW_TAG_class_type => "DW_TAG_class_type",
        gimli::DW_TAG_entry_point => "DW_TAG_entry_point",
        gimli::DW_TAG_enumeration_type => "DW_TAG_enumeration_type",
        gimli::DW_TAG_formal_parameter => "DW_TAG_formal_parameter",
        gimli::DW_TAG_imported_declaration => "DW_TAG_imported_declaration",
        gimli::DW_TAG_label => "DW_TAG_label",
        gimli::DW_TAG_lexical_block => "DW_TAG_lexical_block",
        gimli::DW_TAG_member => "DW_TAG_member",
        gimli::DW_TAG_pointer_type => "DW_TAG_pointer_type",
        gimli::DW_TAG_reference_type => "DW_TAG_reference_type",
        gimli::DW_TAG_compile_unit => "DW_TAG_compile_unit",
        gimli::DW_TAG_string_type => "DW_TAG_string_type",
        gimli::DW_TAG_structure_type => "DW_TAG_structure_type",
        gimli::DW_TAG_subroutine_type => "DW_TAG_subroutine_type",
        gimli::DW_TAG_typedef => "DW_TAG_typedef",
        gimli::DW_TAG_union_type => "DW_TAG_union_type",
        gimli::DW_TAG_unspecified_parameters => "DW_TAG_unspecified_parameters",
        gimli::DW_TAG_variant => "DW_TAG_variant",
        gimli::DW_TAG_common_block => "DW_TAG_common_block",
        gimli::DW_TAG_common_inclusion => "DW_TAG_common_inclusion",
        gimli::DW_TAG_inheritance => "DW_TAG_inheritance",
        gimli::DW_TAG_inlined_subroutine => "DW_TAG_inlined_subroutine",
        gimli::DW_TAG_module => "DW_TAG_module",
        gimli::DW_TAG_ptr_to_member_type => "DW_TAG_ptr_to_member_type",
        gimli::DW_TAG_set_type => "DW_TAG_set_type",
        gimli::DW_TAG_subrange_type => "DW_TAG_subrange_type",
        gimli::DW_TAG_with_stmt => "DW_TAG_with_stmt",
        gimli::DW_TAG_access_declaration => "DW_TAG_access_declaration",
        gimli::DW_TAG_base_type => "DW_TAG_base_type",
        gimli::DW_TAG_catch_block => "DW_TAG_catch_block",
        gimli::DW_TAG_const_type => "DW_TAG_const_type",
        gimli::DW_TAG_constant => "DW_TAG_constant",
        gimli::DW_TAG_enumerator => "DW_TAG_enumerator",
        gimli::DW_TAG_file_type => "DW_TAG_file_type",
        gimli::DW_TAG_friend => "DW_TAG_friend",
        gimli::DW_TAG_namelist => "DW_TAG_namelist",
        gimli::DW_TAG_namelist_item => "DW_TAG_namelist_item",
        gimli::DW_TAG_packed_type => "DW_TAG_packed_type",
        gimli::DW_TAG_subprogram => "DW_TAG_subprogram",
        gimli::DW_TAG_template_type_parameter => "DW_TAG_template_type_parameter",
        gimli::DW_TAG_template_value_parameter => "DW_TAG_template_value_parameter",
        gimli::DW_TAG_thrown_type => "DW_TAG_thrown_type",
        gimli::DW_TAG_try_block => "DW_TAG_try_block",
        gimli::DW_TAG_variant_part => "DW_TAG_variant_part",
        gimli::DW_TAG_variable => "DW_TAG_variable",
        gimli::DW_TAG_volatile_type => "DW_TAG_volatile_type",
        gimli::DW_TAG_dwarf_procedure => "DW_TAG_dwarf_procedure",
        gimli::DW_TAG_restrict_type => "DW_TAG_restrict_type",
        gimli::DW_TAG_interface_type => "DW_TAG_interface_type",
        gimli::DW_TAG_namespace => "DW_TAG_namespace",
        gimli::DW_TAG_imported_module => "DW_TAG_imported_module",
        gimli::DW_TAG_unspecified_type => "DW_TAG_unspecified_type",
        gimli::DW_TAG_partial_unit => "DW_TAG_partial_unit",
        gimli::DW_TAG_imported_unit => "DW_TAG_imported_unit",
        gimli::DW_TAG_condition => "DW_TAG_condition",
        gimli::DW_TAG_shared_type => "DW_TAG_shared_type",
        gimli::DW_TAG_type_unit => "DW_TAG_type_unit",
        gimli::DW_TAG_rvalue_reference_type => "DW_TAG_rvalue_reference_type",
        gimli::DW_TAG_template_alias => "DW_TAG_template_alias",
        gimli::DW_TAG_lo_user => "DW_TAG_lo_user",
        gimli::DW_TAG_hi_user => "DW_TAG_hi_user",
        _ => "Unknown tag",
    }
    .to_owned()
}

fn dw_at_to_string(attr: gimli::DwAt) -> String {
    match attr {
        gimli::DW_AT_sibling => "DW_AT_sibling",
        gimli::DW_AT_location => "DW_AT_location",
        gimli::DW_AT_name => "DW_AT_name",
        gimli::DW_AT_ordering => "DW_AT_ordering",
        gimli::DW_AT_byte_size => "DW_AT_byte_size",
        gimli::DW_AT_bit_offset => "DW_AT_bit_offset",
        gimli::DW_AT_bit_size => "DW_AT_bit_size",
        gimli::DW_AT_stmt_list => "DW_AT_stmt_list",
        gimli::DW_AT_low_pc => "DW_AT_low_pc",
        gimli::DW_AT_high_pc => "DW_AT_high_pc",
        gimli::DW_AT_language => "DW_AT_language",
        gimli::DW_AT_discr => "DW_AT_discr",
        gimli::DW_AT_discr_value => "DW_AT_discr_value",
        gimli::DW_AT_visibility => "DW_AT_visibility",
        gimli::DW_AT_import => "DW_AT_import",
        gimli::DW_AT_string_length => "DW_AT_string_length",
        gimli::DW_AT_common_reference => "DW_AT_common_reference",
        gimli::DW_AT_comp_dir => "DW_AT_comp_dir",
        gimli::DW_AT_const_value => "DW_AT_const_value",
        gimli::DW_AT_containing_type => "DW_AT_containing_type",
        gimli::DW_AT_default_value => "DW_AT_default_value",
        gimli::DW_AT_inline => "DW_AT_inline",
        gimli::DW_AT_is_optional => "DW_AT_is_optional",
        gimli::DW_AT_lower_bound => "DW_AT_lower_bound",
        gimli::DW_AT_producer => "DW_AT_producer",
        gimli::DW_AT_prototyped => "DW_AT_prototyped",
        gimli::DW_AT_return_addr => "DW_AT_return_addr",
        gimli::DW_AT_start_scope => "DW_AT_start_scope",
        gimli::DW_AT_bit_stride => "DW_AT_bit_stride",
        gimli::DW_AT_upper_bound => "DW_AT_upper_bound",
        gimli::DW_AT_abstract_origin => "DW_AT_abstract_origin",
        gimli::DW_AT_accessibility => "DW_AT_accessibility",
        gimli::DW_AT_address_class => "DW_AT_address_class",
        gimli::DW_AT_artificial => "DW_AT_artificial",
        gimli::DW_AT_base_types => "DW_AT_base_types",
        gimli::DW_AT_calling_convention => "DW_AT_calling_convention",
        gimli::DW_AT_count => "DW_AT_count",
        gimli::DW_AT_data_member_location => "DW_AT_data_member_location",
        gimli::DW_AT_decl_column => "DW_AT_decl_column",
        gimli::DW_AT_decl_file => "DW_AT_decl_file",
        gimli::DW_AT_decl_line => "DW_AT_decl_line",
        gimli::DW_AT_declaration => "DW_AT_declaration",
        gimli::DW_AT_discr_list => "DW_AT_discr_list",
        gimli::DW_AT_encoding => "DW_AT_encoding",
        gimli::DW_AT_external => "DW_AT_external",
        gimli::DW_AT_frame_base => "DW_AT_frame_base",
        gimli::DW_AT_friend => "DW_AT_friend",
        gimli::DW_AT_identifier_case => "DW_AT_identifier_case",
        gimli::DW_AT_macro_info => "DW_AT_macro_info",
        gimli::DW_AT_namelist_item => "DW_AT_namelist_item",
        gimli::DW_AT_priority => "DW_AT_priority",
        gimli::DW_AT_segment => "DW_AT_segment",
        gimli::DW_AT_specification => "DW_AT_specification",
        gimli::DW_AT_static_link => "DW_AT_static_link",
        gimli::DW_AT_type => "DW_AT_type",
        gimli::DW_AT_use_location => "DW_AT_use_location",
        gimli::DW_AT_variable_parameter => "DW_AT_variable_parameter",
        gimli::DW_AT_virtuality => "DW_AT_virtuality",
        gimli::DW_AT_vtable_elem_location => "DW_AT_vtable_elem_location",
        gimli::DW_AT_allocated => "DW_AT_allocated",
        gimli::DW_AT_associated => "DW_AT_associated",
        gimli::DW_AT_data_location => "DW_AT_data_location",
        gimli::DW_AT_byte_stride => "DW_AT_byte_stride",
        gimli::DW_AT_entry_pc => "DW_AT_entry_pc",
        gimli::DW_AT_use_UTF8 => "DW_AT_use_UTF8",
        gimli::DW_AT_extension => "DW_AT_extension",
        gimli::DW_AT_ranges => "DW_AT_ranges",
        gimli::DW_AT_trampoline => "DW_AT_trampoline",
        gimli::DW_AT_call_column => "DW_AT_call_column",
        gimli::DW_AT_call_file => "DW_AT_call_file",
        gimli::DW_AT_call_line => "DW_AT_call_line",
        gimli::DW_AT_description => "DW_AT_description",
        gimli::DW_AT_binary_scale => "DW_AT_binary_scale",
        gimli::DW_AT_decimal_scale => "DW_AT_decimal_scale",
        gimli::DW_AT_small => "DW_AT_small",
        gimli::DW_AT_decimal_sign => "DW_AT_decimal_sign",
        gimli::DW_AT_digit_count => "DW_AT_digit_count",
        gimli::DW_AT_picture_string => "DW_AT_picture_string",
        gimli::DW_AT_mutable => "DW_AT_mutable",
        gimli::DW_AT_threads_scaled => "DW_AT_threads_scaled",
        gimli::DW_AT_explicit => "DW_AT_explicit",
        gimli::DW_AT_object_pointer => "DW_AT_object_pointer",
        gimli::DW_AT_endianity => "DW_AT_endianity",
        gimli::DW_AT_elemental => "DW_AT_elemental",
        gimli::DW_AT_pure => "DW_AT_pure",
        gimli::DW_AT_recursive => "DW_AT_recursive",
        gimli::DW_AT_signature => "DW_AT_signature",
        gimli::DW_AT_main_subprogram => "DW_AT_main_subprogram",
        gimli::DW_AT_data_bit_offset => "DW_AT_data_bit_offset",
        gimli::DW_AT_const_expr => "DW_AT_const_expr",
        gimli::DW_AT_enum_class => "DW_AT_enum_class",
        gimli::DW_AT_linkage_name => "DW_AT_linkage_name",
        gimli::DW_AT_lo_user => "DW_AT_lo_user",
        gimli::DW_AT_hi_user => "DW_AT_hi_user",
        _ => "Unknown",
    }
    .to_owned()
}

#[derive(Debug)]
enum TypeName {
    Name(String),
    Ref(Box<TypeName>),
}

impl<R: gimli::Reader + std::cmp::PartialEq> Debugger<R> {
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

    #[allow(dead_code)]
    fn dump_dwarf_attrs(&self) -> Result<(), DebugError> {
        let mut sub_entry;
        let mut unit;
        iter_every_entry!(self, sub_entry unit | {
            println!("NAM: {:?}", unit.name.clone().unwrap().to_string().unwrap());
            println!("ADR: {:#x?}", sub_entry.offset().0);
            println!("TAG: {:?}", tag_to_string(sub_entry.tag()));
            let mut attrs = sub_entry.attrs();
            while let Some(attr) = attrs.next()? {
                println!("ATR: {:?}", dw_at_to_string(attr.name()));
            }
            println!("\n");
        });
        Ok(())
    }

    fn decode_type(&self, offset: AttributeValue<R>) -> Result<TypeName, DebugError> {
        if let gimli::AttributeValue::UnitRef(r) = offset {
            let mut offset_entry;
            let mut offset_unit;
            find_entry_with_offset!(r, self, offset_entry offset_unit | {
               if let Some(name) = offset_entry.attr_value(gimli::DW_AT_name)? {
                return Ok(TypeName::Name(name.string_value(&self.context.dwarf().debug_str).unwrap().to_string().unwrap().to_string()));
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

    fn read_variables(&self) -> Result<(), DebugError> {
        let mut sub_entry;
        let mut unit;
        iter_every_entry!(self, sub_entry unit | {
                    if sub_entry.tag() == gimli::DW_TAG_variable {
                        if let Some(location) = sub_entry.attr_value(gimli::DW_AT_location)? {
                            let location = location.exprloc_value().unwrap();
                            let mut evaluation = location.evaluation(unit.encoding());
                            let mut result = evaluation.evaluate().unwrap();
                            while result != EvaluationResult::Complete {
                                match result {
                                    EvaluationResult::Complete => panic!(),
                                    EvaluationResult::RequiresMemory { address, size, space, base_type } => todo!(),
                                    EvaluationResult::RequiresRegister { register, base_type } => {
                                        let value = self.get_register_from_abi(register.0)?;
                                        result = evaluation.resume_with_register(gimli::Value::U64(value))?;
                                    },
                                    EvaluationResult::RequiresFrameBase => todo!(),
                                    EvaluationResult::RequiresTls(_) => todo!(),
                                    EvaluationResult::RequiresCallFrameCfa => todo!(),
                                    EvaluationResult::RequiresAtLocation(_) => todo!(),
                                    EvaluationResult::RequiresEntryValue(_) => todo!(),
                                    EvaluationResult::RequiresParameterRef(_) => todo!(),
                                    EvaluationResult::RequiresRelocatedAddress(_) => todo!(),
                                    EvaluationResult::RequiresIndexedAddress { index, relocate } => todo!(),
                                    EvaluationResult::RequiresBaseType(_) => todo!(),
        }
                            }
                        }
                        if let Some(name) = sub_entry.attr(gimli::DW_AT_name)? {
                            if let Some(name) = name.string_value(&self.context.dwarf().debug_str) {
                                let name = name.to_string()?;
                                println!("VAR: {:?}", name);
                                println!("{:?}", self.decode_type(sub_entry.attr(gimli::DW_AT_type)?.unwrap().value()));
                                // self.get_entry_at_offset(offset)?;
                            }
                        }
                    }
                });
        Ok(())
    }

    fn get_func_from_addr(&self, addr: u64) -> Result<FunctionMeta, DebugError> {
        let mut meta;
        let mut entry;
        let mut unit;
        iter_every_entry!(
            self,
            entry unit | {
                if entry.tag() == gimli::DW_TAG_subprogram {
                    meta = get_function_meta(&entry, &self.context.dwarf())?;
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
            max_depth -= 1;
            if max_depth == 0 {
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
                Command::DumpDwarf => self.dump_dwarf_attrs()?,
                Command::Help(commands) => {
                    println!("Available commands: ");
                    for command in commands {
                        println!("{}", command);
                    }
                }
                Command::Backtrace => self.backtrace()?,
                Command::ReadVariables => self.read_variables()?,
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
                    crate::prompt::BreakpointPoint::Name(name) => {
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
                    crate::prompt::BreakpointPoint::Address(addr) => {
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
