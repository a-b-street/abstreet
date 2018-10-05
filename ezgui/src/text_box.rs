use keys::key_to_char;
use piston::input::{Button, ButtonEvent, Key, PressEvent, ReleaseEvent};
use {text, Canvas, GfxCtx, InputResult, Text, UserInput, CENTERED};

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
    pub fn new(prompt: &str) -> TextBox {
        TextBox::new_prefilled(prompt, String::from(""))
    }

    pub fn new_prefilled(prompt: &str, line: String) -> TextBox {
        TextBox {
            prompt: prompt.to_string(),
            line,
            cursor_x: 0,
            shift_pressed: false,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        let mut txt = Text::new();
        txt.add_styled_line(
            self.prompt.clone(),
            text::TEXT_FG_COLOR,
            Some(text::TEXT_QUERY_COLOR),
        );

        txt.add_line(self.line[0..self.cursor_x].to_string());
        if self.cursor_x < self.line.len() {
            txt.append(
                self.line[self.cursor_x..=self.cursor_x].to_string(),
                text::TEXT_FG_COLOR,
                Some(text::TEXT_FOCUS_COLOR),
            );
            txt.append(
                self.line[self.cursor_x + 1..].to_string(),
                text::TEXT_FG_COLOR,
                None,
            );
        } else {
            txt.append(
                " ".to_string(),
                text::TEXT_FG_COLOR,
                Some(text::TEXT_FOCUS_COLOR),
            );
        }

        canvas.draw_text(g, txt, CENTERED);
    }

    pub fn event(&mut self, input: &mut UserInput) -> InputResult<()> {
        let ev = input.use_event_directly().clone();

        if let Some(Button::Keyboard(Key::Escape)) = ev.press_args() {
            return InputResult::Canceled;
        }

        // Done?
        if let Some(Button::Keyboard(Key::Return)) = ev.press_args() {
            return InputResult::Done(self.line.clone(), ());
        }

        // Key state tracking
        if let Some(Button::Keyboard(Key::LShift)) = ev.press_args() {
            self.shift_pressed = true;
        }
        if let Some(Button::Keyboard(Key::LShift)) = ev.release_args() {
            self.shift_pressed = false;
        }

        // Cursor movement
        if let Some(Button::Keyboard(Key::Left)) = ev.press_args() {
            if self.cursor_x > 0 {
                self.cursor_x -= 1;
            }
        }
        if let Some(Button::Keyboard(Key::Right)) = ev.press_args() {
            self.cursor_x = (self.cursor_x + 1).min(self.line.len());
        }

        // Backspace
        if let Some(Button::Keyboard(Key::Backspace)) = ev.press_args() {
            if self.cursor_x > 0 {
                self.line.remove(self.cursor_x - 1);
                self.cursor_x -= 1;
            }
        }

        // Insert
        if let Some(Button::Keyboard(key)) = ev.press_args() {
            if let Some(mut c) = key_to_char(key) {
                if !self.shift_pressed {
                    c = c.to_lowercase().next().unwrap();
                }
                self.line.insert(self.cursor_x, c);
                self.cursor_x += 1;
            }
        } else if let Some(args) = ev.button_args() {
            // TODO Need to re-frame key_to_char to understand scancodes. Yay. ><
            if self.shift_pressed && args.scancode == Some(39) {
                self.line.insert(self.cursor_x, ':');
                self.cursor_x += 1;
            }
        }
        InputResult::StillActive
    }
}
