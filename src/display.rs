use egui::{Pos2, Rect, Vec2};
use image::{GenericImage, GenericImageView, Rgba, RgbaImage};

pub const MAX_OUTPUT_PIXELS: u64 = 100_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayGeometry {
    pub session_index: usize,
    pub logical_bounds: Rect,
    pub pixel_size: (u32, u32),
    pub pixel_scale: Vec2,
}

impl DisplayGeometry {
    pub fn new(
        session_index: usize,
        logical_bounds: Rect,
        pixel_size: (u32, u32),
    ) -> Result<Self, String> {
        let finite = logical_bounds.min.x.is_finite()
            && logical_bounds.min.y.is_finite()
            && logical_bounds.max.x.is_finite()
            && logical_bounds.max.y.is_finite();
        if !finite || logical_bounds.width() <= 0.0 || logical_bounds.height() <= 0.0 {
            return Err("显示器逻辑区域无效".to_string());
        }
        if pixel_size.0 == 0 || pixel_size.1 == 0 {
            return Err("显示器像素尺寸无效".to_string());
        }

        Ok(Self {
            session_index,
            logical_bounds,
            pixel_size,
            pixel_scale: Vec2::new(
                pixel_size.0 as f32 / logical_bounds.width(),
                pixel_size.1 as f32 / logical_bounds.height(),
            ),
        })
    }

    pub fn global_to_local_rect(&self, global_rect: Rect) -> Rect {
        Rect::from_min_max(
            global_rect.min - self.logical_bounds.min.to_vec2(),
            global_rect.max - self.logical_bounds.min.to_vec2(),
        )
    }

    pub fn logical_intersection_to_pixels(&self, global_rect: Rect) -> Option<PixelRect> {
        let intersection = self.logical_bounds.intersect(global_rect);
        if intersection.width() <= 0.0 || intersection.height() <= 0.0 {
            return None;
        }

        let local_min = intersection.min - self.logical_bounds.min;
        let local_max = intersection.max - self.logical_bounds.min;
        let min_x = (local_min.x * self.pixel_scale.x)
            .floor()
            .clamp(0.0, self.pixel_size.0 as f32) as u32;
        let min_y = (local_min.y * self.pixel_scale.y)
            .floor()
            .clamp(0.0, self.pixel_size.1 as f32) as u32;
        let max_x = (local_max.x * self.pixel_scale.x)
            .ceil()
            .clamp(0.0, self.pixel_size.0 as f32) as u32;
        let max_y = (local_max.y * self.pixel_scale.y)
            .ceil()
            .clamp(0.0, self.pixel_size.1 as f32) as u32;

        (max_x > min_x && max_y > min_y).then_some(PixelRect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        })
    }
}

#[derive(Clone)]
pub struct DisplayTile {
    pub pixel_rect: PixelRect,
    pub image: RgbaImage,
}

#[derive(Clone)]
pub struct CapturedDisplay {
    pub geometry: DisplayGeometry,
    pub original_image: RgbaImage,
    pub tiles: Vec<DisplayTile>,
}

impl CapturedDisplay {
    pub fn from_image(
        geometry: DisplayGeometry,
        image: RgbaImage,
        max_tile_size: u32,
    ) -> Result<Self, String> {
        if image.dimensions() != geometry.pixel_size {
            return Err("显示器截图尺寸与显示器像素尺寸不一致".to_string());
        }
        if max_tile_size == 0 {
            return Err("纹理分块尺寸必须大于零".to_string());
        }

        let tiles = tile_rects(geometry.pixel_size, max_tile_size)
            .into_iter()
            .map(|pixel_rect| DisplayTile {
                image: image
                    .view(
                        pixel_rect.x,
                        pixel_rect.y,
                        pixel_rect.width,
                        pixel_rect.height,
                    )
                    .to_image(),
                pixel_rect,
            })
            .collect();

        Ok(Self {
            geometry,
            original_image: image,
            tiles,
        })
    }

    #[cfg(test)]
    fn blank(geometry: DisplayGeometry) -> Self {
        let image = RgbaImage::new(geometry.pixel_size.0, geometry.pixel_size.1);
        Self::from_image(geometry, image, 2048).unwrap()
    }
}

