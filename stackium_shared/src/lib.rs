//! Common types used by the debugger, the web API and the UI
//! This crate is used by the debugger, the web API and the UI to communicate with each other

use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Registers {
    pub instruction_pointer: u64,
    pub base_pointer: u64,
    pub stack_pointer: u64,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryMap {
    pub from: u64,
    pub to: u64,
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub shared: bool,
    pub offset: u64,
    pub mapped: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub enum CommandOutput {
    Data(u64),
    Memory(Vec<u8>),
    Variables(Vec<Variable>),
    DiscoveredVariables(Vec<DiscoveredVariable>),
    FunctionMeta(FunctionMeta),
    CodeWindow(Vec<(u64, String, bool)>),
    Registers(Registers),
    DebugMeta(DebugMeta),
    Location(Location),
    DwarfAttributes(Vec<DwarfAttribute>),
    Help(Vec<String>),
    Breakpoints(Vec<Breakpoint>),
    Functions(Vec<FunctionMeta>),
    File(String),
    Backtrace(Vec<FunctionMeta>),
    Maps(Vec<MemoryMap>),
    None,
}

// (internal offset, type)
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema, Clone)]
pub struct DataType(pub Vec<(usize, TypeName)>);

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema, Clone)]
pub enum TypeName {
    /// Name, Byte Size
    Name {
        name: String,
        byte_size: usize,
    },
    /// ArrType, Count
    Arr {
        arr_type: usize,
        count: Vec<usize>,
    },
    Ref {
        index: Option<usize>,
    },
    /// Name (Name, Type, offset), Byte Size
    ProductType {
        name: String,
        members: Vec<(String, usize, usize)>,
        byte_size: usize,
    },
}

impl ToString for DataType {
    fn to_string(&self) -> String {
        self.0
            .iter()
            .fold(String::new(), |acc, t| acc + &t.1.to_string())
    }
}

impl ToString for TypeName {
    fn to_string(&self) -> String {
        match self {
            TypeName::Name { name, byte_size: _ } => name.clone(),
            TypeName::Ref { index } => format!(
                "{}*",
                if let Some(index) = index {
                    index.to_string()
                } else {
                    "void".to_owned()
                }
            ),
            TypeName::Arr {
                arr_type,
                count: length,
            } => format!(
                "{}[{}]",
                arr_type.to_string(),
                length
                    .iter()
                    .map(|i| format!("[{}]", i))
                    .collect::<Vec<String>>()
                    .join("")
            ),
            TypeName::ProductType {
                name,
                members: _prod,
                byte_size: _,
            } => name.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DebugMeta {
    pub binary_name: String,
    pub file_type: String,
    pub files: Vec<String>,
    pub functions: i32,
    pub vars: i32,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema, Clone)]
pub struct Location {
    pub line: u64,
    pub file: String,
    pub column: u64,
}

#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema, Clone)]
pub struct Variable {
    pub name: Option<String>,
    pub type_name: Option<DataType>,
    pub value: Option<u64>,
    pub file: Option<String>,
    pub line: Option<u64>,
    pub addr: Option<u64>,
    pub high_pc: u64,
    pub low_pc: u64,
}

pub const VARIABLE_MEM_PADDING: u64 = 30;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema, Clone)]
pub struct DiscoveredVariable {
    pub name: Option<String>,
    pub types: DataType,
    pub type_index: usize,
    pub file: Option<String>,
    pub line: Option<u64>,
    pub addr: Option<u64>,
    pub memory: Option<Vec<u8>>,
    pub high_pc: u64,
    pub low_pc: u64,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DwarfAttribute {
    pub name: String,
    pub addr: u64,
    pub tag: String,
    pub attrs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, schemars::JsonSchema)]
