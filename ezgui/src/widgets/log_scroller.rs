use crate::{text, Event, GfxCtx, Key, Text, UserInput, CENTERED};
use std::collections::VecDeque;

pub struct LogScroller {
    // TODO store SpanText or similar
    lines: VecDeque<String>,
    capacity: usize,
    y_offset: usize,
}

impl LogScroller {
    pub fn new_with_capacity(capacity: usize) -> LogScroller {
        LogScroller {
            lines: VecDeque::with_capacity(capacity),
            // Store separately, since VecDeque might internally choose a bigger capacity
            capacity,
            y_offset: 0,
        }
    }

    pub fn new_from_lines(lines: Vec<String>) -> LogScroller {
        let capacity = lines.len();
        LogScroller {
            lines: VecDeque::from(lines),
            capacity,
            y_offset: 0,
        }
    }

    // TODO take and store styled text
    pub fn add_line(&mut self, line: &str) {
        if self.lines.len() == self.capacity {
            self.lines.pop_front();
        }
        self.lines.push_back(line.to_string());
    }

    // True if done
    pub fn event(&mut self, input: &mut UserInput) -> bool {
        let maybe_ev = input.use_event_directly();
        if maybe_ev.is_none() {
            return false;
        }
        let ev = maybe_ev.unwrap();

        if ev == Event::KeyPress(Key::Enter) {
            return true;
        } else if ev == Event::KeyPress(Key::UpArrow) {
            if self.y_offset > 0 {
                self.y_offset -= 1;
            }
        } else if ev == Event::KeyPress(Key::DownArrow) {
            self.y_offset += 1;
        }

        false
    }

    // TODO overlapping logic with Menu
    pub fn draw(&self, g: &mut GfxCtx) {
        let mut txt = Text::new();
        // TODO Force padding of everything to a fixed 80% of the screen or so
        txt.add_styled_line("Logs".to_string(), None, Some(text::PROMPT_COLOR));

        // How many lines can we fit on the screen?
        let can_fit = {
            // Subtract 1 for the title, and an additional TODO hacky
            // few to avoid the bottom OSD and stuff.
            let n = (g.canvas.window_height / g.canvas.line_height).floor() as isize - 1 - 6;
            if n <= 0 {
                0
            } else {
                n as usize
            }
        };
        // TODO argh, we want to do this clamping in event() or something; otherwise we can
        // accumulate a bunch of invisible silly y_offsetness
        let mut low_idx = self.y_offset;
        if low_idx + can_fit > self.lines.len() && can_fit <= self.lines.len() {
            low_idx = self.lines.len() - can_fit;
        }
        let high_idx = (low_idx + can_fit).min(self.lines.len());

        // Slice syntax doesn't seem to work for no elements?
        if !self.lines.is_empty() {
            // TODO VecDeque can't be sliced, argh
            let copy: Vec<&String> = self.lines.iter().collect();
            for line in &copy[low_idx..high_idx] {
                txt.add_line(line.to_string());
            }
        }

        g.draw_blocking_text(txt, CENTERED);
    }
}
