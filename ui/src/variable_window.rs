use egui::{Color32, FontId, Pos2, RichText, ScrollArea, Stroke, Vec2};
use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, DataType, MemoryMap, Registers, TypeName, Variable};
use url::Url;

use crate::{command::dispatch_command_and_then, debugger_window::DebuggerWindowImpl};

#[derive(PartialEq)]
enum ActiveTab {
    VariableList,
    StackView,
}

type Section = (u64, u64, String, Promise<Result<Vec<u8>, String>>);

pub struct VariableWindow {
    variables: Promise<Result<Vec<Variable>, String>>,
    backend_url: Url,
    active_tab: ActiveTab,
    registers: Promise<Result<Registers, String>>,
    stack: Option<Promise<Result<Vec<u8>, String>>>,
    hover_text: Option<String>,
    additional_loaded_sections: Vec<Section>,
    mapping: Promise<Result<Vec<MemoryMap>, String>>,
    lock_stack: bool,
    lock_stack_addr: u64,
    rsp_offset: u64,
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
    invert: bool,
    invert_length: f32,
    invert_origin: bool,
) {
    // Horizontal line to vert
    ui.painter().line_segment(
        [
            Pos2::new(rect.max.x - 10.0 - *draw_ref_count as f32 * 15.0, from),
            Pos2::new(
                if invert_origin {
                    rect.min.x + invert_length + 140.
                } else {
                    rect.min.x + 15.0
                },
                from,
            ),
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
        Vec2::new(
            if invert {
                invert_length + *draw_ref_count as f32 * 15.0
            } else {
                (rect.width() - 25.0) * -1f32 + *draw_ref_count as f32 * 15.0
            },
            0.0,
        ),
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
pub fn get_byte_size(types: &DataType, index: usize) -> usize {
    match &types.0[index].1 {
        TypeName::Name { name: _, byte_size } => *byte_size,
        TypeName::Arr { arr_type, count } => {
            count.iter().cloned().reduce(|e1, e2| e1 * e2).unwrap()
                * get_byte_size(types, *arr_type)
        }
        TypeName::Ref { index: _ } => 8usize,
        TypeName::ProductType {
            name: _,
            members: _,
            byte_size,
        } => *byte_size,
    }
}

fn read_value_stack(addr: u64, registers: &Registers, rsp_offset: u64, stack: &[u8]) -> u64 {
    if addr < registers.rsp - rsp_offset {
        return 0;
    }
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
    value
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
    render_variable_override(
        ui,
        rect,
        registers,
        rsp_offset,
        heightpad,
        height,
        color,
        draw_ref_count,
        var,
        offset,
        stack,
        0,
    )
}
fn render_variable_override(
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
    override_index: usize,
) {
    if let (Some(addr), Some(datatype), Some(name)) = (var.addr, &var.type_name, &var.name) {
        let orig_type = &datatype.0[override_index].1;
        match orig_type {
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
                let byte_size = get_byte_size(datatype, *arr_type);

                let bottom = get_y_from_addr(rect, registers.rsp, rsp_offset, heightpad, addr)
                    + height
                    - 2.0;
                let top = get_y_from_addr(
                    rect,
                    registers.rsp,
                    rsp_offset,
                    heightpad,
                    addr + byte_size as u64
                        * count.iter().cloned().reduce(|e1, e2| e1 * e2).unwrap() as u64
                        - 1,
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
                for i in 0..count.iter().cloned().reduce(|e1, e2| e1 * e2).unwrap() {
                    let addr = i as u64 * byte_size as u64 + addr;
                    render_variable_override(
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
                            type_name: Some(datatype.clone()),
                            value: None,
                            file: var.file.clone(),
                            line: var.line.clone(),
                            addr: Some(addr),
                            high_pc: var.high_pc,
                            low_pc: var.low_pc,
                        },
                        offset + 20.0,
                        stack,
                        *arr_type,
                    );
                }
            }
            TypeName::Ref { index } => {
                let bottom = get_y_from_addr(rect, registers.rsp, rsp_offset, heightpad, addr)
                    + height
                    - 2.0;
                let top =
                    get_y_from_addr(rect, registers.rsp, rsp_offset, heightpad, addr + 8 - 1) + 2.0;
                render_var_line(
                    ui,
                    &rect,
                    offset,
                    top,
                    bottom,
                    &format!("{}: {}", name, orig_type.to_string()),
                    color,
                    false,
                );
            }
            TypeName::ProductType {
                name: _typename,
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
                    render_variable_override(
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
                            type_name: Some(datatype.clone()),
                            value: None,
                            file: var.file.clone(),
                            line: var.line.clone(),
                            addr: Some(addr),
                            high_pc: var.high_pc,
                            low_pc: var.low_pc,
                        },
                        offset + 20.0,
                        stack,
                        *membertype,
                    );
                }
            }
        }
    }
}

