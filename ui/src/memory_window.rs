use egui::{Align, Align2, Color32, RichText, Stroke, Vec2b};
use egui_plot::{Arrows, Plot};
use egui_plot::{Line, PlotPoint, PlotPoints, PlotUi, Polygon, Text, VLine};
use poll_promise::Promise;
use stackium_shared::{
    Command, CommandOutput, DiscoveredVariable, Registers, VARIABLE_MEM_PADDING,
};
use std::collections::HashSet;
use std::ops::Range;
use url::Url;

use crate::{
    command::dispatch_command_and_then, debugger_window::DebuggerWindowImpl,
    rotated_plot_text::RotText, variable_window::get_byte_size,
};

pub struct MemoryWindow {
    backend_url: Url,
    variables: Promise<Result<Vec<DiscoveredVariable>, String>>,
    registers: Promise<Result<Registers, String>>,
    grid: bool,
    coordinates: bool,
    cached_addresses: Option<Vec<u64>>,
}

impl MemoryWindow {
    pub fn new(backend_url: Url) -> Self {
        let mut ret = Self {
            backend_url,
            variables: Promise::from_ready(Err(String::new())),
            registers: Promise::from_ready(Err(String::new())),
            grid: false,
            coordinates: false,
            cached_addresses: None,
        };
        ret.dirty();
        ret
    }
}

const ADDR_SPACING: f32 = 1.0f32;
const ADDR_LENGTH: f32 = 5.5f32;
const BAR_THICKNESS: f64 = 1.0f64;

const COLORS: [egui::Color32; 6] = [
    egui::Color32::from_rgb(0x00, 0x00, 0xff),
    egui::Color32::from_rgb(0x00, 0xff, 0x00),
    egui::Color32::from_rgb(0xff, 0x00, 0x00),
    egui::Color32::from_rgb(0x00, 0xff, 0xff),
    egui::Color32::from_rgb(0xff, 0x00, 0xff),
    egui::Color32::from_rgb(0xff, 0xff, 0x00),
];

fn render_pointer_arrow(
    ui: &mut PlotUi,
    start: PlotPoint,
    end: PlotPoint,
    color: &egui::Color32,
    arrow_counter: &mut i32,
    is_invalid: bool,
) {
    // ui.arrows(
    //     Arrows::new(
    //         PlotPoints::new(vec![[start.x, start.y]]),
    //         PlotPoints::new(vec![[end.x, end.y]]),
    //     )
    //     .tip_length(5.0f32),
    // );
    const ARROWS_HOME_POS: f64 = 30f64;
    const ARROWS_HOME_OFFSET: f64 = 0.5;
    const ARROWS_END_OFFSET: f64 = 7f64;

    let arrow_home = ARROWS_HOME_POS + ARROWS_HOME_OFFSET * *arrow_counter as f64;

    if is_invalid {
        ui.line(
            Line::new(PlotPoints::new(vec![
                [start.x, start.y],
                [
                    arrow_home - ARROWS_HOME_OFFSET * (*arrow_counter + 1) as f64,
                    start.y,
                ],
            ]))
            .color(*color),
        );

        ui.text(
            Text::new(
                PlotPoint::new(
                    arrow_home - ARROWS_HOME_OFFSET * (*arrow_counter + 1) as f64,
                    start.y,
                ),
                RichText::new("NULL")
                    .font(egui::FontId {
                        size: text_size(ui) * 1.5,
                        family: egui::FontFamily::Monospace,
                    })
                    .color(ui.ctx().style().visuals.error_fg_color)
                    .strong(),
            )
            .anchor(Align2::LEFT_CENTER),
        );
        return;
    }

    let tip_length = text_size(ui);

    ui.arrows(
        Arrows::new(
            PlotPoints::new(vec![[start.x, start.y]]),
            PlotPoints::new(vec![[arrow_home, start.y]]),
        )
        .tip_length(tip_length)
        .color(*color)
        .highlight(true),
    );
    ui.arrows(
        Arrows::new(
            PlotPoints::new(vec![[arrow_home, start.y]]),
            PlotPoints::new(vec![[arrow_home, end.y]]),
        )
        .tip_length(tip_length)
        .color(*color)
        .highlight(true),
    );
    ui.arrows(
        Arrows::new(
            PlotPoints::new(vec![[arrow_home, end.y]]),
            PlotPoints::new(vec![[end.x + ARROWS_END_OFFSET, end.y]]),
        )
        .tip_length(tip_length)
        .color(*color)
        .highlight(true),
    );
    *arrow_counter += 1;
}

