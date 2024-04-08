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
    fn replace_4_bytes(&self, child: Pid, bytes: u32) -> Result<(), DebugError>;
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
                Ok(b) => b as u32,
                Err(e) => {
                    println!("Error in ptrace::read: {} {:?} {:?}", e, child, address);
                    return Err(DebugError::NixError(e));
                }
            },
            enabled: false,
            location,
        })
    }

    #[cfg(target_arch = "x86_64")]
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

    #[cfg(target_arch = "aarch64")]
    fn replace_byte(&self, child: Pid, byte: u8) -> Result<(), DebugError> {
        let orig_data: u64 = match ptrace::read(child, self.address as *mut _) {
            Ok(b) => b as u64,
            Err(e) => return Err(DebugError::NixError(e)),
        };
        match unsafe {
            ptrace::write(
                child,
                self.address as *mut _,
                ((byte as u64) | (orig_data & !(0xff as u64))) as i64,
            )
        } {
            Ok(_) => Ok(()),
            Err(e) => Err(DebugError::NixError(e)),
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn replace_4_bytes(&self, child: Pid, bytes: u32) -> Result<(), DebugError> {
        let orig_data: u64 = match ptrace::read(child, self.address as *mut _) {
            Ok(b) => b as u64,
            Err(e) => return Err(DebugError::NixError(e)),
        };
        match unsafe {
            ptrace::write(
                child,
                self.address as *mut _,
                ((bytes as u64) | (orig_data & !(0xffffffff as u64))) as *mut c_void,
            )
        } {
            Ok(_) => Ok(()),
            Err(e) => Err(DebugError::NixError(e)),
        }
    }

    #[cfg(target_arch = "aarch64")]
    fn replace_4_bytes(&self, child: Pid, bytes: u32) -> Result<(), DebugError> {
        let orig_data: u64 = match ptrace::read(child, self.address as *mut _) {
            Ok(b) => b as u64,
            Err(e) => return Err(DebugError::NixError(e)),
        };
        match unsafe {
            ptrace::write(
                child,
                self.address as *mut _,
                ((bytes as u64) | (orig_data & !(0xffffffff as u64))) as i64,
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
        #[cfg(target_arch = "x86_64")]
        self.replace_byte(child, 0xcc)?;
        #[cfg(target_arch = "aarch64")]
        self.replace_4_bytes(child, 0xd4200020)?;
        // for arm64 0x200020D4
        self.enabled = true;
        Ok(())
    }

    fn disable(&mut self, child: Pid) -> Result<(), DebugError> {
        if !self.enabled {
            return Err(DebugError::BreakpointInvalidState);
        }
        #[cfg(target_arch = "x86_64")]
        self.replace_byte(child, self.original_byte as u8)?;
        #[cfg(target_arch = "aarch64")]
        self.replace_4_bytes(child, self.original_byte)?;
        self.enabled = false;
        Ok(())
    }
}