fn get_section_y(rect: &egui::Rect, sections: &Vec<Section>, addr: u64) -> f32 {
    let separator_offset = 8.5;
    let mut sum = -separator_offset;
    let line_height = 17f32;
    for (start, end, _, _) in sections {
        if addr >= *start && addr <= *end {
            sum += line_height + (end - addr) as f32 * line_height + separator_offset;
            break;
        } else {
            sum += line_height + (end - start) as f32 * line_height + separator_offset;
        }
    }
    rect.min.y + sum - line_height / 2.0
}

fn read_heap_value(addr: u64, sections: &Vec<Section>) -> Option<u64> {
    for (start, end, _, data) in sections.iter() {
        if addr >= *start && addr <= *end {
            if let Some(Ok(data)) = data.ready() {
                let offset = addr - *start;
                let offset = offset as usize;

                let value = data[offset] as u64
                    | (data[offset + 1] as u64) << 8
                    | (data[offset + 2] as u64) << 16
                    | (data[offset + 3] as u64) << 24
                    | (data[offset + 4] as u64) << 32
                    | (data[offset + 5] as u64) << 40
                    | (data[offset + 6] as u64) << 48
                    | (data[offset + 7] as u64) << 56;
                return Some(value);
            }
        }
    }
    None
}

const COLORS: [Color32; 4] = [
    Color32::DARK_RED,
    Color32::from_rgb(169, 158, 0),
    Color32::DARK_GREEN,
    Color32::DARK_BLUE,
];

//TODO: maybe return possible section to load and factor out section loading code to seperate function in render_stack function
// (size,addr)
fn render_heap_variable(
    ui: &mut egui::Ui,
    rect: &egui::Rect,
    sections: &Vec<Section>,
    addr: u64,
    types: &DataType,
    type_index: usize,
    recurse: usize,
    color_walk: usize,
    draw_ref_count: &mut i32,
) -> Vec<(usize, u64)> {
    let top = get_section_y(
        rect,
        sections,
        addr + get_byte_size(types, type_index) as u64 - 1,
    ) - 3.5;
    let bottom = get_section_y(rect, sections, addr) - 1.5;
    render_var_line(
        ui,
        rect,
        278.0 - recurse as f32 * 24.0,
        top,
        bottom,
        &types.0[type_index].1.to_string(),
        COLORS[color_walk as usize % COLORS.len()],
        true,
    );
    let mut ret_val = vec![];
    match &types.0[type_index].1 {
        TypeName::Arr {
            arr_type: _,
            count: _,
        } => todo!(), //TODO: arrays on the heap
        TypeName::ProductType {
            name: _,
            members,
            byte_size: _,
        } => {
            for (_, membertype, offset) in members {
                ret_val.append(&mut render_heap_variable(
                    ui,
                    rect,
                    sections,
                    addr + *offset as u64,
                    types,
                    *membertype,
                    recurse + 1,
                    color_walk,
                    draw_ref_count,
                ));
            }
        }
        TypeName::Ref { index } => {
            let value = read_heap_value(addr, sections);
            if let Some(value) = value {
                if sections
                    .iter()
                    .any(|(start, end, _, _)| value >= *start && value <= *end)
                {
                    // render recursively
                    if let Some(index) = index {
                        let size = get_byte_size(types, *index);
                        if sections.iter().any(|(start, end, _, _)| {
                            value as u64 + size as u64 >= *start
                                && value as u64 + size as u64 <= *end
                        }) {
                            // type fits
                            ret_val.append(&mut render_heap_variable(
                                ui,
                                rect,
                                sections,
                                value,
                                types,
                                *index,
                                0,
                                color_walk + 1,
                                draw_ref_count,
                            ));
                        } else {
                            // type does not fit
                            // request section to be loaded
                            ret_val.push((size, value));
                        }
                    }
                    render_ref_arrow(
                        ui,
                        rect,
                        draw_ref_count,
                        COLORS[color_walk % COLORS.len()],
                        (top + bottom) / 2.0 + 10.0,
                        get_section_y(rect, sections, value),
                        true,
                        98.0,
                        true,
                    );
                } else {
                    if let Some(index) = index {
                        ret_val.push((get_byte_size(types, *index), value));
                    } else {
                        ret_val.push((8, value));
                    }
                }
            }
        }
        _ => {}
    }
    ret_val
}

