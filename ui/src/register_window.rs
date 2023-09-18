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
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
        let response = match self.registers.ready() {
            Some(registers) => match registers {
                Ok(registers) => {
                    register_label!(ui, "RAX", registers.rax);
                    register_label!(ui, "RCX", registers.rcx);
                    register_label!(ui, "RDX", registers.rdx);
                    register_label!(ui, "RBX", registers.rbx);
                    register_label!(ui, "RSP", registers.rsp);
                    register_label!(ui, "RBP", registers.rbp);
                    register_label!(ui, "RSI", registers.rsi);
                    register_label!(ui, "RDI", registers.rdi)
                }
                Err(e) => ui.label(format!("Err: {}", e)),
            },
            None => ui.spinner(),
        };
        (false, response)
    }
}
