use crate::{text, Canvas, Color, Event, GfxCtx, InputResult, Key, Text, UserInput};
use geom::{Polygon, Pt2D};

// Stores some associated data with each choice
pub struct Menu<T: Clone> {
    prompt: Option<String>,
    choices: Vec<(Key, String, T)>,
    current_idx: Option<usize>,

    origin: Pt2D,
    // The rectangle representing the top row of the menu (not including the optional prompt), then
    // the height of one row
    // TODO Needing a separate call to initialize geometry sucks.
    geometry: Option<(Polygon, f64)>,
}

impl<T: Clone> Menu<T> {
    /*pub fn new(prompt: &str, choices: Vec<(String, T)>) -> Menu<T> {
        if choices.is_empty() {
            panic!("Can't create a menu without choices for \"{}\"", prompt);
        }
        Menu {
            prompt: prompt.to_string(),
            choices,
            current_idx: 0,
        }
    }*/

    pub fn event(&mut self, input: &mut UserInput, canvas: &Canvas) -> InputResult<T> {
        // We have to directly look at stuff here; all of input's methods lie and pretend nothing
        // is happening.
        let maybe_ev = input.use_event_directly();
        if maybe_ev.is_none() {
            return InputResult::StillActive;
        }
        let ev = maybe_ev.unwrap();

        // Handle the mouse
        if let Some((ref row, height)) = self.geometry {
            if ev == Event::LeftMouseButtonDown {
                if let Some(i) = self.current_idx {
                    let (_, choice, data) = self.choices[i].clone();
                    return InputResult::Done(choice, data);
                } else {
                    return InputResult::Canceled;
                }
            } else if let Event::MouseMovedTo(x, y) = ev {
                let cursor_pt = canvas.screen_to_map((x, y));
                let mut matched = false;
                for i in 0..self.choices.len() {
                    if row
                        .translate(0.0, (i as f64) * height)
                        .contains_pt(cursor_pt)
                    {
                        self.current_idx = Some(i);
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    self.current_idx = None;
                }
            }
        }

        // Handle keys
        if ev == Event::KeyPress(Key::Escape) {
            return InputResult::Canceled;
        }

        // TODO Disable arrow keys in context menus and the top menu?
        if let Some(idx) = self.current_idx {
            if ev == Event::KeyPress(Key::Enter) {
                let (_, name, data) = self.choices[idx].clone();
                return InputResult::Done(name, data);
            } else if ev == Event::KeyPress(Key::UpArrow) {
                if idx > 0 {
                    self.current_idx = Some(idx - 1);
                }
            } else if ev == Event::KeyPress(Key::DownArrow) {
                if idx < self.choices.len() - 1 {
                    self.current_idx = Some(idx + 1);
                }
            }
        }

        InputResult::StillActive
    }

    pub(crate) fn calculate_geometry(&mut self, canvas: &mut Canvas) {
        if self.geometry.is_some() {
            return;
        }

        let mut txt = Text::new();
        // TODO prompt
        for (hotkey, choice, _) in &self.choices {
            txt.add_line(format!("{} - {}", hotkey.describe(), choice));
        }
        let (screen_width, screen_height) = canvas.text_dims(&txt);
        let map_width = screen_width / canvas.cam_zoom;
        let map_height = screen_height / canvas.cam_zoom;
        let top_left = Pt2D::new(
            self.origin.x() - (map_width / 2.0),
            self.origin.y() - (map_height / 2.0),
        );
        let row_height = map_height / (self.choices.len() as f64);
        self.geometry = Some((
            Polygon::rectangle_topleft(top_left, map_width, row_height),
            row_height,
        ));
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        let mut txt = Text::new();
        // TODO prompt using Some(text::TEXT_QUERY_COLOR)
        for (idx, (hotkey, choice, _)) in self.choices.iter().enumerate() {
            let bg = if Some(idx) == self.current_idx {
                Some(Color::WHITE)
            } else {
                None
            };
            txt.add_styled_line(hotkey.describe(), Color::BLUE, bg);
            txt.append(format!(" - {}", choice), text::TEXT_FG_COLOR, bg);
        }
        canvas.draw_text_at(g, txt, self.origin);
    }

    pub fn current_choice(&self) -> Option<&T> {
        let idx = self.current_idx?;
        Some(&self.choices[idx].2)
    }
}
