use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, Registers};
use url::Url;

use crate::debugger_window::DebuggerWindowImpl;

pub struct RegisterWindow {
    backend_url: Url,
    registers: Promise<Result<Registers, String>>,
}

impl RegisterWindow {
    pub fn new(backend_url: Url) -> Self {
        let mut ret = Self {
            backend_url,
            registers: Promise::from_ready(Err(String::new())),
        };
        ret.dirty();
        ret
    }
}

macro_rules! register_label {
    ($ui:expr, $reg_nam:expr, $reg:expr) => {
        $ui.label(format!("{}: {:#x} ({})", $reg_nam, $reg, $reg))
    };
}

impl DebuggerWindowImpl for RegisterWindow {
    fn dirty(&mut self) {
        self.registers = dispatch!(self.backend_url.clone(), Command::GetRegister, Registers);
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        match self.registers.ready() {
            Some(registers) => match registers {
                Ok(registers) => {
                    register_label!(ui, "Stack Pointer", registers.stack_pointer);
                    register_label!(ui, "Base Pointer", registers.base_pointer);
                    register_label!(ui, "Instruction Pointer", registers.instruction_pointer)
                }
                Err(e) => ui.label(format!("Err: {}", e)),
            },
            None => ui.spinner(),
        };
        false
    }
}
