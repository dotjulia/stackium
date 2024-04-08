use egui::{epaint::TextShape, Color32, FontId, Id, Pos2, Shape, Stroke, Ui};
use egui_plot::{PlotItem, PlotPoint};

pub struct RotText {
    text: String,
    angle: f32,
    size: f32,
    pos: (f32, f32),
    color: Option<Color32>,
}

impl RotText {
    pub fn new(text: String, angle: f32, size: f32, pos: (f32, f32), color: Option<Color32>) -> Self {
        Self { text, angle, size, pos, color }
    }
}

impl PlotItem for RotText {
    fn shapes(&self, ui: &Ui, transform: &egui_plot::PlotTransform, shapes: &mut Vec<Shape>) {
        let galley = ui.painter().layout(
            self.text.clone(),
            FontId {
                size: self.size,
                family: egui::FontFamily::Monospace,
            },
            egui::Color32::WHITE,
            f32::INFINITY,
        );
        let pos = transform.position_from_point(&PlotPoint::new(self.pos.0, self.pos.1));
        shapes.push(Shape::Text(TextShape {
            pos,
            galley,
            underline: Stroke::NONE,
            fallback_color: self.color.unwrap_or_else(|| ui.visuals().text_color()),
            override_text_color: Some(self.color.unwrap_or_else(|| ui.visuals().text_color())),
            opacity_factor: 1.0,
            angle: self.angle,
        }));
    }

    fn initialize(&mut self, x_range: std::ops::RangeInclusive<f64>) {
        
    }

    fn name(&self) -> &str {
        "Rotated Text"
    }

    fn color(&self) -> Color32 {
        self.color.unwrap_or_default()
    }

    fn highlight(&mut self) {
        todo!()
    }

    fn highlighted(&self) -> bool {
        false
    }

    fn geometry(&self) -> egui_plot::PlotGeometry<'_> {
        egui_plot::PlotGeometry::None
    }

    fn bounds(&self) -> egui_plot::PlotBounds {
        let mut bounds = egui_plot::PlotBounds::NOTHING;
        bounds.extend_with(&PlotPoint::new(self.pos.0 as f64, self.pos.1 as f64));
        bounds
    }

    fn id(&self) -> Option<Id> {
        None
    }
}
