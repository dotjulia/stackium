use std::ffi::c_void;

use nix::{sys::ptrace, unistd::Pid};
use stackium_shared::Breakpoint;

use super::{error::DebugError, util::get_line_from_pc};

pub trait DebuggerBreakpoint {
    fn new<T: gimli::Reader>(
        dwarf: &gimli::Dwarf<T>,
        child: Pid,
        address: *const u8,
    ) -> Result<Breakpoint, DebugError>;
    fn replace_byte(&self, child: Pid, byte: u8) -> Result<(), DebugError>;
    fn enable(&mut self, child: Pid) -> Result<(), DebugError>;
    fn disable(&mut self, child: Pid) -> Result<(), DebugError>;
}

impl DebuggerBreakpoint for Breakpoint {
    fn new<T: gimli::Reader>(
        dwarf: &gimli::Dwarf<T>,
        child: Pid,
        address: *const u8,
    ) -> Result<Self, DebugError> {
        let location = get_line_from_pc(dwarf, address as u64)?;
        Ok(Self {
            address: address as u64,
            original_byte: match ptrace::read(child, address as *mut _) {
                Ok(b) => b as u8,
                Err(e) => {
                    println!("Error in ptrace::read: {} {:?} {:?}", e, child, address);
                    return Err(DebugError::NixError(e));
                }
            },
            enabled: false,
            location,
        })
    }

    fn replace_byte(&self, child: Pid, byte: u8) -> Result<(), DebugError> {
        let orig_data: u64 = match ptrace::read(child, self.address as *mut _) {
            Ok(b) => b as u64,
            Err(e) => return Err(DebugError::NixError(e)),
        };
        match unsafe {
            ptrace::write(
                child,
                self.address as *mut _,
                ((byte as u64) | (orig_data & !(0xff as u64))) as *mut c_void,
            )
        } {
            Ok(_) => Ok(()),
            Err(e) => Err(DebugError::NixError(e)),
        }
    }

    fn enable(&mut self, child: Pid) -> Result<(), DebugError> {
        if self.enabled {
            return Err(DebugError::BreakpointInvalidState);
        }
        self.replace_byte(child, 0xcc)?;
        self.enabled = true;
        Ok(())
    }

    fn disable(&mut self, child: Pid) -> Result<(), DebugError> {
        if !self.enabled {
            return Err(DebugError::BreakpointInvalidState);
        }
        self.replace_byte(child, self.original_byte)?;
        self.enabled = false;
        Ok(())
    }
}
