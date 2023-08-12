use std::num::NonZeroU64;

use gimli::Reader;
use stackium_shared::FunctionMeta;

use super::{error::DebugError, Location};

pub fn get_function_meta<T: Reader>(
    entry: &gimli::DebuggingInformationEntry<T, <T as gimli::Reader>::Offset>,
    dwarf: &gimli::Dwarf<T>,
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
                } else if let Some(str) = attr.string_value(&dwarf.debug_str) {
                    name = Some(str.to_string().unwrap().to_string());
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

pub fn get_piece_addr<T: gimli::Reader>(piece: &gimli::Piece<T>) -> Option<u64> {
    match piece.location {
        gimli::Location::Address { address } => Some(address),
        _ => None,
    }
}

pub fn get_functions<T: gimli::Reader>(
    dwarf: &gimli::Dwarf<T>,
) -> Result<Vec<FunctionMeta>, DebugError> {
    let mut units = dwarf.units();
    let mut ret_val = vec![];
    while let Some(unit_header) = units.next()? {
        let unit = dwarf.unit(unit_header)?;
        let mut cursor = unit.entries();
        while let Some((_, entry)) = cursor.next_dfs()? {
            if entry.tag() != gimli::DW_TAG_subprogram {
                continue;
            }
            ret_val.push(get_function_meta(entry, &dwarf)?);
        }
    }
    Ok(ret_val)
}

pub fn find_function_from_name<T: gimli::Reader>(
    dwarf: &gimli::Dwarf<T>,
    name_to_find: String,
) -> Result<FunctionMeta, DebugError> {
    let mut units = dwarf.units();
    while let Some(unit_header) = units.next()? {
        let unit = dwarf.unit(unit_header)?;
        let mut cursor = unit.entries();
        while let Some((_, entry)) = cursor.next_dfs()? {
            if entry.tag() != gimli::DW_TAG_subprogram {
                continue;
            }
            if let Ok(Some(name)) = entry.attr(gimli::DW_AT_name) {
                if let Some(name) = name.string_value(&dwarf.debug_str) {
                    if let Ok(name) = name.to_string() {
                        if name == name_to_find {
                            return get_function_meta(entry, &dwarf);
                        }
                    }
                }
            }
        }
    }
    Err(DebugError::FunctionNotFound)
}

pub fn get_addr_from_line<T: gimli::Reader>(
    dwarf: &gimli::Dwarf<T>,
    line_to_find: u64,
    file_to_search: String,
) -> Result<u64, DebugError> {
    let mut units = dwarf.units();
    while let Ok(Some(unit_header)) = units.next() {
        if let Ok(unit) = dwarf.unit(unit_header) {
            if let Some(line_program) = unit.line_program {
                let mut rows = line_program.rows();
                while let Ok(Some((header, row))) = rows.next_row() {
                    if let Some(file) = row.file(header) {
                        if let Some(filename) = file.path_name().string_value(&dwarf.debug_str) {
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

pub fn get_line_from_pc<T: Reader>(
    dwarf: &gimli::Dwarf<T>,
    pc: u64,
) -> Result<Location, DebugError> {
    let mut units = dwarf.units();
    while let Ok(Some(unit_header)) = units.next() {
        if let Ok(unit) = dwarf.unit(unit_header) {
            if let Some(line_program) = unit.line_program {
                let mut rows = line_program.rows();
                while let Ok(Some((header, row))) = rows.next_row() {
                    if row.address() == pc {
                        if let Some(file) = row.file(header) {
                            if let Some(filename) = file.path_name().string_value(&dwarf.debug_str)
                            {
                                if let Ok(filename) = filename.to_string() {
                                    return Ok(Location {
                                        line: match row.line() {
                                            Some(l) => l.into(),
                                            None => 0,
                                        },
                                        file: filename.to_string(),
                                        column: match row.column() {
                                            gimli::ColumnType::LeftEdge => 0,
                                            gimli::ColumnType::Column(c) => c.into(),
                                        },
                                    });
                                }
                            }
                        }
                        return Ok(Location {
                            line: match row.line() {
                                Some(l) => l.into(),
                                None => 0,
                            },
                            file: String::new(),
                            column: match row.column() {
                                gimli::ColumnType::LeftEdge => 0,
                                gimli::ColumnType::Column(c) => c.into(),
                            },
                        });
                    }
                }
            }
        }
    }
    Err(DebugError::NoSourceUnitFoundForCurrentPC)
}
