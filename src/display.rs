use egui::{Pos2, Rect, Vec2};
use image::{GenericImageView, RgbaImage};

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
                    distance_squared_to_rect(point, left.geometry.logical_bounds)
                        .total_cmp(&distance_squared_to_rect(
                            point,
                            right.geometry.logical_bounds,
                        ))
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

    #[test]
    fn maps_negative_origin_global_selection_to_native_pixels() {
        let display = geometry(
            0,
            Rect::from_min_size(
                Pos2::new(-1920.0, -200.0),
                Vec2::new(1920.0, 1080.0),
            ),
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
                Rect::from_min_size(
                    Pos2::new(-1280.0, -1024.0),
                    Vec2::new(1280.0, 1024.0),
                ),
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
            session.display_at(Pos2::new(20.0, 20.0)).unwrap().geometry.session_index,
            0
        );
        assert_eq!(
            session.display_at(Pos2::new(180.0, 20.0)).unwrap().geometry.session_index,
            1
        );
    }
}
