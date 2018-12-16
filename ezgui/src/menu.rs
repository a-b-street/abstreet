use crate::{text, Canvas, Color, Event, GfxCtx, InputResult, Key, Text};
use geom::{Polygon, Pt2D};

// Stores some associated data with each choice
pub struct Menu<T: Clone> {
    prompt: Option<String>,
    choices: Vec<(Key, String, T)>,
    current_idx: Option<usize>,

    origin: Pt2D,
    first_choice_row: Polygon,
    row_height: f64,
}

impl<T: Clone> Menu<T> {
    pub fn new(
        prompt: Option<String>,
        choices: Vec<(Key, String, T)>,
        origin: Pt2D,
        canvas: &Canvas,
    ) -> Menu<T> {
        if choices.is_empty() {
            panic!("Can't create a menu without choices for {:?}", prompt);
        }

        // Calculate geometry.
        let mut txt = Text::new();
        // TODO prompt
        for (hotkey, choice, _) in &choices {
            txt.add_line(format!("{} - {}", hotkey.describe(), choice));
        }
        let (screen_width, screen_height) = canvas.text_dims(&txt);
        // Once a menu is created, all other controls (like zooming) are disabled, so this value
        // stays true.
        let map_width = screen_width / canvas.cam_zoom;
        let map_height = screen_height / canvas.cam_zoom;
        let top_left = Pt2D::new(
            origin.x() - (map_width / 2.0),
            origin.y() - (map_height / 2.0),
        );
        let row_height = map_height / (choices.len() as f64);

        Menu {
            prompt,
            choices,
            // TODO Different for wizards
            current_idx: None,
            origin,
            first_choice_row: Polygon::rectangle_topleft(top_left, map_width, row_height),
            row_height,
        }
    }

    pub fn event(&mut self, ev: Event, canvas: &Canvas) -> InputResult<T> {
        // Handle the mouse
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
                if self
                    .first_choice_row
                    .translate(0.0, (i as f64) * self.row_height)
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
            return InputResult::StillActive;
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
