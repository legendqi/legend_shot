use eframe::egui;
use std::collections::VecDeque;
use egui::StrokeKind;

#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
    Select,
    Brush,
    Rectangle,
    Arrow,
    Text,
}

#[derive(Clone, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug)]
pub enum Annotation {
    Rectangle { start: Point, end: Point },
    Arrow { start: Point, end: Point },
    Freehand { points: Vec<Point> },
    Text { position: Point, text: String },
}

pub struct AnnotationTool {
    pub current_tool: ToolType,
    pub annotations: VecDeque<Annotation>,
    pub color: egui::Color32,
    pub stroke_width: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ToolType {
    Rectangle,
    Arrow,
    Brush,
    Text,
}

impl Default for AnnotationTool {
    fn default() -> Self {
        Self {
            current_tool: ToolType::Rectangle,
            annotations: VecDeque::new(),
            color: egui::Color32::RED,
            stroke_width: 2.0,
        }
    }
}

impl AnnotationTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_tool(&mut self, tool: ToolType) {
        self.current_tool = tool;
    }

    pub fn add_annotation(&mut self, annotation: Annotation) {
        self.annotations.push_back(annotation);
    }

    pub fn clear(&mut self) {
        self.annotations.clear();
    }

    pub fn draw(&self, ui: &mut egui::Ui, scale: f32) {
        let painter = ui.painter();

        for annotation in &self.annotations {
            match annotation {
                Annotation::Rectangle { start, end } => {
                    let rect = egui::Rect::from_min_max(
                        egui::pos2(start.x * scale, start.y * scale),
                        egui::pos2(end.x * scale, end.y * scale),
                    );
                    painter.rect_stroke(rect, 0.0, (self.stroke_width, self.color), StrokeKind::Middle);
                }
                Annotation::Arrow { start, end } => {
                    self.draw_arrow(painter, start, end, scale);
                }
                Annotation::Freehand { points } => {
                    if points.len() > 1 {
                        let path: Vec<egui::Pos2> = points
                            .iter()
                            .map(|p| egui::pos2(p.x * scale, p.y * scale))
                            .collect();
                        painter.add(egui::Shape::line(
                            path,
                            egui::Stroke::new(self.stroke_width, self.color),
                        ));
                    }
                }
                Annotation::Text { position, text } => {
                    painter.text(
                        egui::pos2(position.x * scale, position.y * scale),
                        egui::Align2::LEFT_TOP,
                        text,
                        egui::FontId::default(),
                        self.color,
                    );
                }
            }
        }
    }

    fn draw_arrow(&self, painter: &egui::Painter, start: &Point, end: &Point, scale: f32) {
        let start_pos = egui::pos2(start.x * scale, start.y * scale);
        let end_pos = egui::pos2(end.x * scale, end.y * scale);

        // 绘制主线
        painter.line_segment([start_pos, end_pos], (self.stroke_width, self.color));

        // 绘制箭头头部
        let angle = (end_pos - start_pos).angle();
        let arrow_length = 10.0 * scale;
        let arrow_angle = std::f32::consts::FRAC_PI_6;

        let arrow1 = end_pos - egui::Vec2::angled(angle - arrow_angle) * arrow_length;
        let arrow2 = end_pos - egui::Vec2::angled(angle + arrow_angle) * arrow_length;

        painter.line_segment([end_pos, arrow1], (self.stroke_width, self.color));
        painter.line_segment([end_pos, arrow2], (self.stroke_width, self.color));
    }
}