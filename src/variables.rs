use std::os::raw::c_void;

use stackium_shared::{DataType, DiscoveredVariable, MemoryMap, TypeName, Variable, VARIABLE_MEM_PADDING};

use crate::debugger::{error::DebugError, Debugger};
pub fn get_byte_size(types: &DataType, index: usize) -> usize {
    match &types.0[index].1 {
        TypeName::Name { name: _, byte_size } => *byte_size,
        TypeName::Arr { arr_type, count } => {
            count.iter().cloned().fold(1, |e1, e2| e1 * e2) * get_byte_size(types, *arr_type)
        }
        TypeName::Ref { index: _ } => 8usize,
        TypeName::ProductType {
            name: _,
            members: _,
            byte_size,
        } => *byte_size,
    }
}
fn check_variable_recursive(
    debugger: &Debugger,
    mapping: &Vec<MemoryMap>,
    original_var: &DiscoveredVariable,
    addr: u64,
    type_index: usize,
    types: DataType,
    name: String,
    search_mode: bool,
) -> Vec<DiscoveredVariable> {
    let size = get_byte_size(&types, type_index);
    if mapping
        .iter()
        .any(|m| m.from <= addr && addr + size as u64 <= m.to)
    {
        match &types.0[type_index].1 {
            stackium_shared::TypeName::Name {
                name: _,
                byte_size: _,
            } => {
                if !search_mode {
                    // return vec![(addr, name, vec![], type_index, types.clone())];
                    return vec![DiscoveredVariable {
                        addr: Some(addr),
                        name: Some(name),
                        type_index: type_index,
                        types: types.clone(),
                        file: original_var.file.clone(),
                        line: original_var.line.clone(),
                        high_pc: original_var.high_pc,
                        low_pc: original_var.low_pc,
                        memory: None,
                    }];
                } else {
                    return vec![];
                }
            }
            stackium_shared::TypeName::Arr { arr_type, count } => {
                let mut ret_val = vec![];
                for i in 0..count.iter().fold(1, |acc, e| acc * *e) {
                    let mut a = check_variable_recursive(
                        debugger,
                        mapping,
                        original_var,
                        addr + get_byte_size(&types, *arr_type) as u64 * i as u64,
                        *arr_type,
                        types.clone(),
                        format!("{}[{}]", name, i),
                        true,
                    );
                    ret_val.append(&mut a);
                }
                if !search_mode {
                    ret_val.push(DiscoveredVariable {
                        addr: Some(addr),
                        name: Some(name),
                        type_index,
                        types: types.clone(),
                        file: original_var.file.clone(),
                        line: original_var.line.clone(),
                        high_pc: original_var.high_pc,
                        low_pc: original_var.low_pc,
                        memory: None,
                    });
                }
                return ret_val;
            }
            stackium_shared::TypeName::Ref { index } => {
                let mut ret_val = vec![];
                // let value = read_value(memory, addr as usize - section.0 as usize);
                let value = debugger.read(addr as *mut c_void);
                if let Ok(value) = value {
                    if !search_mode {
                        // ret_val.push((
                        //     addr,
                        //     name.clone(),
                        //     vec![Edge {
                        //         connection: value as usize,
                        //         label: String::new(),
                        //     }],
                        //     type_index,
                        //     types.clone(),
                        // ));
                        ret_val.push(DiscoveredVariable {
                            addr: Some(addr),
                            name: Some(name.clone()),
                            type_index,
                            types: types.clone(),
                            file: original_var.file.clone(),
                            line: original_var.line.clone(),
                            high_pc: original_var.high_pc,
                            low_pc: original_var.low_pc,
                            memory: None,
                        });
                    }
                    if let Some(index) = index {
                        // ret_val.append(&mut check_variable_recursive(
                        //     mapping,
                        //     sections,
                        //     backend_url,
                        //     value,
                        //     *index,
                        //     types,
                        //     format!("*{}", name),
                        //     false,
                        // ));
                        ret_val.append(&mut check_variable_recursive(
                            debugger,
                            mapping,
                            original_var,
                            value,
                            *index,
                            types,
                            format!("*{}", name),
                            false,
                        ));
                    }
                } else {
                    println!("Failed to read value at {:x}", addr);
                }
                return ret_val;
            }
            stackium_shared::TypeName::ProductType {
                name: structname,
                members,
                byte_size,
            } => {
                let mut ret_val = vec![];
                for (fieldname, prod_type_offset, offset) in members.iter() {
                    // let mut a = check_variable_recursive(
                    //     mapping,
                    //     sections,
                    //     backend_url,
                    //     addr + *offset as u64,
                    //     *prod_type_offset,
                    //     types.clone(),
                    //     format!("{}.{}", name, fieldname),
                    //     true,
                    // );
                    let mut a = check_variable_recursive(
                        debugger,
                        mapping,
                        original_var,
                        addr + *offset as u64,
                        *prod_type_offset,
                        types.clone(),
                        format!("{}.{}", name, fieldname),
                        true,
                    );
                    ret_val.append(&mut a);
                }
                if !search_mode {
                    // ret_val.push((addr, name, refs, type_index, types.clone()));
                    ret_val.push(DiscoveredVariable {
                        addr: Some(addr),
                        name: Some(name),
                        type_index,
                        types: types.clone(),
                        file: original_var.file.clone(),
                        line: original_var.line.clone(),
                        high_pc: original_var.high_pc,
                        low_pc: original_var.low_pc,
                        memory: None,
                    });
                }
                return ret_val;
            }
        }
    } else {
        vec![]
    }
}
impl Debugger {
    pub fn discover_variables(&self) -> Result<Vec<DiscoveredVariable>, DebugError> {
        let scope_variables = self.read_variables()?;
        let mut variables = vec![];
        let mapping = self.get_maps()?;
        for scope_variable in scope_variables {
            let mut scope_variables = check_variable_recursive(
                &self,
                &mapping,
                &DiscoveredVariable {
                    addr: scope_variable.addr,
                    name: scope_variable.name.clone(),
                    type_index: 0,
                    types: scope_variable.type_name.clone().unwrap(),
                    file: scope_variable.file.clone(),
                    line: scope_variable.line.clone(),
                    high_pc: scope_variable.high_pc,
                    low_pc: scope_variable.low_pc,
                    memory: None,
                },
                scope_variable.addr.unwrap(),
                0,
                scope_variable.type_name.clone().unwrap(),
                scope_variable.name.clone().unwrap_or("unknown".to_string()),
                false,
            );
            variables.append(&mut scope_variables);
        }
        for variable in &mut variables {
            variable.memory = self.read_memory(variable.addr.unwrap() - VARIABLE_MEM_PADDING, get_byte_size(&variable.types, variable.type_index) as u64 + VARIABLE_MEM_PADDING * 2).ok();
        }
        Ok(variables)
    }
}