fn render_type(
    ui: &mut PlotUi,
    variable: &DiscoveredVariable,
    type_index: usize,
    initial_bar: bool,
    addresses: &Vec<u64>,
    stack_range: &Range<u64>,
    offset: usize,
    name_override: Option<String>,
    address: u64,
    color_override: Option<egui::Color32>,
    arrow_counter: &mut i32,
) {
    let color = color_override.unwrap_or(COLORS[address as usize % COLORS.len()]);
    let multiplier = if initial_bar { 2.0 } else { 1.0 };
    if let (Some(name), Some(memory)) = (&variable.name, &variable.memory) {
        let name = name_override.unwrap_or(name.clone());
        let mut position = addr_to_pos(address, &stack_range, Some(addresses));
        const BAR_PADDING: f64 = 0.2;
        position.x += BAR_THICKNESS * (multiplier <= 1.5) as u32 as f64
            + ADDR_LENGTH as f64
            + BAR_THICKNESS * offset as f64
            + offset as f64 * BAR_PADDING;
        let dest = ADDR_SPACING as f64 * get_byte_size(&variable.types, type_index) as f64;
        ui.polygon(
            Polygon::new(PlotPoints::new(vec![
                [position.x - 0.1 * (multiplier - 1.0), position.y],
                [position.x - 0.1 * (multiplier - 1.0), position.y + dest],
                [position.x + BAR_THICKNESS * multiplier, position.y + dest],
                [position.x + BAR_THICKNESS * multiplier, position.y],
            ]))
            .stroke(Stroke::new(1.0, color)),
        );
        ui.add(RotText::new(
            name.clone(),
            -std::f32::consts::FRAC_PI_2,
            text_size(ui),
            (
                (position.x + BAR_THICKNESS * (multiplier - 1.0)) as f32,
                position.y as f32 + 0.2f32,
            ),
            None,
        ));
        match &variable.types.0[type_index].1 {
            stackium_shared::TypeName::Name {
                name: _,
                byte_size: _,
            } => {}
            stackium_shared::TypeName::Arr { arr_type, count } => {
                for i in 0..count.iter().fold(1, |acc, e| acc * *e) {
                    render_type(
                        ui,
                        variable,
                        *arr_type,
                        false,
                        addresses,
                        stack_range,
                        offset + 1,
                        Some(format!("{}[{}]", name, i)),
                        address + get_byte_size(&variable.types, *arr_type) as u64 * i as u64,
                        Some(color),
                        arrow_counter,
                    );
                }
            }
            stackium_shared::TypeName::Ref { index: _ } => {
                let base_addr = variable.addr.unwrap() - VARIABLE_MEM_PADDING;
                let mem_index = (address - base_addr) as usize;
                let ptr_val = u64::from_le_bytes(
                    memory[mem_index..mem_index + 8]
                        .try_into()
                        .expect("slice with incorrect length"),
                );
                let ptr_dst = addr_to_pos(ptr_val, &stack_range, Some(addresses));
                render_pointer_arrow(ui, position, ptr_dst, &color, arrow_counter, ptr_val == 0);
            }
            stackium_shared::TypeName::ProductType {
                name: _,
                members,
                byte_size: _,
            } => {
                for (_, (name, member_type_index, member_offset)) in members.iter().enumerate() {
                    render_type(
                        ui,
                        variable,
                        *member_type_index,
                        false,
                        addresses,
                        stack_range,
                        offset + 1,
                        Some(name.clone()),
                        address + *member_offset as u64,
                        Some(color),
                        arrow_counter,
                    );
                }
            }
        }
    }
}

