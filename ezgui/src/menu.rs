use piston::input::{Button, Key, PressEvent};
use {text, Canvas, GfxCtx, InputResult, TextOSD, UserInput};

pub struct Menu {
    prompt: String,
    choices: Vec<String>,
    current_idx: usize,
}

impl Menu {
    pub fn new(prompt: &str, choices: Vec<String>) -> Menu {
        Menu {
            prompt: prompt.to_string(),
            choices,
            current_idx: 0,
        }
    }

    pub fn event(&mut self, input: &mut UserInput) -> InputResult {
        let ev = input.use_event_directly().clone();
        input.consume_event();

        if let Some(Button::Keyboard(Key::Escape)) = ev.press_args() {
            return InputResult::Canceled;
        }

        if let Some(Button::Keyboard(Key::Return)) = ev.press_args() {
            return InputResult::Done(self.choices[self.current_idx].clone());
        }

        if let Some(Button::Keyboard(Key::Up)) = ev.press_args() {
            if self.current_idx > 0 {
                self.current_idx -= 1;
            }
        }
        if let Some(Button::Keyboard(Key::Down)) = ev.press_args() {
            if self.current_idx < self.choices.len() - 1 {
                self.current_idx += 1;
            }
        }

        InputResult::StillActive
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        let mut osd = TextOSD::new();
        osd.add_styled_line(
            self.prompt.clone(),
            text::TEXT_FG_COLOR,
            Some(text::TEXT_QUERY_COLOR),
        );

        // TODO Silly results from doing this:
        // - The menu width changes as we scroll
        // - Some off-by-one / usize rounding bugs causing menu height to change a bit

        // How many lines can we fit on the screen?
        let can_fit = {
            // Subtract 1 for the prompt, and an additional TODO hacky
            // few to avoid the bottom OSD and stuff.
            let n =
                (f64::from(canvas.window_size.height) / text::LINE_HEIGHT).floor() as isize - 1 - 6;
            if n <= 0 {
                // Weird small window, just display the prompt and bail out.
                canvas.draw_centered_text(g, osd);
                return;
            }
            n as usize
        };

        let low_idx = if self.choices.len() <= can_fit {
            0
        } else {
            let middle = can_fit / 2;
            if self.current_idx >= middle {
                (self.current_idx - middle).min(self.choices.len() - (middle * 2))
            } else {
                0
            }
        };

        for (idx, line) in self.choices.iter().enumerate() {
            if idx < low_idx || idx > low_idx + can_fit {
                continue;
            }
            if self.current_idx == idx {
                osd.add_styled_line(
                    line.clone(),
                    text::TEXT_FG_COLOR,
                    Some(text::TEXT_FOCUS_COLOR),
                );
            } else {
                osd.add_line(line.clone());
            }
        }

        canvas.draw_centered_text(g, osd);
    }

    pub fn current_choice(&self) -> &String {
        &self.choices[self.current_idx]
    }
}
