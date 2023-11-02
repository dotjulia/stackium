use egui::{Rect, Response, Sense, Stroke, Ui, Vec2};
use poll_promise::Promise;
use stackium_shared::{Command, CommandOutput, DataType, MemoryMap, Variable};
use url::Url;

use crate::{debugger_window::DebuggerWindowImpl, variable_window::get_byte_size};

trait NodeContent: Clone {
    fn render(&self, ui: &mut Ui) -> Response;
}

#[derive(Clone)]
struct Node<Data: NodeContent> {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    id: usize,
    connections: Vec<usize>,
    pub data: Data,
}

impl<D: NodeContent> Node<D> {
    pub fn new(id: usize, connections: Vec<usize>, data: D) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 96f32,
            height: 96f32,
            id,
            connections,
            data,
        }
    }

    pub fn rect(&self, canvas: Rect) -> Rect {
        Rect::from_x_y_ranges(
            (canvas.min.x + self.x)..=(canvas.min.x + self.x + self.width),
            (canvas.min.y + self.y)..=(canvas.min.y + self.y + self.height),
        )
    }

    pub fn render(&self, ui: &mut Ui, canvas: Rect) {
        let fill_color = ui.style().visuals.extreme_bg_color;
        let stroke_color = ui.style().visuals.text_color();
        let rect = self.rect(canvas);
        ui.painter().rect(
            rect,
            4.0,
            fill_color,
            Stroke {
                width: 2.0,
                color: stroke_color,
            },
        );
        ui.put(rect, |ui: &mut Ui| self.data.render(ui));
    }
}

struct Graph<Data: NodeContent> {
    pub nodes: Vec<Node<Data>>,
    dragging_node: Option<usize>,
}

impl<D: NodeContent> Graph<D> {
    pub fn new(nodes: Vec<Node<D>>) -> Self {
        Self {
            nodes,
            dragging_node: None,
        }
    }
    pub fn arrange(&mut self) {
        const PADDING: f32 = 10.0;
        let per_line = (self.nodes.len() as f32).sqrt() as usize;
        let mut curr_line_count = 0;
        let mut y = 0f32;
        for node in self.nodes.iter_mut() {
            node.y = node.height * y + y * PADDING;
            node.x = curr_line_count as f32 * node.width + curr_line_count as f32 * PADDING;
            curr_line_count += 1;
            if curr_line_count >= per_line {
                curr_line_count = 0;
                y += 1f32;
            }
        }
    }
    pub fn arrange_place(mut self) -> Self {
        self.arrange();
        self
    }
    pub fn render(&mut self, ui: &mut Ui, width: f32, height: f32) -> Response {
        let (rect, res) = ui.allocate_exact_size(Vec2::new(width, height), Sense::drag());
        let nodes_before = self.nodes.clone();
        for node in self.nodes.iter_mut() {
            node.render(ui, rect);
            for connection in node.connections.iter() {
                if let Some(other_node) = nodes_before.iter().find(|n| n.id == *connection) {
                    ui.painter().line_segment(
                        [node.rect(rect).max, other_node.rect(rect).min],
                        Stroke {
                            width: 4.0,
                            color: ui.visuals().text_color(),
                        },
                    );
                }
            }
        }
        if res.drag_started() {
            if let Some(index) = self
                .nodes
                .iter()
                .position(|n| ui.rect_contains_pointer(n.rect(rect)))
            {
                self.dragging_node = Some(index);
            }
        }
        if let Some(node_index) = self.dragging_node {
            self.nodes[node_index].x += res.drag_delta().x;
            self.nodes[node_index].y += res.drag_delta().y;
            self.nodes[node_index].x = self.nodes[node_index].x.abs();
            self.nodes[node_index].y = self.nodes[node_index].y.abs();
        }
        if res.drag_released() {
            self.dragging_node = None;
        }
        res
    }
}

#[derive(Clone)]
struct VariableNodeData {
    types: DataType,
    typeid: usize,
    addr: u64,
}

impl NodeContent for VariableNodeData {
    fn render(&self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.add_space(4.0);
                match &self.types.0[self.typeid].1 {
                    stackium_shared::TypeName::Name { name, byte_size } => {
                        ui.label(name);
                    }
                    stackium_shared::TypeName::Arr { arr_type, count } => {
                        ui.label(format!(
                            "{}{}",
                            self.types.0[*arr_type].1.to_string(),
                            count
                                .iter()
                                .map(|i| format!("[{}]", i))
                                .collect::<Vec<String>>()
                                .join(""),
                        ));
                    }
                    stackium_shared::TypeName::Ref { index } => {
                        if let Some(index) = index {
                            ui.label(format!("{}*", self.types.0[*index].1.to_string()));
                        } else {
                            ui.label("void*");
                        }
                    }
                    stackium_shared::TypeName::ProductType {
                        name,
                        members,
                        byte_size,
                    } => {
                        ui.label(name);
                        for (name, _, _) in members {
                            ui.label(name);
                        }
                    }
                };
            });
        });
        ui.label(format!("{:#x?}", self.addr))
    }
}

type Section = (u64, u64, Promise<Result<Vec<u8>, String>>);

