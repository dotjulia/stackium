use super::{error::DebugError, Debugger};
use nix::{libc::user_regs_struct, sys::ptrace};
use stackium_shared::Registers;

impl Debugger {
    #[cfg(target_arch = "aarch64")]
    pub fn get_register_from_abi(&self, reg: u16) -> Result<u64, DebugError> {
        let registers = self.get_registers()?;
        if reg == 31 {
            return Ok(registers.sp);
        }
        Ok(registers.regs[reg as usize])
    }
    #[cfg(target_arch = "x86_64")]
    pub fn get_register_from_abi(&self, reg: u16) -> Result<u64, DebugError> {
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

    pub fn get_registers(&self) -> Result<user_regs_struct, DebugError> {
        match ptrace::getregs(self.child) {
            Ok(r) => Ok(r),
            Err(e) => Err(DebugError::NixError(e)),
        }
    }
    pub fn set_registers(&self, reg: user_regs_struct) -> Result<(), DebugError> {
        match ptrace::setregs(self.child, reg) {
            Ok(_) => Ok(()),
            Err(e) => Err(DebugError::NixError(e)),
        }
    }
}

pub trait FromUserRegsStruct {
    fn from_regs(value: user_regs_struct) -> Registers;
}

impl FromUserRegsStruct for Registers {
    #[cfg(target_arch = "x86_64")]
    fn from_regs(value: user_regs_struct) -> Self {
        Registers {
            base_pointer: value.rbp,
            stack_pointer: value.rsp,
            instruction_pointer: value.rip,
        }
    }
    #[cfg(target_arch = "aarch64")]
    fn from_regs(value: user_regs_struct) -> Self {
        Registers {
            base_pointer: value.regs[29],
            stack_pointer: value.sp,
            instruction_pointer: value.pc,
        }
    }
}
