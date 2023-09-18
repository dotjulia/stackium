use egui::{Color32, FontId, Pos2, RichText, ScrollArea, Stroke, Vec2};
use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, Registers, TypeName, Variable};
use url::Url;

use crate::{command::dispatch_command_and_then, debugger_window::DebuggerWindowImpl};

#[derive(PartialEq)]
enum ActiveTab {
    VariableList,
    StackView,
}

pub struct VariableWindow {
    variables: Promise<Result<Vec<Variable>, String>>,
    backend_url: Url,
    active_tab: ActiveTab,
    registers: Promise<Result<Registers, String>>,
    stack: Option<Promise<Result<Vec<u8>, String>>>,
}

fn arrow_tip_length(
    painter: &egui::Painter,
    origin: Pos2,
    vec: Vec2,
    stroke: Stroke,
    tip_length: f32,
) {
    use egui::emath::*;
    let rot = Rot2::from_angle(std::f32::consts::TAU / 10.0);
    let tip = origin + vec;
    let dir = vec.normalized();
    painter.line_segment([origin, tip], stroke);
    painter.line_segment([tip, tip - tip_length * (rot * dir)], stroke);
    painter.line_segment([tip, tip - tip_length * (rot.inverse() * dir)], stroke);
}

impl VariableWindow {
    pub fn new(backend_url: Url) -> Self {
        let mut s = Self {
            variables: Promise::from_ready(Err(String::new())),
            backend_url,
            active_tab: ActiveTab::VariableList,
            registers: Promise::from_ready(Err(String::new())),
            stack: None,
        };
        s.dirty();
        s
    }