fn render_section(ui: &mut egui::Ui, start: u64, memory: &Vec<u8>, name: &String) {
    ui.horizontal(|ui| {
        let line_height = 17f32;
        let (_rect, _) = ui.allocate_exact_size(
            Vec2::new(80.0, line_height + memory.len() as f32 * line_height),
            egui::Sense::hover(),
        );
        // ui.painter().rect(
        //     rect,
        //     0.0,
        //     Color32::WHITE,
        //     egui::Stroke {
        //         width: 2.0,
        //         color: Color32::BLACK,
        //     },
        // );
        ui.vertical(|ui| {
            ui.add(egui::Label::new(name).wrap(false));
            for (i, byte) in memory.iter().enumerate().rev() {
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "{:#x} {:#04x} {}",
                            start + i as u64,
                            byte,
                            if (*byte as char).is_ascii() && (*byte as char) != '\n' {
                                *byte as char
                            } else {
                                '·'
                            }
                        ))
                        .monospace(),
                    )
                    .wrap(false),
                );
            }
            ui.separator();
        });
    });
}

/// return type: (addr, type_index)
fn get_all_ptrs(datatypes: &DataType, type_index: usize, addr: u64) -> Vec<(u64, usize)> {
    match &datatypes.0[type_index].1 {
        TypeName::Name {
            name: _,
            byte_size: _,
        } => vec![],
        TypeName::Arr { arr_type, count } => {
            let mut ptrs = vec![];
            for i in 0..count.iter().cloned().reduce(|e1, e2| e1 * e2).unwrap() {
                ptrs.append(&mut get_all_ptrs(
                    datatypes,
                    *arr_type,
                    addr + i as u64 * get_byte_size(datatypes, *arr_type) as u64,
                ));
            }
            ptrs
        }
        TypeName::Ref { index: _ } => vec![(addr, type_index)],
        TypeName::ProductType {
            name: _,
            members,
            byte_size: _,
        } => {
            let mut ptrs = vec![];
            for (_, type_index, offset) in members {
                ptrs.append(&mut get_all_ptrs(
                    datatypes,
                    *type_index,
                    addr + *offset as u64,
                ));
            }
            ptrs
        }
    }
}
macro_rules! load_section {
    ($backend_url:expr, $sections:expr, $m:expr, $size:expr, $value:expr,) => {
        // Merge if near block is already in $sections
        if $sections
            .iter()
            .any(|(start, end, _, _)| $value >= *start && $value + $size as u64 <= *end)
        {
            // don't load
        } else {
            if let Some(pos) = $sections.iter().position(|(start, end, _, _)| {
                ($value + $size as u64 + 8 >= *start && $value <= *end)
                    || ($value >= *start && $value - 16 <= *end)
            }) {
                if $value + $size as u64 + 8 >= $sections[pos].0 && $value <= $sections[pos].1 {
                    $sections[pos].0 -= $size as u64 + 12;
                } else if $value - 16 <= $sections[pos].1 && $value >= $sections[pos].0 {
                    $sections[pos].1 += $size as u64 + 16;
                }
                $sections[pos].3 = dispatch!(
                    $backend_url,
                    Command::ReadMemory($sections[pos].0, $sections[pos].1 - $sections[pos].0),
                    Memory
                );
            } else {
                $sections.push((
                    $value - 16,
                    $value + $size as u64 + 4,
                    $m.mapped.clone(),
                    dispatch!(
                        $backend_url,
                        Command::ReadMemory($value - 16, 16 + $size as u64 + 4,),
                        Memory
                    ),
                ));
                $sections.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            }
        }
    };
}

