use crate::layout::Widget;
use crate::{
    hotkey, text, Choice, EventCtx, GfxCtx, InputResult, Key, Line, ScreenDims, ScreenPt,
    ScreenRectangle, Text,
};

// Separate from ModalMenu. There are some similarities, but I'm not sure it's worth making both
// complex.

pub struct PopupMenu<T: Clone> {
    choices: Vec<Choice<T>>,
    current_idx: usize,

    pub(crate) state: InputResult<T>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl<T: Clone> PopupMenu<T> {
    pub fn new(choices: Vec<Choice<T>>, ctx: &EventCtx) -> PopupMenu<T> {
        let mut m = PopupMenu {
            choices,
            current_idx: 0,

            state: InputResult::StillActive,

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(0.0, 0.0),
        };
        m.recalculate_dims(ctx);
        m
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        match self.state {
            InputResult::StillActive => {}
            _ => unreachable!(),
        }

        // Handle the mouse
        if ctx.redo_mouseover() {
            let cursor = ctx.canvas.get_cursor_in_screen_space();
            let mut top_left = self.top_left;
            for idx in 0..self.choices.len() {
                let rect = ScreenRectangle {
                    x1: top_left.x,
                    y1: top_left.y,
                    x2: top_left.x + self.dims.width,
                    y2: top_left.y + ctx.default_line_height(),
                };
                if rect.contains(cursor) {
                    self.current_idx = idx;
                    break;
                }
                top_left.y += ctx.default_line_height();
            }
        }
        {
            let choice = &self.choices[self.current_idx];
            if ctx.normal_left_click() {
                // Did we actually click the entry?
                let mut top_left = self.top_left;
                top_left.y += ctx.default_line_height() * (self.current_idx as f64);
                let rect = ScreenRectangle {
                    x1: top_left.x,
                    y1: top_left.y,
                    x2: top_left.x + self.dims.width,
                    y2: top_left.y + ctx.default_line_height(),
                };
                if rect.contains(ctx.canvas.get_cursor_in_screen_space()) {
                    if choice.active {
                        self.state = InputResult::Done(choice.label.clone(), choice.data.clone());
                        return;
                    }
                }
            }
        }

        // Handle hotkeys
        for choice in &self.choices {
            if !choice.active {
                continue;
            }
            if let Some(hotkey) = choice.hotkey {
                if ctx.input.new_was_pressed(hotkey) {
                    self.state = InputResult::Done(choice.label.clone(), choice.data.clone());
                    return;
                }
            }
        }

        // Handle nav keys
        if ctx.input.new_was_pressed(hotkey(Key::Enter).unwrap()) {
            let choice = &self.choices[self.current_idx];
            if choice.active {
                self.state = InputResult::Done(choice.label.clone(), choice.data.clone());
                return;
            } else {
                return;
            }
        } else if ctx.input.new_was_pressed(hotkey(Key::UpArrow).unwrap()) {
            if self.current_idx > 0 {
                self.current_idx -= 1;
            }
        } else if ctx.input.new_was_pressed(hotkey(Key::DownArrow).unwrap()) {
            if self.current_idx < self.choices.len() - 1 {
                self.current_idx += 1;
            }
        } else if ctx.input.new_was_pressed(hotkey(Key::Escape).unwrap()) {
            self.state = InputResult::Canceled;
            return;
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.draw_text_at_screenspace_topleft(&self.calculate_txt(), self.top_left);
    }

    pub fn current_choice(&self) -> &T {
        &self.choices[self.current_idx].data
    }

    fn recalculate_dims(&mut self, ctx: &EventCtx) {
        self.dims = ctx.text_dims(&self.calculate_txt());
    }

    fn calculate_txt(&self) -> Text {
        let mut txt = Text::new();

        for (idx, choice) in self.choices.iter().enumerate() {
            if choice.active {
                if let Some(key) = choice.hotkey {
                    txt.add_appended(vec![
                        Line(key.describe()).fg(text::HOTKEY_COLOR),
                        Line(format!(" - {}", choice.label)),
                    ]);
                } else {
                    txt.add(Line(&choice.label));
                }
            } else {
                if let Some(key) = choice.hotkey {
                    txt.add(
                        Line(format!("{} - {}", key.describe(), choice.label))
                            .fg(text::INACTIVE_CHOICE_COLOR),
                    );
                } else {
                    txt.add(Line(&choice.label).fg(text::INACTIVE_CHOICE_COLOR));
                }
            }

            // TODO BG color should be on the TextSpan, so this isn't so terrible?
            if idx == self.current_idx {
                txt.highlight_last_line(text::SELECTED_COLOR);
            }
        }
        txt
    }
}

impl<T: Clone> Widget for PopupMenu<T> {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}
