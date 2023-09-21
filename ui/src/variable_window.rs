use egui::{Color32, FontId, Pos2, RichText, ScrollArea, Stroke, Vec2};
use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, MemoryMap, Registers, TypeName, Variable};
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
    hover_text: Option<String>,
    additional_loaded_sections: Vec<(u64, Vec<u8>)>,
    mapping: Promise<Result<Vec<MemoryMap>, String>>,
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

fn get_y_from_addr(
    rect: &egui::Rect,
    stack_ptr: u64,
    rsp_offset: u64,
    heightpad: f32,
    addr: u64,
) -> f32 {
    return rect.max.y
        - ((addr as i64 - (stack_ptr - rsp_offset) as i64) as f32 * heightpad as f32
            + heightpad as f32);
}
fn render_ref_arrow(
    ui: &egui::Ui,
    rect: &egui::Rect,
    draw_ref_count: &mut i32,
    color: Color32,
    from: f32,
    to: f32,
) {
    // Horizontal line to vert
    ui.painter().line_segment(
        [
            Pos2::new(rect.max.x - 10.0 - *draw_ref_count as f32 * 15.0, from),
            Pos2::new(rect.min.x + 20.0, from),
        ],
        Stroke { width: 3.0, color },
    );
    // Vertical Line
    ui.painter().line_segment(
        [
            Pos2::new(rect.max.x - 10.0 - *draw_ref_count as f32 * 15.0, from),
            Pos2::new(rect.max.x - 10.0 - *draw_ref_count as f32 * 15.0, to),
        ],
        Stroke { width: 3.0, color },
    );
    // arrow back
    arrow_tip_length(
        ui.painter(),
        Pos2::new(rect.max.x - 10.0 - *draw_ref_count as f32 * 15.0, to),
        Vec2::new(-rect.width() + 25.0 + *draw_ref_count as f32 * 15.0, 0.0),
        Stroke { width: 3.0, color },
        10.0,
    );
    *draw_ref_count += 1;
}
fn render_invalid_ptr_arrow(ui: &egui::Ui, rect: &egui::Rect, pos: f32, color: Color32) {
    // Horizontal line to vert
    ui.painter().line_segment(
        [
            Pos2::new(rect.max.x - 80.0, pos),
            Pos2::new(rect.min.x + 20.0, pos),
        ],
        Stroke { width: 3.0, color },
    );
    ui.painter().text(
        Pos2::new(rect.max.x - 70.0, pos),
        egui::Align2::LEFT_CENTER,
        "?",
        FontId {
            size: 24.0,
            family: egui::FontFamily::Monospace,
        },
        color,
    );
}
fn render_var_line(
    ui: &egui::Ui,
    rect: &egui::Rect,
    offset: f32,
    top: f32,
    bottom: f32,
    name: &str,
    color: Color32,
    inline: bool,
) {
    ui.painter().line_segment(
        [
            Pos2::new(rect.min.x + offset, bottom),
            Pos2::new(rect.min.x + offset, top),
        ],
        Stroke {
            width: if inline { 18.0 } else { 10.0 },
            color,
        },
    );
    if inline {
        let galley = ui.painter().layout(
            name.to_string(),
            FontId {
                size: 15.0,
                family: egui::FontFamily::Monospace,
            },
            egui::Color32::WHITE,
            bottom - top,
        );
        let pos = Pos2::new(rect.min.x + offset - 8.0, bottom - 5.0);
        ui.painter().add(egui::Shape::Text(egui::epaint::TextShape {
            pos,
            galley,
            underline: egui::Stroke::NONE,
            override_text_color: None,
            angle: -std::f32::consts::PI / 2.0,
        }));
    } else {
        ui.painter().text(
            Pos2::new(rect.min.x + 15.0 + offset, top + (bottom - top) / 2.0),
            egui::Align2::LEFT_CENTER,
            name,
            FontId {
                size: 10.0,
                family: egui::FontFamily::Monospace,
            },
            color,
        );
    }
}
fn get_byte_size(typename: &TypeName) -> usize {
    match typename {
        TypeName::Name { name: _, byte_size } => *byte_size,
        TypeName::Arr { arr_type, count } => count * get_byte_size(arr_type),
        TypeName::Ref(_) => 8usize,
        TypeName::ProductType {
            name: _,
            members: _,
            byte_size,
        } => *byte_size,
    }
}
fn render_variable(
    ui: &egui::Ui,
    rect: &egui::Rect,
    registers: &Registers,
    rsp_offset: u64,
    heightpad: f32,
    height: f32,
    color: Color32,
    draw_ref_count: &mut i32,
    var: &Variable,
    offset: f32,
    stack: &Vec<u8>,
) {
    if let (Some(addr), Some(typename), Some(name)) = (var.addr, &var.type_name, &var.name) {
        match typename {
            TypeName::Name {
                name: typename,
                byte_size,
            } => {
                let top = get_y_from_addr(
                    rect,
                    registers.rsp,
                    rsp_offset,
                    heightpad,
                    addr + *byte_size as u64 - 1,
                ) + 2.0;
                let bottom = get_y_from_addr(rect, registers.rsp, rsp_offset, heightpad, addr)
                    + height
                    - 2.0;
                render_var_line(
                    ui,
                    &rect,
                    offset,
                    top,
                    bottom,
                    &format!("{}: {}", name, typename),
                    color,
                    false,
                );
            }
            TypeName::Arr { arr_type, count } => {
                let byte_size = get_byte_size(arr_type.as_ref());

                let bottom = get_y_from_addr(rect, registers.rsp, rsp_offset, heightpad, addr)
                    + height
                    - 2.0;

                let top = get_y_from_addr(
                    rect,
                    registers.rsp,
                    rsp_offset,
                    heightpad,
                    addr + byte_size as u64 * *count as u64 - 1,
                ) + 2.0;
                let offset = offset + 5.0;
                render_var_line(
                    ui,
                    &rect,
                    offset,
                    top,
                    bottom,
                    &format!("{}", name),
                    color,
                    true,
                );
                for i in 0..*count {
                    let addr = i as u64 * byte_size as u64 + addr;
                    render_variable(
                        ui,
                        rect,
                        registers,
                        rsp_offset,
                        heightpad,
                        height,
                        color,
                        draw_ref_count,
                        &Variable {
                            name: Some(format!("{}[{}]", name, i)),
                            type_name: Some(*arr_type.clone()),
                            value: None,
                            file: var.file.clone(),
                            line: var.line.clone(),
                            addr: Some(addr),
                            high_pc: var.high_pc,
                            low_pc: var.low_pc,
                        },
                        offset + 20.0,
                        stack,
                    );
                }
            }
            TypeName::Ref(typename) => {
                let bottom = get_y_from_addr(rect, registers.rsp, rsp_offset, heightpad, addr)
                    + height
                    - 2.0;
                let top =
                    get_y_from_addr(rect, registers.rsp, rsp_offset, heightpad, addr + 8 - 1) + 2.0;
                // if let Some(value) = var.value {
                let index = addr as usize - (registers.rsp - rsp_offset) as usize;
                let value = &stack[index..index + 8];
                let value = value[0] as u64
                    | (value[1] as u64) << 8
                    | (value[2] as u64) << 16
                    | (value[3] as u64) << 24
                    | (value[4] as u64) << 32
                    | (value[5] as u64) << 40
                    | (value[6] as u64) << 48
                    | (value[7] as u64) << 56;
                if value >= registers.rsp - rsp_offset && value <= registers.rbp + 16 {
                    render_ref_arrow(
                        ui,
                        &rect,
                        draw_ref_count,
                        color,
                        top + (bottom - top) / 2.0 - 10.0,
                        get_y_from_addr(rect, registers.rsp, rsp_offset, heightpad, value),
                    );
                } else {
                    render_invalid_ptr_arrow(ui, rect, top + (bottom - top) / 2.0 - 10.0, color)
                }
                render_var_line(
                    ui,
                    &rect,
                    offset,
                    top,
                    bottom,
                    &format!("{}: {}", name, typename.to_string()),
                    color,
                    false,
                );
            }
            TypeName::ProductType {
                name: typename,
                members,
                byte_size,
            } => {
                let bottom = get_y_from_addr(rect, registers.rsp, rsp_offset, heightpad, addr)
                    + height
                    - 2.0;
                let top = get_y_from_addr(
                    rect,
                    registers.rsp,
                    rsp_offset,
                    heightpad,
                    addr + *byte_size as u64 - 1,
                ) + 2.0;
                let offset = offset + 5.0;
                render_var_line(
                    ui,
                    &rect,
                    offset,
                    top,
                    bottom,
                    &format!("{}", name),
                    color,
                    true,
                );
                for (name, membertype, offset_byte) in members {
                    let addr = addr + *offset_byte as u64;
                    render_variable(
                        ui,
                        rect,
                        registers,
                        rsp_offset,
                        heightpad,
                        height,
                        color,
                        draw_ref_count,
                        &Variable {
                            name: Some(name.clone()),
                            type_name: Some(membertype.clone()),
                            value: None,
                            file: var.file.clone(),
                            line: var.line.clone(),
                            addr: Some(addr),
                            high_pc: var.high_pc,
                            low_pc: var.low_pc,
                        },
                        offset + 20.0,
                        stack,
                    );
                }
            }
        }
    }
}

