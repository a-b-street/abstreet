use crate::{text, Event, GfxCtx, Key, Text, UserInput, CENTERED};

// TODO Just displays text, no scrolling.
pub struct LogScroller {
    text: Text,
}

impl LogScroller {
    pub fn new(title: String, lines: Vec<String>) -> LogScroller {
        let mut text = Text::new();
        text.add_styled_line(title, None, Some(text::PROMPT_COLOR));
        for line in lines {
            text.add_line(line);
        }
        LogScroller { text }
    }

    // True if done
    pub fn event(&mut self, input: &mut UserInput) -> bool {
        let maybe_ev = input.use_event_directly();
        if maybe_ev.is_none() {
            return false;
        }
        let ev = maybe_ev.unwrap();

        if ev == Event::KeyPress(Key::Enter)
            || ev == Event::KeyPress(Key::Space)
            || ev == Event::KeyPress(Key::Escape)
        {
            return true;
        }

        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.draw_blocking_text(&self.text, CENTERED);
    }
}
