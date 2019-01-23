use crate::{text, Canvas, Event, GfxCtx, InputResult, Key, Text, UserInput, CENTERED};

// Stores some associated data with each choice
// TODO Dedupe with the other menu, which doesn't need to scroll.
pub struct ScrollingMenu<T: Clone> {
    prompt: String,
    choices: Vec<(String, T)>,
    current_idx: usize,
}

impl<T: Clone> ScrollingMenu<T> {
    pub fn new(prompt: &str, choices: Vec<(String, T)>) -> ScrollingMenu<T> {
        if choices.is_empty() {
            panic!("Can't create a menu without choices for \"{}\"", prompt);
        }
        ScrollingMenu {
            prompt: prompt.to_string(),
            choices,
            current_idx: 0,
        }
    }

    pub fn event(&mut self, input: &mut UserInput) -> InputResult<T> {
        let maybe_ev = input.use_event_directly();
        if maybe_ev.is_none() {
            return InputResult::StillActive;
        }
        let ev = maybe_ev.unwrap();

        if ev == Event::KeyPress(Key::Escape) {
            return InputResult::Canceled;
        } else if ev == Event::KeyPress(Key::Enter) {
            // TODO instead of requiring clone, we could drain choices to take ownership of the
            // item. but without consuming self here, it's a bit sketchy to do that.
            let (name, item) = self.choices[self.current_idx].clone();
            return InputResult::Done(name, item);
        } else if ev == Event::KeyPress(Key::UpArrow) {
            if self.current_idx > 0 {
                self.current_idx -= 1;
            }
        } else if ev == Event::KeyPress(Key::DownArrow) {
            if self.current_idx < self.choices.len() - 1 {
                self.current_idx += 1;
            }
        }

        InputResult::StillActive
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        let mut txt = Text::new();
        txt.add_styled_line(self.prompt.clone(), None, Some(text::PROMPT_COLOR));

        // TODO Silly results from doing this:
        // - The menu width changes as we scroll
        // - Some off-by-one / usize rounding bugs causing menu height to change a bit

        // How many lines can we fit on the screen?
        let can_fit = {
            // Subtract 1 for the prompt, and an additional TODO hacky
            // few to avoid the bottom OSD and stuff.
            let n = (canvas.window_height / canvas.line_height).floor() as isize - 1 - 6;
            if n <= 0 {
                // Weird small window, just display the prompt and bail out.
                canvas.draw_blocking_text(g, txt, CENTERED);
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

        for (idx, (line, _)) in self.choices.iter().enumerate() {
            if idx < low_idx || idx > low_idx + can_fit {
                continue;
            }
            if self.current_idx == idx {
                txt.add_styled_line(line.clone(), None, Some(text::SELECTED_COLOR));
            } else {
                txt.add_line(line.clone());
            }
        }

        canvas.draw_blocking_text(g, txt, CENTERED);
    }

    pub fn current_choice(&self) -> &T {
        &self.choices[self.current_idx].1
    }
}