impl VariableWindow {
    pub fn new(backend_url: Url) -> Self {
        let mut s = Self {
            variables: Promise::from_ready(Err(String::new())),
            backend_url,
            active_tab: ActiveTab::VariableList,
            registers: Promise::from_ready(Err(String::new())),
            stack: None,
            hover_text: None,
            additional_loaded_sections: vec![],
            mapping: Promise::from_ready(Err(String::new())),
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
                                            ui.add(
                                                egui::Label::new(format!(
                                                    "{}: {}",
                                                    variable
                                                        .name
                                                        .clone()
                                                        .unwrap_or("unknown".to_owned()),
                                                    variable
                                                        .type_name
                                                        .clone()
                                                        .unwrap_or(
                                                            stackium_shared::TypeName::Name {
                                                                name: "??".to_owned(),
                                                                byte_size: 0
                                                            }
                                                        )
                                                        .to_string()
                                                ))
                                                .wrap(false),
                                            );
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
                        ScrollArea::vertical().max_height(900.0).show(ui, |ui| {
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
                                let (rect, response) = ui.allocate_exact_size(
                                    Vec2::new(200.0, heightpad + heightpad * stack.len() as f32),
                                    egui::Sense::hover(),
                                );
                                if let Some(hover_text) = &self.hover_text {
                                    response.on_hover_text_at_pointer(hover_text);
                                }

                                // ui.painter().rect_filled(rect, 0.0, egui::Color32::WHITE);
                                let colors = [
                                    Color32::DARK_RED,
                                    Color32::from_rgb(169, 158, 0),
                                    Color32::DARK_GREEN,
                                    Color32::DARK_BLUE,
                                ];
                                let mut draw_ref_count = 0;

                                if let Some(Ok(vars)) = self.variables.ready() {
                                    let vars: Vec<Variable> = vars
                                        .iter()
                                        .filter(|v| {
                                            v.low_pc <= registers.rip && v.high_pc >= registers.rip
                                        })
                                        .map(|v| v.clone())
                                        .collect();
                                    for (ivar, var) in vars
                                        .iter()
                                        .chain(
                                            [
                                                Variable {
                                                    name: Some("Return Address".to_owned()),
                                                    type_name: Some(
                                                        stackium_shared::TypeName::Ref(Box::from(
                                                            stackium_shared::TypeName::Name {
                                                                name: "void".to_owned(),
                                                                byte_size: 0,
                                                            },
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
                                                            stackium_shared::TypeName::Name {
                                                                name: "void".to_owned(),
                                                                byte_size: 0,
                                                            },
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
                                        render_variable(
                                            ui,
                                            &rect,
                                            registers,
                                            rsp_offset,
                                            heightpad,
                                            height,
                                            colors[ivar % colors.len()],
                                            &mut draw_ref_count,
                                            var,
                                            0f32,
                                            stack,
                                        );
                                        if let (Some(typename), Some(addr)) =
                                            (&var.type_name, var.addr)
                                        {
                                            let size = get_byte_size(typename);
                                            let bottom = get_y_from_addr(
                                                &rect,
                                                registers.rsp,
                                                rsp_offset,
                                                heightpad,
                                                addr,
                                            );

                                            let top = get_y_from_addr(
                                                &rect,
                                                registers.rsp,
                                                rsp_offset,
                                                heightpad,
                                                addr + size as u64,
                                            );
                                            if ui.rect_contains_pointer(
                                                egui::Rect::from_x_y_ranges(
                                                    0f32..=100000f32,
                                                    top..=bottom,
                                                ),
                                            ) {
                                                self.hover_text = Some(typename.to_string());
                                            }
                                        }
                                    }
                                    ui.painter().arrow(
                                        Pos2::new(
                                            rect.min.x + 8.0,
                                            get_y_from_addr(
                                                &rect,
                                                registers.rsp,
                                                rsp_offset,
                                                heightpad,
                                                registers.rsp,
                                            ) + height / 2.0,
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
                                            get_y_from_addr(
                                                &rect,
                                                registers.rsp,
                                                rsp_offset,
                                                heightpad,
                                                registers.rsp,
                                            ) + height / 2.0,
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
        self.additional_loaded_sections.clear();
        self.variables = dispatch!(self.backend_url.clone(), Command::ReadVariables, Variables);
        self.registers = dispatch!(self.backend_url.clone(), Command::GetRegister, Registers);
        self.mapping = dispatch!(self.backend_url.clone(), Command::Maps, Maps);
        self.stack = None
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.active_tab,
                ActiveTab::VariableList,
                "Variable List",
            );
            ui.selectable_value(&mut self.active_tab, ActiveTab::StackView, "Memory");
        });

        let res = match self.active_tab {
            ActiveTab::VariableList => self.render_variable_list(ui),
            ActiveTab::StackView => self.render_stack(ui),
        };
        (false, res)
    }
}
