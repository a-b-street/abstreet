use crate::{
    text, Canvas, Event, GfxCtx, InputResult, Key, Line, ScreenDims, ScreenPt, ScreenRectangle,
    Text,
};
use std::collections::{BTreeMap, BTreeSet};

pub enum ContextMenu {
    Inactive(BTreeSet<Key>),
    Building(ScreenPt, BTreeMap<Key, String>),
    Displaying(ContextMenuImpl),
    Clicked(String),
}

impl ContextMenu {
    pub fn new() -> ContextMenu {
        ContextMenu::Inactive(BTreeSet::new())
    }

    pub fn maybe_build(self, canvas: &Canvas) -> ContextMenu {
        match self {
            ContextMenu::Building(origin, actions) => {
                if actions.is_empty() {
                    ContextMenu::new()
                } else {
                    ContextMenu::Displaying(ContextMenuImpl::new(
                        actions
                            .into_iter()
                            .map(|(key, action)| (action, key))
                            .collect(),
                        origin,
                        canvas,
                    ))
                }
            }
            _ => self,
        }
    }
}

pub struct ContextMenuImpl {
    choices: Vec<(String, Key)>,
    current_idx: Option<usize>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl ContextMenuImpl {
    pub fn new(choices: Vec<(String, Key)>, corner: ScreenPt, canvas: &Canvas) -> ContextMenuImpl {
        let mut m = ContextMenuImpl {
            choices,
            current_idx: None,

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(0.0, 0.0),
        };
        let (w, h) = canvas.text_dims(&m.calculate_txt());
        m.dims = ScreenDims::new(w, h);
        m.top_left = m.dims.top_left_for_corner(corner, canvas);

        m
    }

    pub fn event(&mut self, ev: Event, canvas: &Canvas) -> InputResult<()> {
        // Handle the mouse
        match ev {
            Event::WindowLostCursor => {
                self.current_idx = None;
            }
            Event::MouseMovedTo(cursor) => {
                self.current_idx = None;
                let mut top_left = self.top_left;
                for idx in 0..self.choices.len() {
                    let rect = ScreenRectangle {
                        x1: top_left.x,
                        y1: top_left.y,
                        x2: top_left.x + self.dims.width,
                        y2: top_left.y + canvas.line_height,
                    };
                    if rect.contains(cursor) {
                        self.current_idx = Some(idx);
                        break;
                    }
                    top_left.y += canvas.line_height;
                }
            }
            Event::LeftMouseButtonDown => {
                if let Some(idx) = self.current_idx {
                    return InputResult::Done(self.choices[idx].0.clone(), ());
                } else {
                    return InputResult::Canceled;
                }
            }
            _ => {}
        }

        // Handle hotkeys
        for (action, key) in &self.choices {
            if ev == Event::KeyPress(*key) {
                return InputResult::Done(action.clone(), ());
            }
        }

        if ev == Event::KeyPress(Key::Escape) {
            return InputResult::Canceled;
        }

        InputResult::StillActive
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.draw_text_at_screenspace_topleft(&self.calculate_txt(), self.top_left);
    }

    pub fn all_keys(&self) -> Vec<Key> {
        self.choices.iter().map(|(_, key)| *key).collect()
    }

    fn calculate_txt(&self) -> Text {
        let mut txt = Text::new();

        for (idx, (action, key)) in self.choices.iter().enumerate() {
            txt.add_appended(vec![
                Line(key.describe()).fg(text::HOTKEY_COLOR),
                Line(format!(" - {}", action)),
            ]);

            // TODO BG color should be on the TextSpan, so this isn't so terrible?
            if Some(idx) == self.current_idx {
                txt.highlight_last_line(text::SELECTED_COLOR);
            }
        }
        txt
    }
}
