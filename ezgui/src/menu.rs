use crate::{text, Canvas, Color, Event, GfxCtx, InputResult, Key, Text};
use geom::{Polygon, Pt2D};

// Stores some associated data with each choice
pub struct Menu<T: Clone> {
    prompt: Option<String>,
    // The bool is whether this choice is active or not
    choices: Vec<(Option<Key>, String, bool, T)>,
    current_idx: Option<usize>,

    top_left: Pt2D,
    first_choice_row: Polygon,
    row_height: f64,
}

pub enum Position {
    CenteredAt(Pt2D),
    TopLeft(Pt2D),
}

impl<T: Clone> Menu<T> {
    pub fn new(
        prompt: Option<String>,
        choices: Vec<(Option<Key>, String, bool, T)>,
        select_first: bool,
        pos: Position,
        canvas: &Canvas,
    ) -> Menu<T> {
        if choices.is_empty() {
            panic!("Can't create a menu without choices for {:?}", prompt);
        }

        // Calculate geometry.
        let mut txt = Text::new();
        if let Some(ref line) = prompt {
            txt.add_line(line.to_string());
        }
        for (hotkey, choice, _, _) in &choices {
            if let Some(key) = hotkey {
                txt.add_line(format!("{} - {}", key.describe(), choice));
            } else {
                txt.add_line(choice.to_string());
            }
        }
        let (screen_width, screen_height) = canvas.text_dims(&txt);
        // Once a menu is created, all other controls (like zooming) are disabled, so this value
        // stays true.
        let map_width = screen_width / canvas.cam_zoom;
        let map_height = screen_height / canvas.cam_zoom;
        let row_height = map_height / (txt.num_lines() as f64);
        let top_left = match pos {
            Position::TopLeft(pt) => pt,
            Position::CenteredAt(at) => {
                let pt = Pt2D::new(at.x() - (map_width / 2.0), at.y() - (map_height / 2.0));
                if prompt.is_some() {
                    pt.offset(0.0, row_height)
                } else {
                    pt
                }
            }
        };

        Menu {
            prompt,
            choices,
            current_idx: if select_first { Some(0) } else { None },
            top_left,
            first_choice_row: Polygon::rectangle_topleft(top_left, map_width, row_height),
            row_height,
        }
    }

    pub fn event(&mut self, ev: Event, canvas: &Canvas) -> InputResult<T> {
        // Handle the mouse
        if ev == Event::LeftMouseButtonDown {
            if let Some(i) = self.current_idx {
                let (_, choice, active, data) = self.choices[i].clone();
                if active {
                    return InputResult::Done(choice, data);
                } else {
                    return InputResult::StillActive;
                }
            } else {
                return InputResult::Canceled;
            }
        } else if ev == Event::RightMouseButtonDown {
            return InputResult::Canceled;
        } else if let Event::MouseMovedTo(x, y) = ev {
            let cursor_pt = canvas.screen_to_map((x, y));
            let mut matched = false;
            for i in 0..self.choices.len() {
                if self.choices[i].2
                    && self
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
                let (_, name, active, data) = self.choices[idx].clone();
                if active {
                    return InputResult::Done(name, data);
                } else {
                    return InputResult::StillActive;
                }
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
        if let Some(ref line) = self.prompt {
            txt.add_styled_line(
                line.to_string(),
                text::TEXT_FG_COLOR,
                Some(text::TEXT_QUERY_COLOR),
            );
        }
        for (idx, (hotkey, choice, active, _)) in self.choices.iter().enumerate() {
            let bg = if Some(idx) == self.current_idx {
                Some(Color::WHITE)
            } else {
                None
            };
            if *active {
                if let Some(key) = hotkey {
                    txt.add_styled_line(key.describe(), Color::BLUE, bg);
                    txt.append(format!(" - {}", choice), text::TEXT_FG_COLOR, bg);
                } else {
                    txt.add_styled_line(choice.to_string(), text::TEXT_FG_COLOR, bg);
                }
            } else {
                if let Some(key) = hotkey {
                    txt.add_styled_line(
                        format!("{} - {}", key.describe(), choice),
                        Color::grey(0.8),
                        bg,
                    );
                } else {
                    txt.add_styled_line(format!("{}", choice), Color::grey(0.8), bg);
                }
            }
        }
        canvas.draw_text_at_topleft(g, txt, self.top_left);
    }

    pub fn current_choice(&self) -> Option<&T> {
        let idx = self.current_idx?;
        Some(&self.choices[idx].3)
    }
}