fn render_variable(
    variable: &DiscoveredVariable,
    addresses: &Vec<u64>,
    ui: &mut PlotUi,
    stack_range: Range<u64>,
    initial_bar: bool,
    arrow_counter: &mut i32,
) {
    if let (Some(address), Some(name), Some(memory)) =
        (variable.addr, &variable.name, &variable.memory)
    {
        // if address < stack_range.start || address > stack_range.end {
        //     // RENDER ADDRESSES FOR NON STACK'D VARIABLES
        //     for (i, _) in memory.iter().enumerate() {
        //         let addr = address - VARIABLE_MEM_PADDING + i as u64;
        //         let mut byte_pos = addr_to_pos(addr, &stack_range, Some(addresses));
        //         byte_pos.y += 0.5f64;
        //         ui.text(
        //             Text::new(
        //                 byte_pos,
        //                 RichText::new(format!("{:012x}", addr)).font(egui::FontId {
        //                     size: text_size(ui),
        //                     family: egui::FontFamily::Monospace,
        //                 }),
        //             )
        //             .anchor(Align2::LEFT_CENTER),
        //         );
        //     }
        // }
        render_type(
            ui,
            variable,
            variable.type_index,
            true,
            addresses,
            &stack_range,
            0,
            None,
            address,
            None,
            arrow_counter,
        );
        for (i, byte) in memory.iter().enumerate() {
            let addr = address - VARIABLE_MEM_PADDING + i as u64;
            let mut byte_pos = addr_to_pos(addr, &stack_range, Some(addresses));
            byte_pos.x += ADDR_LENGTH as f64;
            byte_pos.y += 0.5f64;
            ui.text(
                Text::new(
                    byte_pos,
                    RichText::new(format!("{:02x}", byte)).font(egui::FontId {
                        size: text_size(ui),
                        family: egui::FontFamily::Monospace,
                    }),
                )
                .anchor(Align2::LEFT_CENTER),
            );
        }
    }
}

const LOAD_POS: f64 = 20f64;

fn addr_to_pos(address: u64, stack_range: &Range<u64>, addresses: Option<&Vec<u64>>) -> PlotPoint {
    if address < stack_range.start || address >= stack_range.end {
        let mut offset: i64 = -1;
        if let Some(addresses) = addresses {
            offset = addresses
                .iter()
                .position(|&x| x == address)
                .map(|x| x as i64)
                .unwrap_or(-5);
        }
        PlotPoint::new(LOAD_POS, offset as f32 * ADDR_SPACING)
    } else {
        PlotPoint::new(0, (address - stack_range.start) as f32 * ADDR_SPACING)
    }
}

fn text_size(plot_ui: &PlotUi) -> f32 {
    let scale = plot_ui
        .transform()
        .rect_from_values(&[0.0, 0.0].into(), &[1.0, 1.0].into())
        .size()
        * 0.7;
    scale.max_elem().clamp(0.001, 250.0)
}

fn render_category(ui: &mut PlotUi, category: &str, rect: [PlotPoint; 2]) {
    const TOP_TEXT_OFFSET: f64 = 1.0;
    const GENERAL_PADDING: f64 = 0.5;
    ui.add(
        Polygon::new(PlotPoints::new(vec![
            [rect[0].x - GENERAL_PADDING, rect[0].y - GENERAL_PADDING],
            [rect[0].x - GENERAL_PADDING, rect[1].y + TOP_TEXT_OFFSET],
            [rect[1].x + GENERAL_PADDING, rect[1].y + TOP_TEXT_OFFSET],
            [rect[1].x + GENERAL_PADDING, rect[0].y - GENERAL_PADDING],
        ]))
        .stroke(Stroke::new(1.0, egui::Color32::GRAY)),
    );
    ui.text(
        Text::new(
            PlotPoint::new(rect[0].x, rect[1].y),
            RichText::new(category.to_string()).font(egui::FontId {
                size: text_size(ui),
                family: egui::FontFamily::Monospace,
            }),
        )
        .anchor(Align2::LEFT_BOTTOM),
    );
}