pub struct FunctionMeta {
    pub name: Option<String>,
    pub low_pc: Option<u64>,
    pub high_pc: Option<u64>,
    pub return_addr: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Breakpoint {
    pub address: u64,
    pub original_byte: u32,
    pub enabled: bool,
    pub location: Location,
}

/// Specifies a location for a breakpoint
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
pub enum BreakpointPoint {
    /// At the start of the specified function
    Name(String),
    /// At the specified address
    Address(u64),
    /// At the specified location (ignores column)
    Location(Location),
}

/// A command for the debugger to execute
/// When using the web API take a look at the request JSON schema at the `/schema` endpoint
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(tag = "Command", content = "Argument")]
pub enum Command {
    /// Resumes the execution of the child
    Continue,
    /// Quits the debugger
    Quit,
    /// Returns all registers with their current value
    GetRegister,
    /// Steps the child by one instruction
    StepInstruction,
    /// Finds a function with the specified name
    FindFunc(String),
    /// Read from the specified address
    Read(u64),
    /// Read memory specifying the address and the length in bytes
    ReadMemory(u64, u64),
    /// Returns the address of the current instruction
    ProgramCounter,
    /// Provides statistics of the current program
    DebugMeta,
    /// Dumps all dwarf debug information; useful for debugging
    DumpDwarf,
    /// Retrieves the current location in the source code
    Location,
    /// Find the address of a line in the source code
    FindLine {
        line: u64,
        filename: String,
    },
    /// Step over the current function call by continuing execution until another line in the current function is reached
    StepOut,
    /// Continue execution until a new line in the source code is reached
    StepIn,
    /// View the source code around the current location
    ViewSource(usize),
    /// Get the current backtrace
    Backtrace,
    /// For debugging purposes
    WaitPid,
    /// Read all variables found in the debug symbols
    #[deprecated(note = "Use DiscoverVariables instead")]
    ReadVariables,
    /// Discovers variables, returns all variables from ReadVariables and additionally variables on
    /// the heap
    DiscoverVariables,
    /// Restarts the process being debugged
    RestartDebugee,
    /// Set a breakpoints at the specified location
    SetBreakpoint(BreakpointPoint),
    /// Retrieve all current breakpoints
    GetBreakpoints,
    /// Deletes the breakpoint at the specified address
    DeleteBreakpoint(u64),
    /// Retrieve a list of all functions
    GetFunctions,
    /// Get source file
    GetFile(String),
    /// Get the disassembly of the binary using objdump
    Disassemble,
    /// For the CLI implementation
    Help,
    Maps,
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.split(" ").into_iter();
        match iter.next().ok_or("empty command".to_string())? {
            "get_functions" => Ok(Command::GetFunctions),
            "location" => Ok(Command::Location),
            "continue" => Ok(Command::Continue),
            "maps" => Ok(Command::Maps),
            "waitpid" => Ok(Command::WaitPid),
            "disassemble" => Ok(Command::Disassemble),
            "get_breakpoints" => Ok(Command::GetBreakpoints),
            "quit" => Ok(Command::Quit),
            "get_registers" => Ok(Command::GetRegister),
            "step_instruction" => Ok(Command::StepInstruction),
            "pc" => Ok(Command::ProgramCounter),
            "dump_dwarf" => Ok(Command::DumpDwarf),
            "backtrace" => Ok(Command::Backtrace),
            "step_in" => Ok(Command::StepIn),
            "read_variables" => Ok(Command::ReadVariables),
            "discover_variables" => Ok(Command::DiscoverVariables),
            "debug_meta" => Ok(Command::DebugMeta),
            "read" => Ok(Command::Read(
                u64::from_str_radix(
                    iter.next()
                        .ok_or(format!("read requires argument \"{}\"", s))?
                        .trim_start_matches("0x"),
                    16,
                )
                .map_err(|a| a.to_string())?,
            )),
            "help" => Ok(Command::Help),
            "find_line" => Ok(Command::FindLine {
                line: iter
                    .next()
                    .ok_or(format!("find_line requires 1st argument line \"{}\"", s))?
                    .parse::<u64>()
                    .map_err(|a| a.to_string())?,
                filename: iter
                    .next()
                    .ok_or(format!("find_line requires 2nd argument file \"{}\"", s))?
                    .to_string(),
            }),
            "find_func" => Ok(Command::FindFunc(
                iter.next()
                    .ok_or(format!("find_func requires argument \"{}\"", s))?
                    .to_string(),
            )),
            "step_out" => Ok(Command::StepOut),
            "src" => Ok(Command::ViewSource(
                iter.next()
                    .ok_or(format!("src requires argument \"{}\"", s))?
                    .parse::<usize>()
                    .map_err(|a| a.to_string())?,
            )),
            "set_breakpoint" => Ok(Command::SetBreakpoint(
                match u64::from_str_radix(
                    iter.clone()
                        .next()
                        .ok_or(format!("set_breakpoint requires argument \"{}\"", s))?
                        .trim_start_matches("0x"),
                    16,
                ) {
                    Ok(a) => BreakpointPoint::Address(a),
                    Err(_) => BreakpointPoint::Name(
                        iter.next()
                            .ok_or(format!("set_breakpoint requires argument \"{}\"", s))?
                            .to_string(),
                    ),
                },
            )),
            _ => Err("Unknown command".to_string()),
        }
    }
}
