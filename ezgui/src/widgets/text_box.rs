use crate::layout::Widget;
use crate::{
    text, Canvas, Event, GfxCtx, InputResult, Key, Line, ScreenDims, ScreenPt, Text, UserInput,
};

// TODO right now, only a single line

pub struct TextBox {
    prompt: String,
    // TODO A rope would be cool.
    line: String,
    cursor_x: usize,
    shift_pressed: bool,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl TextBox {
    pub fn new(prompt: &str, prefilled: Option<String>, canvas: &Canvas) -> TextBox {
        let line = prefilled.unwrap_or_else(String::new);
        let mut tb = TextBox {
            prompt: prompt.to_string(),
            cursor_x: line.len(),
            line,
            shift_pressed: false,

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(0.0, 0.0),
        };
        // TODO Assume the dims never exceed the prompt width?
        tb.dims = canvas.text_dims(&tb.get_text());
        tb
    }

    pub(crate) fn get_text(&self) -> Text {
        let mut txt = Text::prompt(&self.prompt);
        txt.add(Line(&self.line[0..self.cursor_x]));
        if self.cursor_x < self.line.len() {
            // TODO This "cursor" looks awful!
            txt.append_all(vec![
                Line("|").fg(text::SELECTED_COLOR),
                Line(&self.line[self.cursor_x..=self.cursor_x]),
                Line(&self.line[self.cursor_x + 1..]),
            ]);
        } else {
            txt.append(Line("|").fg(text::SELECTED_COLOR));
        }
        txt
    }

    pub(crate) fn get_line(&self) -> &str {
        &self.line
    }

    pub(crate) fn set_text(&mut self, line: String) {
        self.line = line;
        self.cursor_x = self.line.len();
    }

    pub fn event(&mut self, input: &mut UserInput) -> InputResult<()> {
        let maybe_ev = input.use_event_directly();
        if maybe_ev.is_none() {
            return InputResult::StillActive;
        }
        let ev = maybe_ev.unwrap();

        if ev == Event::KeyPress(Key::Escape) {
            return InputResult::Canceled;
        } else if ev == Event::KeyPress(Key::Enter) {
            return InputResult::Done(self.line.clone(), ());
        } else if ev == Event::KeyPress(Key::LeftShift) {
            self.shift_pressed = true;
        } else if ev == Event::KeyRelease(Key::LeftShift) {
            self.shift_pressed = false;
        } else if ev == Event::KeyPress(Key::LeftArrow) {
            if self.cursor_x > 0 {
                self.cursor_x -= 1;
            }
        } else if ev == Event::KeyPress(Key::RightArrow) {
            self.cursor_x = (self.cursor_x + 1).min(self.line.len());
        } else if ev == Event::KeyPress(Key::Backspace) {
            if self.cursor_x > 0 {
                self.line.remove(self.cursor_x - 1);
                self.cursor_x -= 1;
            }
        } else if let Event::KeyPress(key) = ev {
            if let Some(c) = key.to_char(self.shift_pressed) {
                self.line.insert(self.cursor_x, c);
                self.cursor_x += 1;
            }
        };
        InputResult::StillActive
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.draw_text_at_screenspace_topleft(&self.get_text(), self.top_left);
    }
}

impl Widget for TextBox {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}