impl VariableWindow {
    pub fn new(backend_url: Url) -> Self {
        let mut s = Self {
            variables: Promise::from_ready(Err(String::new())),
            backend_url,
            active_tab: ActiveTab::StackView,
            registers: Promise::from_ready(Err(String::new())),
            stack: None,
            hover_text: None,
            additional_loaded_sections: vec![],
            mapping: Promise::from_ready(Err(String::new())),
            lock_stack: false,
            lock_stack_addr: 0,
            rsp_offset: 16,
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
                                                        .unwrap_or(DataType(vec![(
                                                            0,
                                                            stackium_shared::TypeName::Name {
                                                                name: "??".to_owned(),
                                                                byte_size: 0
                                                            }
                                                        )]))
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
        let rsp_offset = self.rsp_offset;
        if let Some(Ok(registers)) = self.registers.ready() {
            let stack_start = if self.lock_stack {
                self.lock_stack_addr
            } else {
                registers.rbp
            };
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
                                let mut draw_ref_count = 0;

                                if let Some(Ok(vars)) = self.variables.ready() {
                                    let vars: Vec<Variable> = vars
                                        .iter()
                                        .filter(|v| {
                                            v.low_pc <= registers.rip && v.high_pc >= registers.rip
                                        })
                                        .map(|v| v.clone())
                                        .collect();

                                    // addr, value, types, type_index
                                    let mut heap_vars = Vec::<(u64, u64, DataType, usize)>::new();
                                    ui.with_layout(
                                        egui::Layout::top_down(egui::Align::TOP),
                                        |ui| {
                                            for (ivar, var) in vars
                                                .iter()
                                                .chain(
                                                    [
                                                        Variable {
                                                            name: Some("Return Address".to_owned()),
                                                            type_name: Some(DataType(vec![(
                                                                0,
                                                                stackium_shared::TypeName::Ref {
                                                                    index: None,
                                                                },
                                                            )])),
                                                            value: None,
                                                            file: None,
                                                            line: None,
                                                            addr: Some(stack_start + 8),
                                                            high_pc: 0,
                                                            low_pc: 0,
                                                        },
                                                        Variable {
                                                            name: Some(
                                                                "Calling Base Pointer".to_owned(),
                                                            ),
                                                            type_name: Some(DataType(vec![(
                                                                0,
                                                                stackium_shared::TypeName::Ref {
                                                                    index: None,
                                                                },
                                                            )])),
                                                            value: None,
                                                            file: None,
                                                            line: None,
                                                            addr: Some(stack_start),
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
                                                    COLORS[ivar % COLORS.len()],
                                                    &mut draw_ref_count,
                                                    var,
                                                    0f32,
                                                    stack,
                                                );
                                                if let (Some(addr), Some(datatype)) =
                                                    (&var.addr, &var.type_name)
                                                {
                                                    for (addr, typeindex) in
                                                        get_all_ptrs(datatype, 0, *addr)
                                                    {
                                                        let value = read_value_stack(
                                                            addr, registers, rsp_offset, &stack,
                                                        );
                                                        if value >= registers.rsp - rsp_offset
                                                            && value <= stack_start + 16
                                                        {
                                                            let current_y = get_y_from_addr(
                                                                &rect,
                                                                registers.rsp,
                                                                rsp_offset,
                                                                heightpad,
                                                                addr + 2,
                                                            ) - 10.0;
                                                            let dst_y = get_y_from_addr(
                                                                &rect,
                                                                registers.rsp,
                                                                rsp_offset,
                                                                heightpad,
                                                                value,
                                                            );
                                                            render_ref_arrow(
                                                                ui,
                                                                &rect,
                                                                &mut draw_ref_count,
                                                                COLORS[ivar % COLORS.len()],
                                                                current_y,
                                                                dst_y,
                                                                false,
                                                                0.0,
                                                                false,
                                                            );
                                                            if !vars
                                                                .iter()
                                                                .any(|v| v.addr == Some(value))
                                                            {
                                                                // render stack variable if not in var arr
                                                                if let TypeName::Ref {
                                                                    index: Some(index),
                                                                } = datatype.0[typeindex].1
                                                                {
                                                                    render_variable_override(
                                                                        ui,
                                                                        &rect,
                                                                        registers,
                                                                        rsp_offset,
                                                                        heightpad,
                                                                        height,
                                                                        COLORS[ivar % COLORS.len()],
                                                                        &mut draw_ref_count,
                                                                        &Variable {
                                                                            name: Some(
                                                                                datatype.0[index]
                                                                                    .1
                                                                                    .to_string(),
                                                                            ),
                                                                            type_name: Some(
                                                                                datatype.clone(),
                                                                            ),
                                                                            value: None,
                                                                            file: None,
                                                                            line: None,
                                                                            addr: Some(value),
                                                                            high_pc: 0,
                                                                            low_pc: 0,
                                                                        },
                                                                        0f32,
                                                                        stack,
                                                                        index,
                                                                    );
                                                                }
                                                            }
                                                        } else if let Some((
                                                            start,
                                                            end,
                                                            name,
                                                            region,
                                                        )) = self
                                                            .additional_loaded_sections
                                                            .iter()
                                                            .find(|(start, end, _, _)| {
                                                                value >= *start
                                                                    && value
                                                                        //TODO: check for correct size?
                                                                        // + get_byte_size(
                                                                            // datatype, typeindex,
                                                                        // )
                                                                            as u64
                                                                        <= *end
                                                            })
                                                        {
                                                            // everything ok 👍
                                                            // draw arrow

                                                            let current_y = get_y_from_addr(
                                                                &rect,
                                                                registers.rsp,
                                                                rsp_offset,
                                                                heightpad,
                                                                addr + 2,
                                                            ) - 10.0;
                                                            let dst_y = get_section_y(
                                                                &rect,
                                                                &self.additional_loaded_sections,
                                                                value,
                                                            );
                                                            if !heap_vars
                                                                .iter()
                                                                .any(|(_, v, _, _)| *v == value)
                                                            {
                                                                heap_vars.push((
                                                                    addr,
                                                                    value,
                                                                    datatype.clone(),
                                                                    typeindex,
                                                                ));
                                                            }
                                                            render_ref_arrow(
                                                                ui,
                                                                &rect,
                                                                &mut draw_ref_count,
                                                                COLORS[ivar % COLORS.len()],
                                                                current_y,
                                                                dst_y,
                                                                true,
                                                                98.0,
                                                                false,
                                                            )
                                                            // if let Some(Ok(region)) = region.ready() {
                                                            //     render_section(ui, *start, region, name);
                                                            // }
                                                        } else if let Some(Ok(mapping)) =
                                                            self.mapping.ready()
                                                        {
                                                            if let Some(m) =
                                                                mapping.iter().find(|map| {
                                                                    map.from <= value
                                                                        && value
                                                                            + get_byte_size(
                                                                                datatype, typeindex,
                                                                            )
                                                                                as u64
                                                                            <= map.to
                                                                })
                                                            {
                                                                let size = if let TypeName::Ref {
                                                                    index: Some(index),
                                                                } =
                                                                    datatype.0[typeindex].1
                                                                {
                                                                    get_byte_size(datatype, index)
                                                                } else {
                                                                    8
                                                                };
                                                                load_section!(
                                                                    self.backend_url.clone(),
                                                                    self.additional_loaded_sections,
                                                                    m,
                                                                    size,
                                                                    value,
                                                                );
                                                            } else {
                                                                let current_y = get_y_from_addr(
                                                                    &rect,
                                                                    registers.rsp,
                                                                    rsp_offset,
                                                                    heightpad,
                                                                    addr + 2,
                                                                ) - 10.0;
                                                                render_invalid_ptr_arrow(
                                                                    ui,
                                                                    &rect,
                                                                    current_y,
                                                                    COLORS[ivar % COLORS.len()],
                                                                )
                                                            }
                                                        }
                                                    }
                                                }
                                                if let (Some(typename), Some(addr)) =
                                                    (&var.type_name, var.addr)
                                                {
                                                    let size = get_byte_size(typename, 0);
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
                                                        self.hover_text =
                                                            Some(typename.to_string());
                                                    }
                                                }
                                            }

                                            for (start, _, name, section) in
                                                self.additional_loaded_sections.iter()
                                            {
                                                if let Some(Ok(section)) = section.ready() {
                                                    render_section(ui, *start, section, name);
                                                }
                                            }
                                            for (i, (addr, value, datatype, index)) in
                                                heap_vars.iter().enumerate()
                                            {
                                                if let TypeName::Ref { index: Some(index) } =
                                                    datatype.0[*index].1
                                                {
                                                    let sections_to_check = render_heap_variable(
                                                        ui,
                                                        &rect,
                                                        &self.additional_loaded_sections,
                                                        *value,
                                                        datatype,
                                                        index,
                                                        0,
                                                        0,
                                                        &mut draw_ref_count,
                                                    );
                                                    if let Some(Ok(m)) = self.mapping.ready() {
                                                        for (size, value) in sections_to_check {
                                                            if let Some(m) = m.iter().find(|map| {
                                                                map.from <= value
                                                                    && value + size as u64 <= map.to
                                                            }) {
                                                                load_section!(
                                                                    self.backend_url.clone(),
                                                                    self.additional_loaded_sections,
                                                                    m,
                                                                    size,
                                                                    value,
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        },
                                    );
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
                                            color: ui.visuals().text_color(),
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
                                        ui.visuals().text_color(),
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
                    if stack_start >= registers.rsp {
                        self.stack = Some(dispatch_command_and_then(
                            self.backend_url.clone(),
                            Command::ReadMemory(
                                registers.rsp - rsp_offset,
                                (stack_start - registers.rsp) + 16 + rsp_offset,
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
        // ui.horizontal(|ui| {
        // ui.selectable_value(
        //     &mut self.active_tab,
        //     ActiveTab::VariableList,
        //     "Variable List",
        // );
        // ui.selectable_value(&mut self.active_tab, ActiveTab::StackView, "Memory");
        // });
        let mut stack_dirty = false;
        if let Some(Ok(registers)) = self.registers.ready() {
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut self.lock_stack, "Lock stack start")
                    .clicked()
                {
                    if self.lock_stack {
                        self.lock_stack_addr = registers.rbp;
                    }
                }
                if self.lock_stack {
                    ui.label(format!("Locked at: {:#x}", self.lock_stack_addr));
                }
                if ui
                    .add(egui::Slider::new(&mut self.rsp_offset, 0..=64).text("Stack End Offset"))
                    .changed()
                {
                    stack_dirty = true;
                }
            });
        }
        if stack_dirty {
            self.dirty();
        }

        let res = match self.active_tab {
            ActiveTab::VariableList => self.render_variable_list(ui),
            ActiveTab::StackView => self.render_stack(ui),
        };
        (false, res)
    }
}
