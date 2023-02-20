use nix::{sys::ptrace, unistd::Pid};

use crate::debugger::DebugError;

pub struct Breakpoint {
    pub address: *const u8,
    original_byte: u8,
    enabled: bool,
}

impl Breakpoint {
    pub fn new(child: Pid, address: *const u8) -> Result<Self, DebugError> {
        Ok(Self {
            address,
            original_byte: match ptrace::read(child, address as *mut _) {
                Ok(b) => b as u8,
                Err(e) => {
                    println!("Error in ptrace::read: {}", e);
                    return Err(DebugError::NixError(e));
                }
            },
            enabled: false,
        })
    }

    fn replace_byte(&self, child: Pid, byte: u8) -> Result<(), DebugError> {
        let orig_data: u32 = match ptrace::read(child, self.address as *mut _) {
            Ok(b) => b as u32,
            Err(e) => return Err(DebugError::NixError(e)),
        };
        match ptrace::write(
            child,
            self.address as *mut _,
            ((byte as u32) | (orig_data & !0xff)) as i32,
        ) {
            Ok(_) => Ok(()),
            Err(e) => Err(DebugError::NixError(e)),
        }
    }

    pub fn enable(&mut self, child: Pid) -> Result<(), DebugError> {
        if self.enabled {
            return Err(DebugError::BreakpointInvalidState);
        }
        self.replace_byte(child, 0xcc)?;
        self.enabled = true;
        Ok(())
    }

    pub fn disable(&mut self, child: Pid) -> Result<(), DebugError> {
        if !self.enabled {
            return Err(DebugError::BreakpointInvalidState);
        }
        self.replace_byte(child, self.original_byte)?;
        self.enabled = false;
        Ok(())
    }
}