    fn render_variable_list(&mut self, ui: &mut egui::Ui) -> egui::Response {
        match self.variables.ready() {
            Some(variables) => match variables {
                Ok(variables) => {
                    egui_extras::TableBuilder::new(ui)
                        .striped(true)
                        .column(egui_extras::Column::auto().at_least(80.).resizable(true))
                        .column(egui_extras::Column::auto().at_least(100.).resizable(true))
                        .column(egui_extras::Column::remainder())
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.heading("Name");
                            });
                            header.col(|ui| {
                                ui.heading("Address");
                            });
                            header.col(|ui| {
                                ui.heading("Content");
                            });
                        })
                        .body(|mut body| {
                            let mut sorted_variables = variables.clone();

                            sorted_variables
                                .sort_by(|b, a| a.addr.unwrap_or(0).cmp(&b.addr.unwrap_or(0)));
                            for variable in sorted_variables.iter() {
                                if let (Some(address), Some(value)) =
                                    (variable.addr, variable.value)
                                {
                                    body.row(20.0, |mut row| {
                                        row.col(|ui| {
                                            ui.label(format!(
                                                "{}: {}",
                                                variable
                                                    .name
                                                    .clone()
                                                    .unwrap_or("unknown".to_owned()),
                                                variable
                                                    .type_name
                                                    .clone()
                                                    .unwrap_or(stackium_shared::TypeName::Name(
                                                        "??".to_owned()
                                                    ))
                                                    .to_string()
                                            ));
                                        });
                                        row.col(|ui| {
                                            ui.label(format!("{:#x}", address));
                                        });
                                        row.col(|ui| {
                                            ui.label(format!("{:#x}", value));
                                        });
                                    });
                                }
                            }
                        });
                    ui.separator()
                }
                Err(err) => ui.label(err),
            },
            None => ui.spinner(),
        }
    }

    fn render_stack(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let rsp_offset = 16u64;
        if let Some(Ok(registers)) = self.registers.ready() {
            match &self.stack {
                Some(s) => {
                    if let Some(Ok(stack)) = s.ready() {
                        //TODO: find a solution for the window height
                        ScrollArea::vertical().max_height(800.0).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let height = 15.0;
                                egui_extras::TableBuilder::new(ui)
                                    .vscroll(false)
                                    .column(egui_extras::Column::auto().at_least(130.0))
                                    .column(egui_extras::Column::auto().at_least(40.0))
                                    .body(|mut body| {
                                        body.row(height, |mut row| {
                                            row.col(|ui| {
                                                ui.label(
                                                    RichText::new("Address")
                                                        .color(ui.visuals().strong_text_color()),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    RichText::new("Byte")
                                                        .color(ui.visuals().strong_text_color()),
                                                );
                                            });
                                        });
                                        for (i, byte) in stack.iter().enumerate().rev() {
                                            body.row(height, |mut row| {
                                                row.col(|ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{:#x}",
                                                            (registers.rsp - rsp_offset) + i as u64
                                                        ))
                                                        .family(egui::FontFamily::Monospace),
                                                    );
                                                });
                                                row.col(|ui| {
                                                    ui.label(
                                                        RichText::new(format!("{:#04X}", byte))
                                                            .family(egui::FontFamily::Monospace),
                                                    );
                                                });
                                            })
                                        }
                                    });

                                let heightpad = height + 3.0;
                                let (rect, _) = ui.allocate_exact_size(
                                    Vec2::new(200.0, heightpad + heightpad * stack.len() as f32),
                                    egui::Sense::focusable_noninteractive(),
                                );
                                let get_y_from_addr = |addr: u64| -> f32 {
                                    return rect.max.y
                                        - ((addr as i64 - (registers.rsp - rsp_offset) as i64)
                                            as f32
                                            * heightpad as f32
                                            + heightpad as f32);
                                };
                                // ui.painter().rect_filled(rect, 0.0, egui::Color32::WHITE);
                                let colors =
                                    [Color32::RED, Color32::YELLOW, Color32::GREEN, Color32::BLUE];
                                if let Some(Ok(vars)) = self.variables.ready() {
                                    let vars: Vec<Variable> = vars
                                        .iter()
                                        .filter(|v| {
                                            v.low_pc <= registers.rip && v.high_pc >= registers.rip
                                        })
                                        .map(|v| v.clone())
                                        .collect();
                                    let mut draw_ref_count = 0;
                                    for (ivar, var) in vars
                                        .iter()
                                        .chain(
                                            [
                                                Variable {
                                                    name: Some("Return Address".to_owned()),
                                                    type_name: Some(
                                                        stackium_shared::TypeName::Ref(Box::from(
                                                            stackium_shared::TypeName::Name(
                                                                "void".to_owned(),
                                                            ),
                                                        )),
                                                    ),
                                                    value: None,
                                                    file: None,
                                                    line: None,
                                                    addr: Some(registers.rbp + 8),
                                                    high_pc: 0,
                                                    low_pc: 0,
                                                },
                                                Variable {
                                                    name: Some("Calling Base Pointer".to_owned()),
                                                    type_name: Some(
                                                        stackium_shared::TypeName::Ref(Box::from(
                                                            stackium_shared::TypeName::Name(
                                                                "void".to_owned(),
                                                            ),
                                                        )),
                                                    ),
                                                    value: None,
                                                    file: None,
                                                    line: None,
                                                    addr: Some(registers.rbp),
                                                    high_pc: 0,
                                                    low_pc: 0,
                                                },
                                            ]
                                            .iter(),
                                        )
                                        .enumerate()
                                    {
                                        if let (Some(addr), Some(typename), Some(name)) =
                                            (var.addr, &var.type_name, &var.name)
                                        {
                                            let bytesize = match typename {
                                                stackium_shared::TypeName::Name(typename) => {
                                                    match typename.as_str() {
                                                        x if x.contains("char") => 1,
                                                        "int" => 4,
                                                        "short" => 2,
                                                        "long" => 8,
                                                        _ => 1,
                                                    }
                                                }
                                                stackium_shared::TypeName::Ref(_) => 8,
                                            };
                                            let bottom = get_y_from_addr(addr) + height - 2.0;
                                            let top = get_y_from_addr(addr + bytesize - 1) + 2.0;
                                            if let (TypeName::Ref(_), Some(value)) =
                                                (typename, var.value)
                                            {
                                                // Horizontal line to vert
                                                ui.painter().line_segment(
                                                    [
                                                        Pos2::new(
                                                            rect.max.x
                                                                - 10.0
                                                                - draw_ref_count as f32 * 15.0,
                                                            top + (bottom - top) / 2.0 - 10.0,
                                                        ),
                                                        Pos2::new(
                                                            rect.min.x + 20.0,
                                                            top + (bottom - top) / 2.0 - 10.0,
                                                        ),
                                                    ],
                                                    Stroke {
                                                        width: 3.0,
                                                        color: colors[ivar % colors.len()],
                                                    },
                                                );
                                                if value >= registers.rsp - rsp_offset
                                                    && value <= registers.rbp + 16
                                                {
                                                    // Vertical Line
                                                    ui.painter().line_segment(
                                                        [
                                                            Pos2::new(
                                                                rect.max.x
                                                                    - 10.0
                                                                    - draw_ref_count as f32 * 15.0,
                                                                top + (bottom - top) / 2.0 - 10.0,
                                                            ),
                                                            Pos2::new(
                                                                rect.max.x
                                                                    - 10.0
                                                                    - draw_ref_count as f32 * 15.0,
                                                                get_y_from_addr(value),
                                                            ),
                                                        ],
                                                        Stroke {
                                                            width: 3.0,
                                                            color: colors[ivar % colors.len()],
                                                        },
                                                    );
                                                    // arrow back
                                                    arrow_tip_length(
                                                        ui.painter(),
                                                        Pos2::new(
                                                            rect.max.x
                                                                - 10.0
                                                                - draw_ref_count as f32 * 15.0,
                                                            get_y_from_addr(value),
                                                        ),
                                                        Vec2::new(
                                                            -rect.width()
                                                                + 25.0
                                                                + draw_ref_count as f32 * 15.0,
                                                            0.0,
                                                        ),
                                                        Stroke {
                                                            width: 3.0,
                                                            color: colors[ivar % colors.len()],
                                                        },
                                                        10.0,
                                                    );
                                                    draw_ref_count += 1;
                                                }
                                            }
                                            ui.painter().line_segment(
                                                [
                                                    Pos2::new(rect.min.x, bottom),
                                                    Pos2::new(rect.min.x, top),
                                                ],
                                                Stroke {
                                                    width: 10.0,
                                                    color: colors[ivar % colors.len()],
                                                },
                                            );
                                            ui.painter().text(
                                                Pos2::new(
                                                    rect.min.x + 15.0,
                                                    top + (bottom - top) / 2.0,
                                                ),
                                                egui::Align2::LEFT_CENTER,
                                                format!("{}: {}", name, typename.to_string()),
                                                FontId {
                                                    size: 10.0,
                                                    family: egui::FontFamily::Monospace,
                                                },
                                                colors[ivar % colors.len()],
                                            );
                                        }
                                    }
                                    ui.painter().arrow(
                                        Pos2::new(
                                            rect.min.x + 8.0,
                                            get_y_from_addr(registers.rsp) + height / 2.0,
                                        ),
                                        Vec2::new(-15.0, 0.0),
                                        Stroke {
                                            width: 2.0,
                                            color: Color32::WHITE,
                                        },
                                    );
                                    ui.painter().text(
                                        Pos2::new(
                                            rect.min.x + 15.0,
                                            get_y_from_addr(registers.rsp) + height / 2.0,
                                        ),
                                        egui::Align2::LEFT_CENTER,
                                        "Stack Pointer",
                                        FontId {
                                            size: 10.0,
                                            family: egui::FontFamily::Monospace,
                                        },
                                        Color32::WHITE,
                                    );
                                }

                                // let mut cur_pos = rect.min;
                                // cur_pos.y += heightpad + height / 2.0;
                                // for (_, _) in stack.iter().enumerate().rev() {
                                //     ui.painter()
                                //         .circle_filled(cur_pos, 5.0, egui::Color32::BLACK);
                                //     cur_pos.y += heightpad;
                                // }
                            });
                        });
                    }
                }
                None => {
                    if registers.rbp >= registers.rsp {
                        self.stack = Some(dispatch_command_and_then(
                            self.backend_url.clone(),
                            Command::ReadMemory(
                                registers.rsp - rsp_offset,
                                (registers.rbp - registers.rsp) + 16 + rsp_offset,
                            ),
                            |out| match out {
                                CommandOutput::Memory(mem) => mem,
                                _ => unreachable!(),
                            },
                        ));
                    }
                }
            }
            ui.label("")
        } else {
            ui.spinner()
        }
    }
}

impl DebuggerWindowImpl for VariableWindow {
    fn dirty(&mut self) {
        self.variables = dispatch!(self.backend_url.clone(), Command::ReadVariables, Variables);
        self.registers = dispatch!(self.backend_url.clone(), Command::GetRegister, Registers);
        self.stack = None
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.active_tab,
                ActiveTab::VariableList,
                "Variable List",
            );
            ui.selectable_value(&mut self.active_tab, ActiveTab::StackView, "Stack");
        });

        let res = match self.active_tab {
            ActiveTab::VariableList => self.render_variable_list(ui),
            ActiveTab::StackView => self.render_stack(ui),
        };
        (false, res)
    }
}
