use crate::{text, Canvas, Event, GfxCtx, InputResult, Key, Text, UserInput, CENTERED};

// TODO right now, only a single line

pub struct TextBox {
    prompt: String,
    // TODO A rope would be cool.
    // TODO dont be pub
    pub line: String,
    cursor_x: usize,
    shift_pressed: bool,
}

impl TextBox {
    pub fn new(prompt: &str, prefilled: Option<String>) -> TextBox {
        let line = prefilled.unwrap_or_else(String::new);
        TextBox {
            prompt: prompt.to_string(),
            cursor_x: line.len(),
            line,
            shift_pressed: false,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        let mut txt = Text::new();
        txt.add_styled_line(self.prompt.clone(), None, Some(text::PROMPT_COLOR));

        txt.add_line(self.line[0..self.cursor_x].to_string());
        if self.cursor_x < self.line.len() {
            txt.append(
                self.line[self.cursor_x..=self.cursor_x].to_string(),
                None,
                Some(text::SELECTED_COLOR),
            );
            txt.append(self.line[self.cursor_x + 1..].to_string(), None, None);
        } else {
            txt.append(" ".to_string(), None, Some(text::SELECTED_COLOR));
        }

        canvas.draw_blocking_text(g, txt, CENTERED);
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
}