pub struct CaptureSession {
    pub displays: Vec<CapturedDisplay>,
    pub desktop_bounds: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputTransform {
    pub selection: Rect,
    pub scale: Vec2,
}

impl OutputTransform {
    pub fn global_to_output(&self, point: Pos2) -> Pos2 {
        let local = point - self.selection.min;
        Pos2::new(local.x * self.scale.x, local.y * self.scale.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompositionPiece {
    pub display_index: usize,
    pub source: PixelRect,
    pub destination: PixelRect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositionPlan {
    pub transform: OutputTransform,
    pub output_size: (u32, u32),
    pub pieces: Vec<CompositionPiece>,
}

pub struct ComposedSelection {
    pub image: RgbaImage,
    pub transform: OutputTransform,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolbarPlacement {
    pub display_index: usize,
    pub global_position: Pos2,
}

pub fn place_toolbar(
    displays: &[DisplayGeometry],
    selection: Rect,
    endpoint: Pos2,
    toolbar_size: Vec2,
    margin: f32,
) -> Result<ToolbarPlacement, String> {
    if displays.is_empty() {
        return Err("没有可用于放置工具栏的显示器".to_string());
    }
    if toolbar_size.x <= 0.0 || toolbar_size.y <= 0.0 {
        return Err("工具栏尺寸必须大于零".to_string());
    }
    let (display_index, display) = displays
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            distance_squared_to_rect(endpoint, left.logical_bounds)
                .total_cmp(&distance_squared_to_rect(endpoint, right.logical_bounds))
        })
        .expect("non-empty displays checked above");
    let bounds = display.logical_bounds;
    let max_x = (bounds.max.x - toolbar_size.x).max(bounds.min.x);
    let x = (selection.center().x - toolbar_size.x * 0.5).clamp(bounds.min.x, max_x);
    let below = selection.max.y + margin;
    let above = selection.min.y - toolbar_size.y - margin;
    let preferred_y = if below + toolbar_size.y <= bounds.max.y {
        below
    } else {
        above
    };
    let max_y = (bounds.max.y - toolbar_size.y).max(bounds.min.y);
    let y = preferred_y.clamp(bounds.min.y, max_y);

    Ok(ToolbarPlacement {
        display_index,
        global_position: Pos2::new(x, y),
    })
}

impl CaptureSession {
    pub fn new(displays: Vec<CapturedDisplay>) -> Result<Self, String> {
        let mut display_iter = displays.iter();
        let first = display_iter
            .next()
            .ok_or_else(|| "未检测到可截图的显示器".to_string())?;
        let desktop_bounds = display_iter.fold(first.geometry.logical_bounds, |bounds, display| {
            bounds.union(display.geometry.logical_bounds)
        });

        Ok(Self {
            displays,
            desktop_bounds,
        })
    }

    pub fn display_at(&self, point: Pos2) -> Option<&CapturedDisplay> {
        self.displays
            .iter()
            .find(|display| display.geometry.logical_bounds.contains(point))
            .or_else(|| {
                self.displays.iter().min_by(|left, right| {
                    distance_squared_to_rect(point, left.geometry.logical_bounds).total_cmp(
                        &distance_squared_to_rect(point, right.geometry.logical_bounds),
                    )
                })
            })
    }
}

pub fn tile_rects(pixel_size: (u32, u32), max_tile_size: u32) -> Vec<PixelRect> {
    if pixel_size.0 == 0 || pixel_size.1 == 0 || max_tile_size == 0 {
        return Vec::new();
    }

    let mut tiles = Vec::new();
    for y in (0..pixel_size.1).step_by(max_tile_size as usize) {
        for x in (0..pixel_size.0).step_by(max_tile_size as usize) {
            tiles.push(PixelRect {
                x,
                y,
                width: (pixel_size.0 - x).min(max_tile_size),
                height: (pixel_size.1 - y).min(max_tile_size),
            });
        }
    }
    tiles
}

pub fn plan_composition(
    displays: &[DisplayGeometry],
    selection: Rect,
) -> Result<CompositionPlan, String> {
    let selection = normalize_rect(selection);
    if selection.width() <= 0.0 || selection.height() <= 0.0 {
        return Err("截图区域不能为空".to_string());
    }

    let intersecting: Vec<(usize, &DisplayGeometry, Rect)> = displays
        .iter()
        .enumerate()
        .filter_map(|(display_index, display)| {
            let intersection = display.logical_bounds.intersect(selection);
            (intersection.width() > 0.0 && intersection.height() > 0.0).then_some((
                display_index,
                display,
                intersection,
            ))
        })
        .collect();
    if intersecting.is_empty() {
        return Err("截图区域未覆盖任何显示器".to_string());
    }

    let scale = intersecting
        .iter()
        .fold(Vec2::ZERO, |scale, (_, display, _)| {
            Vec2::new(
                scale.x.max(display.pixel_scale.x),
                scale.y.max(display.pixel_scale.y),
            )
        });
    let output_width = (selection.width() * scale.x).ceil() as u32;
    let output_height = (selection.height() * scale.y).ceil() as u32;
    let output_pixels = u64::from(output_width)
        .checked_mul(u64::from(output_height))
        .ok_or_else(|| "截图输出尺寸溢出".to_string())?;
    if output_pixels > MAX_OUTPUT_PIXELS {
        return Err(format!(
            "截图输出超过像素上限 {MAX_OUTPUT_PIXELS}，请缩小选择区域"
        ));
    }

    let pieces = intersecting
        .into_iter()
        .map(|(display_index, display, intersection)| {
            let source = display
                .logical_intersection_to_pixels(intersection)
                .ok_or_else(|| "无法计算显示器裁剪区域".to_string())?;
            let local_min = intersection.min - selection.min;
            let local_max = intersection.max - selection.min;
            let min_x = (local_min.x * scale.x)
                .floor()
                .clamp(0.0, output_width as f32) as u32;
            let min_y = (local_min.y * scale.y)
                .floor()
                .clamp(0.0, output_height as f32) as u32;
            let max_x = (local_max.x * scale.x)
                .ceil()
                .clamp(0.0, output_width as f32) as u32;
            let max_y = (local_max.y * scale.y)
                .ceil()
                .clamp(0.0, output_height as f32) as u32;
            if max_x <= min_x || max_y <= min_y {
                return Err("无法计算显示器合成区域".to_string());
            }
            Ok(CompositionPiece {
                display_index,
                source,
                destination: PixelRect {
                    x: min_x,
                    y: min_y,
                    width: max_x - min_x,
                    height: max_y - min_y,
                },
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(CompositionPlan {
        transform: OutputTransform { selection, scale },
        output_size: (output_width, output_height),
        pieces,
    })
}

pub fn compose_selection(
    displays: &[CapturedDisplay],
    selection: Rect,
) -> Result<ComposedSelection, String> {
    let geometries: Vec<_> = displays
        .iter()
        .map(|display| display.geometry.clone())
        .collect();
    let plan = plan_composition(&geometries, selection)?;
    let mut output =
        RgbaImage::from_pixel(plan.output_size.0, plan.output_size.1, Rgba([0, 0, 0, 0]));

    for piece in &plan.pieces {
        let display = displays
            .get(piece.display_index)
            .ok_or_else(|| "显示器合成索引无效".to_string())?;
        let source = display
            .original_image
            .view(
                piece.source.x,
                piece.source.y,
                piece.source.width,
                piece.source.height,
            )
            .to_image();
        let source = if source.dimensions() == (piece.destination.width, piece.destination.height) {
            source
        } else {
            image::imageops::resize(
                &source,
                piece.destination.width,
                piece.destination.height,
                image::imageops::FilterType::Lanczos3,
            )
        };
        output
            .copy_from(&source, piece.destination.x, piece.destination.y)
            .map_err(|error| format!("合成显示器截图失败: {error}"))?;
    }

    Ok(ComposedSelection {
        image: output,
        transform: plan.transform,
    })
}

fn normalize_rect(rect: Rect) -> Rect {
    Rect::from_min_max(
        Pos2::new(rect.min.x.min(rect.max.x), rect.min.y.min(rect.max.y)),
        Pos2::new(rect.min.x.max(rect.max.x), rect.min.y.max(rect.max.y)),
    )
}

fn distance_squared_to_rect(point: Pos2, rect: Rect) -> f32 {
    let dx = if point.x < rect.min.x {
        rect.min.x - point.x
    } else if point.x > rect.max.x {
        point.x - rect.max.x
    } else {
        0.0
    };
    let dy = if point.y < rect.min.y {
        rect.min.y - point.y
    } else if point.y > rect.max.y {
        point.y - rect.max.y
    } else {
        0.0
    };
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(index: usize, bounds: Rect, pixels: (u32, u32)) -> DisplayGeometry {
        DisplayGeometry::new(index, bounds, pixels).unwrap()
    }

    fn solid_display(
        index: usize,
        bounds: (f32, f32, f32, f32),
        pixels: (u32, u32),
        color: [u8; 4],
    ) -> CapturedDisplay {
        let geometry = geometry(
            index,
            Rect::from_min_size(Pos2::new(bounds.0, bounds.1), Vec2::new(bounds.2, bounds.3)),
            pixels,
        );
        let image = RgbaImage::from_pixel(pixels.0, pixels.1, image::Rgba(color));
        CapturedDisplay::from_image(geometry, image, 2048).unwrap()
    }

    #[test]
    fn maps_negative_origin_global_selection_to_native_pixels() {
        let display = geometry(
            0,
            Rect::from_min_size(Pos2::new(-1920.0, -200.0), Vec2::new(1920.0, 1080.0)),
            (1920, 1080),
        );

        assert_eq!(
            display.logical_intersection_to_pixels(Rect::from_min_max(
                Pos2::new(-1820.0, -150.0),
                Pos2::new(-1780.0, -120.0),
            )),
            Some(PixelRect {
                x: 100,
                y: 50,
                width: 40,
                height: 30,
            })
        );
    }

    #[test]
    fn retina_mapping_floors_min_and_ceils_max_edges() {
        let display = geometry(
            0,
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1512.0, 982.0)),
            (3024, 1964),
        );

        assert_eq!(
            display.logical_intersection_to_pixels(Rect::from_min_max(
                Pos2::new(300.25, 400.25),
                Pos2::new(985.25, 607.25),
            )),
            Some(PixelRect {
                x: 600,
                y: 800,
                width: 1371,
                height: 415,
            })
        );
    }

    #[test]
    fn desktop_bounds_include_left_and_upper_displays() {
        let displays = vec![
            CapturedDisplay::blank(geometry(
                0,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0)),
                (1920, 1080),
            )),
            CapturedDisplay::blank(geometry(
                1,
                Rect::from_min_size(Pos2::new(-1280.0, -1024.0), Vec2::new(1280.0, 1024.0)),
                (1280, 1024),
            )),
        ];

        let session = CaptureSession::new(displays).unwrap();

        assert_eq!(session.desktop_bounds.min, Pos2::new(-1280.0, -1024.0));
        assert_eq!(session.desktop_bounds.max, Pos2::new(1920.0, 1080.0));
    }

    #[test]
    fn tiles_one_display_without_reusing_another_display_identity() {
        assert_eq!(
            tile_rects((4097, 2050), 2048),
            vec![
                PixelRect {
                    x: 0,
                    y: 0,
                    width: 2048,
                    height: 2048,
                },
                PixelRect {
                    x: 2048,
                    y: 0,
                    width: 2048,
                    height: 2048,
                },
                PixelRect {
                    x: 4096,
                    y: 0,
                    width: 1,
                    height: 2048,
                },
                PixelRect {
                    x: 0,
                    y: 2048,
                    width: 2048,
                    height: 2,
                },
                PixelRect {
                    x: 2048,
                    y: 2048,
                    width: 2048,
                    height: 2,
                },
                PixelRect {
                    x: 4096,
                    y: 2048,
                    width: 1,
                    height: 2,
                },
            ]
        );
    }

    #[test]
    fn rejects_invalid_display_geometry() {
        assert!(
            DisplayGeometry::new(
                0,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(0.0, 100.0)),
                (100, 100),
            )
            .is_err()
        );
        assert!(
            DisplayGeometry::new(
                0,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0)),
                (0, 100),
            )
            .is_err()
        );
    }