fn render_addresses(ui: &mut PlotUi, stack_range: &Range<u64>, addresses: &Vec<u64>) {
    if stack_range.end <= stack_range.start {
        return;
    }
    render_category(
        ui,
        "Stack",
        [
            PlotPoint::new(0.0, 0.0),
            PlotPoint::new(
                ADDR_LENGTH as f32 * 2.0,
                (stack_range.end - stack_range.start) as f32 * ADDR_SPACING,
            ),
        ],
    );
    for addr in addresses {
        let mut addr_pos = addr_to_pos(*addr, &stack_range, Some(addresses));
        addr_pos.y += 0.5f64;
        ui.text(
            Text::new(
                addr_pos,
                RichText::new(format!("{:012x}", addr)).font(egui::FontId {
                    size: text_size(ui),
                    family: egui::FontFamily::Monospace,
                }),
            )
            .anchor(Align2::LEFT_CENTER),
        );
    }
    for (_, addr) in stack_range.clone().enumerate() {
        let mut addr_pos = addr_to_pos(addr, &stack_range, None);
        addr_pos.y += 0.5f64;
        ui.text(
            Text::new(
                addr_pos,
                RichText::new(format!("{:012x}", addr)).font(egui::FontId {
                    size: text_size(ui),
                    family: egui::FontFamily::Monospace,
                }),
            )
            .anchor(Align2::LEFT_CENTER),
        );
    }
}

impl DebuggerWindowImpl for MemoryWindow {
    fn dirty(&mut self) {
        self.variables = dispatch!(
            self.backend_url.clone(),
            Command::DiscoverVariables,
            DiscoveredVariables
        );
        self.registers = dispatch!(self.backend_url.clone(), Command::GetRegister, Registers);
        self.cached_addresses = None;
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.grid, "Show Grid");
            ui.checkbox(&mut self.coordinates, "Show Coordinates");
        });
        if let (Some(Ok(variables)), Some(Ok(registers))) =
            (self.variables.ready(), self.registers.ready())
        {
            let stack_range = registers.stack_pointer..registers.base_pointer;
            let mut deduplicated_variables = variables.clone();
            deduplicated_variables.sort_by(|a, b| a.addr.unwrap().cmp(&b.addr.unwrap()));
            deduplicated_variables.dedup_by(|a, b| a.addr.unwrap() == b.addr.unwrap());
            self.cached_addresses = None;
            if self.cached_addresses.is_none() {
                let mut addresses = deduplicated_variables
                    .iter()
                    .map(|v| {
                        (v.addr.unwrap() - VARIABLE_MEM_PADDING)
                            ..(v.addr.unwrap() - VARIABLE_MEM_PADDING
                                + v.memory.as_ref().unwrap().len() as u64)
                    })
                    .flatten()
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                addresses.sort();
                for a in stack_range.clone() {
                    addresses.remove(addresses.iter().position(|&x| x == a).unwrap_or_default());
                }
                self.cached_addresses = Some(addresses.into_iter().collect());
            }
            let mut arrow_counter = 0;
            Plot::new("Memory")
                // .height(600f32)
                .show_axes([false, false])
                .show_grid(Vec2b::new(self.grid, self.grid))
                .data_aspect(1.0)
                .auto_bounds(Vec2b::new(true, true))
                .show_x(self.coordinates)
                .show_y(self.coordinates)
                // .allow_zoom(false)
                .show(ui, |ui| {
                    render_addresses(ui, &stack_range, self.cached_addresses.as_ref().unwrap());
                    for variable in deduplicated_variables {
                        render_variable(
                            &variable,
                            self.cached_addresses.as_ref().unwrap(),
                            ui,
                            stack_range.clone(),
                            true,
                            &mut arrow_counter,
                        );
                    }
                });
        } else {
            ui.spinner();
        }
        false
    }
}
