use std::collections::VecDeque;

use egui::{Pos2, Rect};

use crate::app_default::{Annotation, ScreenshotApp, Tool};

const HISTORY_LIMIT: usize = 100;

#[derive(Clone, PartialEq)]
struct EditState {
    selection: Option<Rect>,
    annotations: Vec<Annotation>,
    number: Option<i32>,
}

/// Stores editing data only; screenshot pixels stay in the capture session.
#[derive(Clone, Default)]
pub(crate) struct EditHistory {
    undo: VecDeque<EditState>,
    redo: Vec<EditState>,
    pending: Option<EditState>,
}

impl ScreenshotApp {
    fn edit_state(&self) -> EditState {
        EditState {
            selection: self.selection_rect,
            annotations: self.annotations.clone(),
            number: self.number_input,
        }
    }

    pub(crate) fn begin_edit(&mut self) {
        if self.edit_history.pending.is_none() {
            self.edit_history.pending = Some(self.edit_state());
        }
    }

    pub(crate) fn commit_edit(&mut self) {
        if let Some(before) = self.edit_history.pending.take()
            && before != self.edit_state()
        {
            self.edit_history.undo.push_back(before);
            if self.edit_history.undo.len() > HISTORY_LIMIT {
                self.edit_history.undo.pop_front();
            }
            self.edit_history.redo.clear();
        }
    }

    fn restore_edit(&mut self, state: EditState) {
        self.selection_rect = state.selection;
        self.annotations = state.annotations;
        self.number_input = state.number;
        self.selection_start = state.selection.map_or(Pos2::ZERO, |r| r.min);
        self.selection_end = state.selection.map_or(Pos2::ZERO, |r| r.max);
        self.current_annotation = None;
        self.text_input = None;
        self.text_input_finalized = false;
        self.is_moving_box = false;
        self.resize_handle = None;
        self.is_selecting = false;
        self.original_selection_rect = None;
        self.show_toolbar = self.selection_rect.is_some();
        self.last_click_time = f64::NEG_INFINITY;
    }

    pub(crate) fn cancel_edit(&mut self) {
        if let Some(before) = self.edit_history.pending.take() {
            self.restore_edit(before);
        }
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.edit_history.pending.is_some() || !self.edit_history.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.edit_history.pending.is_none() && !self.edit_history.redo.is_empty()
    }

    pub(crate) fn undo_edit(&mut self) {
        if self.edit_history.pending.is_some() {
            self.cancel_edit();
        } else if let Some(before) = self.edit_history.undo.pop_back() {
            let current = self.edit_state();
            self.edit_history.redo.push(current);
            self.restore_edit(before);
        }
    }

    pub(crate) fn redo_edit(&mut self) {
        if self.edit_history.pending.is_some() {
            return;
        }
        if let Some(after) = self.edit_history.redo.pop() {
            let current = self.edit_state();
            self.edit_history.undo.push_back(current);
            self.restore_edit(after);
        }
    }

    pub(crate) fn clear_selection_edits(&mut self) {
        self.edit_history = EditHistory::default();
        self.restore_edit(EditState {
            selection: None,
            annotations: Vec::new(),
            number: None,
        });
        self.current_tool = Tool::Select;
        self.toolbar_placement = None;
        self.toolbar_rect_global = None;
    }
}

#[cfg(test)]
mod tests {
    use crate::app_default::{Annotation, ScreenshotApp, Tool};
    use egui::{Color32, Pos2, Rect, vec2};

    fn numbered_mark(number: i32) -> Annotation {
        Annotation {
            tool: Tool::Number,
            points: vec![Pos2::new(20.0, 20.0)],
            color: Color32::RED,
            stroke_width: 3.0,
            text: String::new(),
            number: Some(number),
        }
    }

    #[test]
    fn undo_and_redo_restore_annotation_and_next_number() {
        let mut app = ScreenshotApp::default();
        app.begin_edit();
        app.annotations.push(numbered_mark(1));
        app.number_input = Some(1);
        app.commit_edit();
        app.undo_edit();
        assert!(app.annotations.is_empty());
        assert_eq!(app.number_input, None);
        app.redo_edit();
        assert_eq!(app.annotations.len(), 1);
        assert_eq!(app.annotations[0].number, Some(1));
        assert_eq!(app.number_input, Some(1));
    }

