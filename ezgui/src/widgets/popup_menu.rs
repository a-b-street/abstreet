use crate::layout::Widget;
use crate::{
    hotkey, layout, text, Choice, EventCtx, GfxCtx, InputResult, Key, Line, ScreenDims, ScreenPt,
    ScreenRectangle, Text,
};

// Separate from ModalMenu. There are some similarities, but I'm not sure it's worth making both
// complex.

pub struct PopupMenu<T: Clone> {
    prompt: Text,
    choices: Vec<Choice<T>>,
    current_idx: usize,
    standalone_layout: Option<layout::ContainerOrientation>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl<T: Clone> PopupMenu<T> {
    pub fn new(prompt: Text, choices: Vec<Choice<T>>, ctx: &EventCtx) -> PopupMenu<T> {
        let mut m = PopupMenu {
            prompt,
            choices,
            current_idx: 0,
            standalone_layout: Some(layout::ContainerOrientation::Centered),

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(0.0, 0.0),
        };
        m.recalculate_dims(ctx);
        m
    }

    pub fn event(&mut self, ctx: &mut EventCtx) -> InputResult<T> {
        if let Some(o) = self.standalone_layout {
            layout::stack_vertically(o, ctx.canvas, vec![self]);
            self.recalculate_dims(ctx);
        }

        // Handle the mouse
        if ctx.redo_mouseover() {
            let cursor = ctx.canvas.get_cursor_in_screen_space();
            let mut top_left = self.top_left;
            top_left.y += ctx.canvas.text_dims(&self.prompt).1;
            for idx in 0..self.choices.len() {
                let rect = ScreenRectangle {
                    x1: top_left.x,
                    y1: top_left.y,
                    x2: top_left.x + self.dims.width,
                    y2: top_left.y + ctx.canvas.line_height,
                };
                if rect.contains(cursor) {
                    self.current_idx = idx;
                    break;
                }
                top_left.y += ctx.canvas.line_height;
            }
        }
        {
            let choice = &self.choices[self.current_idx];
            if ctx.input.left_mouse_button_pressed() && choice.active {
                return InputResult::Done(choice.label.clone(), choice.data.clone());
            }
        }

        // Handle hotkeys
        for choice in &self.choices {
            if !choice.active {
                continue;
            }
            if let Some(hotkey) = choice.hotkey {
                if ctx.input.new_was_pressed(hotkey) {
                    return InputResult::Done(choice.label.clone(), choice.data.clone());
                }
            }
        }

        // Handle nav keys
        if ctx.input.new_was_pressed(hotkey(Key::Enter).unwrap()) {
            let choice = &self.choices[self.current_idx];
            if choice.active {
                return InputResult::Done(choice.label.clone(), choice.data.clone());
            } else {
                return InputResult::StillActive;
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
            return InputResult::Canceled;
        }

        InputResult::StillActive
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.draw_text_at_screenspace_topleft(&self.calculate_txt(), self.top_left);
    }

    pub fn current_choice(&self) -> &T {
        &self.choices[self.current_idx].data
    }

    fn recalculate_dims(&mut self, ctx: &EventCtx) {
        let (w, h) = ctx.canvas.text_dims(&self.calculate_txt());
        self.dims = ScreenDims::new(w, h);
    }

    fn calculate_txt(&self) -> Text {
        let mut txt = self.prompt.clone();

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

    fn set_pos(&mut self, top_left: ScreenPt, _total_width: f64) {
        self.top_left = top_left;
        // TODO Stretch to fill total width if it's smaller than us? Or that's impossible
    }
}