pub struct GraphWindow {
    backend_url: Url,
    graph: Graph<VariableNodeData>,
    variables: Promise<Result<Vec<Variable>, String>>,
    mapping: Promise<Result<Vec<MemoryMap>, String>>,
    additional_loaded_sections: Vec<Section>,
}

impl GraphWindow {
    pub fn new(backend_url: Url) -> Self {
        let mut ret = Self {
            backend_url,
            graph: Graph::new(vec![]).arrange_place(),
            variables: Promise::from_ready(Err(String::new())),
            mapping: Promise::from_ready(Err(String::new())),
            additional_loaded_sections: vec![],
        };
        ret.dirty();
        ret
    }
}

impl DebuggerWindowImpl for GraphWindow {
    fn dirty(&mut self) {
        self.graph.nodes = vec![];
        self.variables = dispatch!(self.backend_url.clone(), Command::ReadVariables, Variables);
        self.mapping = dispatch!(self.backend_url.clone(), Command::Maps, Maps);
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> (bool, egui::Response) {
        let mut found_vars = vec![];
        if let (Some(Ok(mapping)), Some(Ok(variables))) =
            (self.mapping.ready(), self.variables.ready())
        {
            for variable in variables {
                if let (Some(addr), Some(types)) = (variable.addr, &variable.type_name) {
                    found_vars.append(&mut check_variable_recursive(
                        mapping,
                        &mut self.additional_loaded_sections,
                        &self.backend_url,
                        addr,
                        0,
                        types.clone(),
                        false,
                    ));
                }
            }
        }
        push_variables(&found_vars, &mut self.graph);
        let response = self
            .graph
            .render(ui, ui.available_width(), ui.available_height());
        (false, response)
    }
}

fn push_variables(
    vars: &Vec<(u64, Vec<u64>, usize, DataType)>,
    graph: &mut Graph<VariableNodeData>,
) {
    let mut did_add = false;
    for (addr, refs, typeid, types) in vars {
        if let Some(node) = graph
            .nodes
            .iter_mut()
            .find(|node| node.id == *addr as usize)
        {
            node.connections = refs.iter().map(|i| *i as usize).collect();
        } else {
            did_add = true;
            graph.nodes.push(Node::new(
                *addr as usize,
                refs.iter().map(|i| *i as usize).collect(),
                VariableNodeData {
                    types: types.clone(),
                    typeid: *typeid,
                    addr: *addr,
                },
            ));
        }
    }
    if did_add {
        graph.arrange();
    }
}

fn read_value(memory: &Vec<u8>, offset: usize) -> u64 {
    let value = &memory[offset..offset + 8];
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

/// Search Mode specifies if found types should be reported or just search for references which will then report types again
/// Return Type: (addr, [references], type_index, types)
fn check_variable_recursive(
    mapping: &Vec<MemoryMap>,
    sections: &mut Vec<Section>,
    backend_url: &Url,
    addr: u64,
    type_index: usize,
    types: DataType,
    search_mode: bool,
) -> Vec<(u64, Vec<u64>, usize, DataType)> {
    let size = get_byte_size(&types, type_index);
    if let Some(section) = sections
        .iter()
        .find(|(start, end, _)| addr >= *start && addr + size as u64 - 1 <= *end)
    {
        if let Some(Ok(memory)) = section.2.ready() {
            match &types.0[type_index].1 {
                stackium_shared::TypeName::Name { name, byte_size } => {
                    if !search_mode {
                        return vec![(addr, vec![], type_index, types.clone())];
                    } else {
                        return vec![];
                    }
                }
                stackium_shared::TypeName::Arr { arr_type, count } => {
                    todo!()
                }
                stackium_shared::TypeName::Ref { index } => {
                    let mut ret_val = vec![];
                    let value = read_value(memory, addr as usize - section.0 as usize);
                    if !search_mode {
                        ret_val.push((addr, vec![value], type_index, types.clone()));
                    }
                    if let Some(index) = index {
                        ret_val.append(&mut check_variable_recursive(
                            mapping,
                            sections,
                            backend_url,
                            value,
                            *index,
                            types,
                            false,
                        ));
                    }
                    return ret_val;
                }
                stackium_shared::TypeName::ProductType {
                    name,
                    members,
                    byte_size,
                } => {
                    let mut ret_val = vec![];
                    let mut refs = vec![];
                    for (_, prod_type_offset, offset) in members.iter() {
                        let mut a = check_variable_recursive(
                            mapping,
                            sections,
                            backend_url,
                            addr + *offset as u64,
                            *prod_type_offset,
                            types.clone(),
                            true,
                        );
                        if let Some(first) = a.iter().last() {
                            refs.push(first.0);
                        }
                        ret_val.append(&mut a);
                    }
                    if !search_mode {
                        ret_val.push((addr, refs, type_index, types.clone()));
                    }
                    return ret_val;
                }
            }
        } else {
            return vec![];
        }
    } else {
        if mapping
            .iter()
            .any(|m| m.from <= addr && addr + size as u64 <= m.to)
        {
            sections.push((
                addr,
                addr + size as u64 - 1,
                dispatch!(
                    backend_url.clone(),
                    Command::ReadMemory(addr, size as u64),
                    Memory
                ),
            ));
        }
        return vec![];
    }
}
