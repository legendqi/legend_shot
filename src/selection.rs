use egui::{Color32, CursorIcon, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};

use crate::app_default::{ScreenshotApp, Tool};
use crate::display::{CapturedDisplay, plan_composition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

impl ResizeHandle {
    pub fn cursor(self) -> CursorIcon {
        match self {
            Self::North | Self::South => CursorIcon::ResizeVertical,
            Self::East | Self::West => CursorIcon::ResizeHorizontal,
            Self::NorthWest | Self::SouthEast => CursorIcon::ResizeNwSe,
            Self::NorthEast | Self::SouthWest => CursorIcon::ResizeNeSw,
        }
    }
}

fn handle_positions(rect: Rect) -> [(ResizeHandle, Pos2); 8] {
    [
        (ResizeHandle::NorthWest, rect.left_top()),
        (ResizeHandle::NorthEast, rect.right_top()),
        (ResizeHandle::SouthEast, rect.right_bottom()),
        (ResizeHandle::SouthWest, rect.left_bottom()),
        (ResizeHandle::North, rect.center_top()),
        (ResizeHandle::East, rect.right_center()),
        (ResizeHandle::South, rect.center_bottom()),
        (ResizeHandle::West, rect.left_center()),
    ]
}

pub fn hit_test_handle(rect: Rect, point: Pos2, tolerance: f32) -> Option<ResizeHandle> {
    handle_positions(rect)
        .into_iter()
        .filter(|(_, position)| {
            (position.x - point.x).abs() <= tolerance && (position.y - point.y).abs() <= tolerance
        })
        .min_by(|(_, left), (_, right)| {
            left.distance_sq(point).total_cmp(&right.distance_sq(point))
        })
        .map(|(handle, _)| handle)
}

/// Resize from the drag's original rectangle. The opposite edges stay anchored.
pub fn resize_selection(original: Rect, handle: ResizeHandle, delta: Vec2, bounds: Rect) -> Rect {
    let mut rect = bounded_selection(original, bounds);
    let min_width = 1.0_f32.min(bounds.width());
    let min_height = 1.0_f32.min(bounds.height());
    if matches!(
        handle,
        ResizeHandle::NorthWest | ResizeHandle::West | ResizeHandle::SouthWest
    ) {
        rect.min.x = (rect.min.x + delta.x).clamp(bounds.min.x, rect.max.x - min_width);
    }
    if matches!(
        handle,
        ResizeHandle::NorthEast | ResizeHandle::East | ResizeHandle::SouthEast
    ) {
        rect.max.x = (rect.max.x + delta.x).clamp(rect.min.x + min_width, bounds.max.x);
    }
    if matches!(
        handle,
        ResizeHandle::NorthWest | ResizeHandle::North | ResizeHandle::NorthEast
    ) {
        rect.min.y = (rect.min.y + delta.y).clamp(bounds.min.y, rect.max.y - min_height);
    }
    if matches!(
        handle,
        ResizeHandle::SouthWest | ResizeHandle::South | ResizeHandle::SouthEast
    ) {
        rect.max.y = (rect.max.y + delta.y).clamp(rect.min.y + min_height, bounds.max.y);
    }
    rect
}

/// Arrow movement uses logical pixels; resize mode changes the right/bottom edges.
pub fn nudge_selection(rect: Rect, delta: Vec2, resize: bool, bounds: Rect) -> Rect {
    if resize {
        return resize_selection(rect, ResizeHandle::SouthEast, delta, bounds);
    }
    let rect = bounded_selection(rect, bounds);
    rect.translate(Vec2::new(
        delta
            .x
            .clamp(bounds.min.x - rect.min.x, bounds.max.x - rect.max.x),
        delta
            .y
            .clamp(bounds.min.y - rect.min.y, bounds.max.y - rect.max.y),
    ))
}

fn bounded_selection(rect: Rect, bounds: Rect) -> Rect {
    let size = Vec2::new(
        rect.width()
            .clamp(1.0_f32.min(bounds.width()), bounds.width()),
        rect.height()
            .clamp(1.0_f32.min(bounds.height()), bounds.height()),
    );
    Rect::from_min_size(
        Pos2::new(
            rect.min.x.clamp(bounds.min.x, bounds.max.x - size.x),
            rect.min.y.clamp(bounds.min.y, bounds.max.y - size.y),
        ),
        size,
    )
}

/// Editor-only decoration: never changes annotation clipping or capture pixels.
pub fn draw_selection_aids(app: &ScreenshotApp, display_index: usize, ui: &mut egui::Ui) {
    let Some(session) = app.capture_session.as_ref() else {
        return;
    };
    let Some(display) = session.displays.get(display_index) else {
        return;
    };
    let bounds = display.geometry.logical_bounds;
    let origin = bounds.min.to_vec2();
    let local_bounds = Rect::from_min_size(Pos2::ZERO, bounds.size());
    let painter = ui
        .painter()
        .with_clip_rect(ui.clip_rect().intersect(local_bounds));
    let selection_tool = matches!(app.current_tool, Tool::Select | Tool::MoveBox);

    if (app.show_toolbar || app.resize_handle.is_some())
        && selection_tool
        && let Some(selection) = app.selection_rect
    {
        for (_, point) in handle_positions(selection) {
            if bounds.contains(point) {
                painter.rect(
                    Rect::from_center_size(point - origin, Vec2::splat(7.0)),
                    1,
                    Color32::WHITE,
                    Stroke::new(1.0, Color32::from_rgb(40, 115, 255)),
                    StrokeKind::Inside,
                );
            }
        }
    }

    let pointer = app
        .pointer_snapshot
        .map(|snapshot| snapshot.global_position);
    // Half-open bounds make a shared monitor edge belong to exactly one viewport.
    let pointer_display = pointer.and_then(|point| {
        session
            .displays
            .iter()
            .position(|candidate| contains_pixel_point(candidate.geometry.logical_bounds, point))
    });
    if selection_tool
        && let Some(pointer) = pointer
        && pointer_display == Some(display_index)
        && !app
            .toolbar_rect_global
            .filter(|_| app.show_toolbar)
            .is_some_and(|rect| rect.contains(pointer))
        && let Some(handle) = app.resize_handle.or_else(|| {
            app.selection_rect
                .filter(|_| app.show_toolbar)
                .and_then(|rect| hit_test_handle(rect, pointer, 6.0))
        })
    {
        ui.ctx().set_cursor_icon(handle.cursor());
    }
    let mut obstacles = Vec::new();
    if app.show_toolbar
        && let Some(toolbar) = app.toolbar_rect_global
    {
        obstacles.push(toolbar.expand(6.0));
    }
    if (app.is_selecting || selection_tool)
        && pointer_display == Some(display_index)
        && let Some(pointer) = pointer
        && !obstacles.iter().any(|rect| rect.contains(pointer))
        && let Some(panel) = draw_magnifier(display, pointer, &painter, &obstacles)
    {
        obstacles.push(panel.expand(6.0));
    }

    let dimensions_display = app
        .toolbar_placement
        .filter(|_| app.show_toolbar)
        .map(|placement| placement.display_index)
        .or(pointer_display);
    if dimensions_display != Some(display_index) {
        return;
    }
    let Some(selection) = app.selection_rect.filter(|rect| rect.area() > 0.0) else {
        return;
    };
    let geometries = session
        .displays
        .iter()
        .map(|display| display.geometry.clone())
        .collect::<Vec<_>>();
    let output = match plan_composition(&geometries, selection) {
        Ok(plan) => format!("{} × {} px", plan.output_size.0, plan.output_size.1),
        Err(_) => "无法输出当前选区".to_string(),
    };
    let text = format!(
        "{:.1} × {:.1} 逻辑 · {output}",
        selection.width(),
        selection.height()
    );
    let galley = painter.layout_no_wrap(text, FontId::proportional(12.0), Color32::WHITE);
    let size = galley.size() + Vec2::new(12.0, 10.0);
    let anchor = selection.intersect(bounds).min;
    if let Some(panel) = place_aid(anchor, size, bounds, &obstacles) {
        let local = panel.translate(-origin);
        painter.rect_filled(local, 4, Color32::from_black_alpha(225));
        painter.galley(local.min + Vec2::new(6.0, 5.0), galley, Color32::WHITE);
    }
}

fn draw_magnifier(
    display: &CapturedDisplay,
    point: Pos2,
    painter: &egui::Painter,
    obstacles: &[Rect],
) -> Option<Rect> {
    const SIDE: i64 = 11;
    const CELL: f32 = 9.0;
    const PADDING: f32 = 6.0;
    let ((pixel_x, pixel_y), _) = sample_original_pixel(display, point)?;
    let grid_size = SIDE as f32 * CELL;
    let panel = place_aid(
        point,
        Vec2::new(grid_size + PADDING * 2.0, grid_size + PADDING * 2.0 + 20.0),
        display.geometry.logical_bounds,
        obstacles,
    )?;
    let local = panel.translate(-display.geometry.logical_bounds.min.to_vec2());
    painter.rect_filled(local, 4, Color32::from_black_alpha(240));
    let grid_min = local.min + Vec2::splat(PADDING);
    let image = &display.original_image;
    for row in 0..SIDE {
        for column in 0..SIDE {
            let x = (i64::from(pixel_x) + column - SIDE / 2).clamp(0, i64::from(image.width()) - 1)
                as u32;
            let y = (i64::from(pixel_y) + row - SIDE / 2).clamp(0, i64::from(image.height()) - 1)
                as u32;
            let pixel = image.get_pixel(x, y);
            painter.rect_filled(
                Rect::from_min_size(
                    grid_min + Vec2::new(column as f32 * CELL, row as f32 * CELL),
                    Vec2::splat(CELL),
                ),
                0,
                Color32::from_rgba_unmultiplied(pixel[0], pixel[1], pixel[2], pixel[3]),
            );
        }
    }
    let center = Rect::from_min_size(
        grid_min + Vec2::splat((SIDE / 2) as f32 * CELL),
        Vec2::splat(CELL),
    );
    painter.rect_stroke(
        center.expand(1.0),
        0,
        Stroke::new(1.0, Color32::BLACK),
        StrokeKind::Outside,
    );
    painter.rect_stroke(
        center,
        0,
        Stroke::new(1.0, Color32::WHITE),
        StrokeKind::Inside,
    );
    painter.text(
        Pos2::new(local.center().x, local.max.y - 10.0),
        egui::Align2::CENTER_CENTER,
        format!("{:.0}, {:.0}", point.x, point.y),
        FontId::monospace(11.0),
        Color32::WHITE,
    );
    Some(panel)
}

fn place_aid(anchor: Pos2, size: Vec2, bounds: Rect, obstacles: &[Rect]) -> Option<Rect> {
    if size.x > bounds.width() || size.y > bounds.height() {
        return None;
    }
    let gap = 18.0;
    let right = anchor.x + gap;
    let left = anchor.x - gap - size.x;
    let below = anchor.y + gap;
    let above = anchor.y - gap - size.y;
    let max = bounds.max - size;
    [
        Pos2::new(right, below),
        Pos2::new(left, below),
        Pos2::new(right, above),
        Pos2::new(left, above),
        bounds.min,
        Pos2::new(max.x, bounds.min.y),
        Pos2::new(bounds.min.x, max.y),
        max,
    ]
    .into_iter()
    .map(|min| {
        Rect::from_min_size(
            Pos2::new(
                min.x.clamp(bounds.min.x, max.x),
                min.y.clamp(bounds.min.y, max.y),
            ),
            size,
        )
    })
    .find(|candidate| {
        !obstacles
            .iter()
            .any(|obstacle| candidate.intersects(*obstacle))
    })
}

fn contains_pixel_point(bounds: Rect, point: Pos2) -> bool {
    point.x >= bounds.min.x
        && point.x < bounds.max.x
        && point.y >= bounds.min.y
        && point.y < bounds.max.y
}

fn sample_original_pixel(display: &CapturedDisplay, point: Pos2) -> Option<((u32, u32), Color32)> {
    if !contains_pixel_point(display.geometry.logical_bounds, point) {
        return None;
    }
    let local = point - display.geometry.logical_bounds.min;
    let x = ((local.x * display.geometry.pixel_scale.x).floor() as u32)
        .min(display.original_image.width() - 1);
    let y = ((local.y * display.geometry.pixel_scale.y).floor() as u32)
        .min(display.original_image.height() - 1);
    let pixel = display.original_image.get_pixel(x, y);
    Some((
        (x, y),
        Color32::from_rgba_unmultiplied(pixel[0], pixel[1], pixel[2], pixel[3]),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
    }

    #[test]
    fn finds_all_eight_handles_on_negative_origin_selection() {
        let selection = rect(-300.0, -200.0, -100.0, -100.0);
        for (point, expected) in [
            ((-300.0, -200.0), ResizeHandle::NorthWest),
            ((-200.0, -200.0), ResizeHandle::North),
            ((-100.0, -200.0), ResizeHandle::NorthEast),
            ((-100.0, -150.0), ResizeHandle::East),
            ((-100.0, -100.0), ResizeHandle::SouthEast),
            ((-200.0, -100.0), ResizeHandle::South),
            ((-300.0, -100.0), ResizeHandle::SouthWest),
            ((-300.0, -150.0), ResizeHandle::West),
        ] {
            assert_eq!(
                hit_test_handle(selection, Pos2::new(point.0, point.1), 6.0),
                Some(expected)
            );
        }
        assert_eq!(
            hit_test_handle(selection, Pos2::new(-200.0, -150.0), 6.0),
            None
        );
        assert_eq!(
            hit_test_handle(selection, Pos2::new(-307.0, -200.0), 6.0),
            None
        );
    }

    #[test]
    fn overlapping_handle_targets_choose_nearest_handle() {
        assert_eq!(
            hit_test_handle(rect(0.0, 0.0, 2.0, 2.0), Pos2::new(2.0, 1.0), 6.0),
            Some(ResizeHandle::East)
        );
    }

    #[test]
    fn resize_cursor_matches_handle_axis() {
        for (handle, expected) in [
            (ResizeHandle::North, CursorIcon::ResizeVertical),
            (ResizeHandle::East, CursorIcon::ResizeHorizontal),
            (ResizeHandle::NorthWest, CursorIcon::ResizeNwSe),
            (ResizeHandle::SouthEast, CursorIcon::ResizeNwSe),
            (ResizeHandle::NorthEast, CursorIcon::ResizeNeSw),
            (ResizeHandle::SouthWest, CursorIcon::ResizeNeSw),
        ] {
            assert_eq!(handle.cursor(), expected);
        }
    }

    #[test]
    fn each_handle_moves_only_its_edges() {
        let selection = rect(10.0, 20.0, 110.0, 120.0);
        let bounds = rect(-100.0, -100.0, 200.0, 200.0);
        for (handle, expected) in [
            (ResizeHandle::NorthWest, rect(13.0, 24.0, 110.0, 120.0)),
            (ResizeHandle::North, rect(10.0, 24.0, 110.0, 120.0)),
            (ResizeHandle::NorthEast, rect(10.0, 24.0, 113.0, 120.0)),
            (ResizeHandle::East, rect(10.0, 20.0, 113.0, 120.0)),
            (ResizeHandle::SouthEast, rect(10.0, 20.0, 113.0, 124.0)),
            (ResizeHandle::South, rect(10.0, 20.0, 110.0, 124.0)),
            (ResizeHandle::SouthWest, rect(13.0, 20.0, 110.0, 124.0)),
            (ResizeHandle::West, rect(13.0, 20.0, 110.0, 120.0)),
        ] {
            assert_eq!(
                resize_selection(selection, handle, Vec2::new(3.0, 4.0), bounds),
                expected
            );
        }
    }

    #[test]
    fn crossing_opposite_corner_stops_at_one_logical_pixel() {
        let selection = rect(-100.0, -100.0, -10.0, -20.0);
        let bounds = rect(-200.0, -200.0, 200.0, 200.0);
        assert_eq!(
            resize_selection(
                selection,
                ResizeHandle::NorthWest,
                Vec2::splat(1000.0),
                bounds
            ),
            rect(-11.0, -21.0, -10.0, -20.0)
        );
        assert_eq!(
            resize_selection(
                selection,
                ResizeHandle::SouthEast,
                Vec2::splat(-1000.0),
                bounds
            ),
            rect(-100.0, -100.0, -99.0, -99.0)
        );
    }

    #[test]
    fn resizing_clamps_at_negative_desktop_edges_without_moving_anchor() {
        let selection = rect(-100.0, -100.0, 100.0, 100.0);
        let bounds = rect(-500.0, -300.0, 500.0, 400.0);
        assert_eq!(
            resize_selection(
                selection,
                ResizeHandle::NorthWest,
                Vec2::splat(-1000.0),
                bounds
            ),
            rect(-500.0, -300.0, 100.0, 100.0)
        );
        assert_eq!(
            resize_selection(
                selection,
                ResizeHandle::SouthEast,
                Vec2::splat(1000.0),
                bounds
            ),
            rect(-100.0, -100.0, 500.0, 400.0)
        );
    }

    #[test]
    fn moving_preserves_size_at_every_desktop_edge() {
        let selection = rect(-100.0, -100.0, 100.0, 100.0);
        let bounds = rect(-500.0, -300.0, 500.0, 400.0);
        assert_eq!(
            nudge_selection(selection, Vec2::new(-1000.0, -1000.0), false, bounds),
            rect(-500.0, -300.0, -300.0, -100.0)
        );
        assert_eq!(
            nudge_selection(selection, Vec2::new(1000.0, 1000.0), false, bounds),
            rect(300.0, 200.0, 500.0, 400.0)
        );
    }

    #[test]
    fn keyboard_resize_moves_right_and_bottom_edges_only() {
        let selection = rect(-100.0, -100.0, 100.0, 100.0);
        let bounds = rect(-500.0, -300.0, 500.0, 400.0);
        assert_eq!(
            nudge_selection(selection, Vec2::new(-10.0, 1.0), true, bounds),
            rect(-100.0, -100.0, 90.0, 101.0)
        );
        assert_eq!(
            nudge_selection(selection, Vec2::splat(-1000.0), true, bounds),
            rect(-100.0, -100.0, -99.0, -99.0)
        );
    }

    #[test]
    fn extreme_deltas_stay_finite_and_inside_desktop() {
        let selection = rect(-100.0, -100.0, 100.0, 100.0);
        let bounds = rect(-500.0, -300.0, 500.0, 400.0);
        assert_eq!(
            nudge_selection(selection, Vec2::splat(f32::MAX), false, bounds),
            rect(300.0, 200.0, 500.0, 400.0)
        );
        assert_eq!(
            resize_selection(
                selection,
                ResizeHandle::NorthEast,
                Vec2::new(f32::MAX, -f32::MAX),
                bounds
            ),
            rect(-100.0, -300.0, 500.0, 100.0)
        );
    }

    #[test]
    fn aid_stays_visible_at_display_corner_and_avoids_toolbar() {
        let bounds = rect(-1920.0, -200.0, 0.0, 880.0);
        let toolbar = rect(-350.0, 650.0, 0.0, 720.0);
        let aid = place_aid(
            Pos2::new(-1.0, 700.0),
            Vec2::new(120.0, 140.0),
            bounds,
            &[toolbar],
        )
        .unwrap();
        assert!(bounds.contains_rect(aid));
        assert!(!aid.intersects(toolbar));
        assert_eq!(aid.size(), Vec2::new(120.0, 140.0));
    }

    #[test]
    fn aid_is_omitted_when_no_space_can_avoid_toolbar() {
        let bounds = rect(0.0, 0.0, 100.0, 100.0);
        assert_eq!(
            place_aid(
                Pos2::new(50.0, 50.0),
                Vec2::new(50.0, 50.0),
                bounds,
                &[bounds]
            ),
            None
        );
        assert_eq!(
            place_aid(Pos2::ZERO, Vec2::new(101.0, 100.0), bounds, &[]),
            None
        );
    }

    #[test]
    fn magnifier_reads_native_original_pixel_with_negative_origin_and_retina_scale() {
        use crate::display::DisplayGeometry;
        let geometry = DisplayGeometry::new(0, rect(-10.0, -20.0, 0.0, -10.0), (20, 20)).unwrap();
        let mut source = image::RgbaImage::new(20, 20);
        source.put_pixel(3, 5, image::Rgba([17, 34, 51, 255]));
        let mut display = CapturedDisplay::from_image(geometry, source, 20).unwrap();
        // Tiles may be transformed independently; sampling must retain the capture pixels.
        display.tiles[0]
            .image
            .put_pixel(3, 5, image::Rgba([255, 0, 0, 255]));
        assert_eq!(
            sample_original_pixel(&display, Pos2::new(-8.1, -17.1)),
            Some(((3, 5), Color32::from_rgb(17, 34, 51)))
        );
        assert_eq!(sample_original_pixel(&display, Pos2::new(0.0, -17.1)), None);
        assert_eq!(
            sample_original_pixel(&display, Pos2::new(-10.1, -20.0)),
            None
        );
    }
}