    #[test]
    fn global_rect_converts_to_viewport_local_rect() {
        let display = geometry(
            0,
            Rect::from_min_size(Pos2::new(-500.0, 200.0), Vec2::new(500.0, 300.0)),
            (500, 300),
        );

        assert_eq!(
            display.global_to_local_rect(Rect::from_min_max(
                Pos2::new(-450.0, 225.0),
                Pos2::new(-400.0, 275.0),
            )),
            Rect::from_min_max(Pos2::new(50.0, 25.0), Pos2::new(100.0, 75.0))
        );
    }

    #[test]
    fn display_at_chooses_containing_then_nearest_display() {
        let session = CaptureSession::new(vec![
            CapturedDisplay::blank(geometry(
                0,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0)),
                (100, 100),
            )),
            CapturedDisplay::blank(geometry(
                1,
                Rect::from_min_size(Pos2::new(200.0, 0.0), Vec2::new(100.0, 100.0)),
                (100, 100),
            )),
        ])
        .unwrap();

        assert_eq!(
            session
                .display_at(Pos2::new(20.0, 20.0))
                .unwrap()
                .geometry
                .session_index,
            0
        );
        assert_eq!(
            session
                .display_at(Pos2::new(180.0, 20.0))
                .unwrap()
                .geometry
                .session_index,
            1
        );
    }

    #[test]
    fn composition_keeps_monitor_gap_transparent() {
        let left = solid_display(0, (0.0, 0.0, 100.0, 100.0), (100, 100), [255, 0, 0, 255]);
        let right = solid_display(1, (150.0, 0.0, 100.0, 100.0), (100, 100), [0, 0, 255, 255]);

        let composed = compose_selection(
            &[left, right],
            Rect::from_min_size(Pos2::ZERO, Vec2::new(250.0, 100.0)),
        )
        .unwrap();

        assert_eq!(composed.image.dimensions(), (250, 100));
        assert_eq!(composed.image.get_pixel(10, 10).0, [255, 0, 0, 255]);
        assert_eq!(composed.image.get_pixel(125, 10).0, [0, 0, 0, 0]);
        assert_eq!(composed.image.get_pixel(200, 10).0, [0, 0, 255, 255]);
    }

    #[test]
    fn composition_uses_highest_scale_without_changing_apparent_size() {
        let normal = solid_display(0, (0.0, 0.0, 100.0, 100.0), (100, 100), [255, 0, 0, 255]);
        let retina = solid_display(1, (100.0, 0.0, 100.0, 100.0), (200, 200), [0, 255, 0, 255]);

        let composed = compose_selection(
            &[normal, retina],
            Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 100.0)),
        )
        .unwrap();

        assert_eq!(composed.image.dimensions(), (400, 200));
        assert_eq!(composed.transform.scale, Vec2::new(2.0, 2.0));
        assert_eq!(
            composed.transform.global_to_output(Pos2::new(125.0, 25.0)),
            Pos2::new(250.0, 50.0)
        );
        assert_eq!(composed.image.get_pixel(100, 100).0, [255, 0, 0, 255]);
        assert_eq!(composed.image.get_pixel(300, 100).0, [0, 255, 0, 255]);
    }

    #[test]
    fn composition_rejects_empty_selection_and_no_display_intersection() {
        let display = solid_display(0, (0.0, 0.0, 100.0, 100.0), (100, 100), [0, 0, 0, 255]);

        assert!(compose_selection(std::slice::from_ref(&display), Rect::ZERO).is_err());
        assert!(
            compose_selection(
                &[display],
                Rect::from_min_size(Pos2::new(200.0, 200.0), Vec2::new(10.0, 10.0)),
            )
            .is_err()
        );
    }

    #[test]
    fn composition_plan_rejects_oversized_output_without_allocating_it() {
        let display = geometry(
            0,
            Rect::from_min_size(Pos2::ZERO, Vec2::new(20_000.0, 20_000.0)),
            (20_000, 20_000),
        );

        let error = plan_composition(
            &[display],
            Rect::from_min_size(Pos2::ZERO, Vec2::new(20_000.0, 20_000.0)),
        )
        .unwrap_err();

        assert!(error.contains("100000000"));
    }

    #[test]
    fn toolbar_uses_release_monitor_and_flips_above_at_bottom_edge() {
        let displays = vec![
            geometry(
                0,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0)),
                (1000, 800),
            ),
            geometry(
                1,
                Rect::from_min_size(Pos2::new(1000.0, 0.0), Vec2::new(1000.0, 800.0)),
                (1000, 800),
            ),
        ];
        let placement = super::place_toolbar(
            &displays,
            Rect::from_min_max(Pos2::new(900.0, 600.0), Pos2::new(1800.0, 790.0)),
            Pos2::new(1700.0, 790.0),
            Vec2::new(300.0, 50.0),
            8.0,
        )
        .unwrap();

        assert_eq!(placement.display_index, 1);
        assert!(placement.global_position.y < 600.0);
        assert!(placement.global_position.x >= 1000.0);
        assert!(placement.global_position.x + 300.0 <= 2000.0);
    }

    #[test]
    fn toolbar_chooses_nearest_display_when_endpoint_is_in_gap() {
        let displays = vec![
            geometry(
                0,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0)),
                (100, 100),
            ),
            geometry(
                1,
                Rect::from_min_size(Pos2::new(200.0, 0.0), Vec2::new(100.0, 100.0)),
                (100, 100),
            ),
        ];

        assert_eq!(
            super::place_toolbar(
                &displays,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(250.0, 80.0)),
                Pos2::new(180.0, 50.0),
                Vec2::new(50.0, 20.0),
                4.0,
            )
            .unwrap()
            .display_index,
            1
        );
    }
}
