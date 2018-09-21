use keys::key_to_char;
use piston::input::{Button, ButtonEvent, Event, Key, PressEvent, ReleaseEvent};
use {Canvas, GfxCtx, TextOSD};

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
        let mut osd = TextOSD::new();
        osd.add_highlighted_line(self.prompt.clone());
        osd.add_line_with_cursor(self.line.clone(), self.cursor_x);
        canvas.draw_centered_text(g, osd);
    }

    // TODO a way to abort out
    // Returns true if the user confirmed their entry.
    // TODO return the entered string if done...
    pub fn event(&mut self, ev: &Event) -> bool {
        // Done?
        if let Some(Button::Keyboard(Key::Return)) = ev.press_args() {
            return true;
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
        false
    }
}