    #[test]
    fn new_edit_invalidates_redo_but_noop_does_not() {
        let mut app = ScreenshotApp::default();
        app.begin_edit();
        app.annotations.push(numbered_mark(1));
        app.commit_edit();
        app.undo_edit();
        app.begin_edit();
        app.commit_edit();
        assert!(app.can_redo());
        app.begin_edit();
        app.annotations.push(numbered_mark(2));
        app.commit_edit();
        assert!(!app.can_redo());
        app.redo_edit();
        assert_eq!(app.annotations[0].number, Some(2));
    }

    #[test]
    fn drag_is_one_history_action_and_restores_selection_endpoints() {
        let mut app = ScreenshotApp::default();
        let original = Rect::from_min_size(Pos2::new(-50.0, 20.0), vec2(100.0, 50.0));
        app.selection_rect = Some(original);
        app.begin_edit();
        app.selection_rect = Some(original.translate(vec2(5.0, 0.0)));
        app.begin_edit(); // repeated held frames must not replace the original checkpoint
        app.selection_rect = Some(original.translate(vec2(10.0, 0.0)));
        app.commit_edit();
        app.undo_edit();
        assert_eq!(app.selection_rect, Some(original));
        assert_eq!(app.selection_start, original.min);
        assert_eq!(app.selection_end, original.max);
        assert!(!app.can_undo());
    }

    #[test]
    fn cancelling_unfinished_mark_restores_number_without_consuming_undo() {
        let mut app = ScreenshotApp::default();
        app.begin_edit();
        app.annotations.push(numbered_mark(1));
        app.number_input = Some(1);
        app.commit_edit();
        app.begin_edit();
        app.number_input = Some(2);
        app.current_annotation = Some(numbered_mark(2));
        app.undo_edit();
        assert!(app.current_annotation.is_none());
        assert_eq!(app.annotations.len(), 1);
        assert_eq!(app.number_input, Some(1));
        assert!(app.can_undo());
    }

    #[test]
    fn clearing_selection_discards_history_and_annotations() {
        let mut app = ScreenshotApp {
            selection_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0))),
            ..Default::default()
        };
        app.begin_edit();
        app.annotations.push(numbered_mark(1));
        app.commit_edit();
        app.clear_selection_edits();
        assert!(app.selection_rect.is_none());
        assert!(app.annotations.is_empty());
        assert!(!app.can_undo());
        assert!(!app.can_redo());
    }

    #[test]
    fn ocr_recapture_cancellation_restores_edit_history() {
        let mut app = ScreenshotApp {
            selection_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0))),
            ..Default::default()
        };
        app.begin_edit();
        app.annotations.push(numbered_mark(1));
        app.number_input = Some(1);
        app.commit_edit();
        assert!(app.begin_ocr_recapture());
        assert!(!app.can_undo());
        assert!(app.cancel_ocr_recapture());
        app.undo_edit();
        assert!(app.annotations.is_empty());
        assert_eq!(app.number_input, None);
    }

    #[test]
    fn capture_reset_cannot_restore_edits_from_previous_screenshot() {
        let mut app = ScreenshotApp::default();
        app.begin_edit();
        app.annotations.push(numbered_mark(1));
        app.commit_edit();
        app.clear_capture_session();
        assert!(!app.can_undo());
        app.undo_edit();
        assert!(app.annotations.is_empty());
    }

    #[test]
    fn history_evicts_oldest_actions_without_copying_capture_pixels() {
        let mut app = ScreenshotApp::default();
        for i in 1..=105 {
            app.begin_edit();
            app.number_input = Some(i);
            app.commit_edit();
        }
        for _ in 0..100 {
            app.undo_edit();
        }
        assert_eq!(app.number_input, Some(5));
        assert!(!app.can_undo());
    }
}
