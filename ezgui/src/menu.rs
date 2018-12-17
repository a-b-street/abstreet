use crate::screen_geom::ScreenRectangle;
use crate::text::LINE_HEIGHT;
use crate::{text, Canvas, Color, Event, GfxCtx, InputResult, Key, ScreenPt, Text};

// Stores some associated data with each choice
pub struct Menu<T: Clone> {
    prompt: Option<String>,
    // The bool is whether this choice is active or not
    choices: Vec<(Option<Key>, String, bool, T)>,
    current_idx: Option<usize>,

    top_left: ScreenPt,
    first_choice_row: ScreenRectangle,
    row_height: f64,
}

pub enum Position {
    ScreenCenter,
    TopLeftAt(ScreenPt),
    TopRightOfScreen,
}

impl<T: Clone> Menu<T> {
    pub fn new(
        prompt: Option<String>,
        choices: Vec<(Option<Key>, String, T)>,
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
        for (hotkey, choice, _) in &choices {
            if let Some(key) = hotkey {
                txt.add_line(format!("{} - {}", key.describe(), choice));
            } else {
                txt.add_line(choice.to_string());
            }
        }
        let (total_width, total_height) = canvas.text_dims(&txt);
        let row_height = total_height / (txt.num_lines() as f64);
        let top_left = match pos {
            Position::TopLeftAt(pt) => pt,
            Position::ScreenCenter => {
                let mut pt = canvas.center_to_screen_pt();
                pt.x -= total_width / 2.0;
                pt.y -= total_height / 2.0;
                pt
            }
            Position::TopRightOfScreen => {
                ScreenPt::new((canvas.window_size.width as f64) - total_width, LINE_HEIGHT)
            }
        };

        Menu {
            // All choices start active.
            choices: choices
                .into_iter()
                .map(|(key, choice, data)| (key, choice, true, data))
                .collect(),
            current_idx: if select_first { Some(0) } else { None },
            top_left,
            first_choice_row: if prompt.is_some() {
                ScreenRectangle {
                    x1: top_left.x,
                    y1: top_left.y + row_height,
                    x2: top_left.x + total_width,
                    y2: top_left.y + (2.0 * row_height),
                }
            } else {
                ScreenRectangle {
                    x1: top_left.x,
                    y1: top_left.y,
                    x2: top_left.x + total_width,
                    y2: top_left.y + row_height,
                }
            },
            row_height,
            prompt,
        }
    }

    pub fn event(&mut self, ev: Event) -> InputResult<T> {
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
        } else if let Event::MouseMovedTo(pt) = ev {
            let mut matched = false;
            for i in 0..self.choices.len() {
                if self.choices[i].2
                    && self
                        .first_choice_row
                        .translate(0.0, (i as f64) * self.row_height)
                        .contains(pt)
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
        canvas.draw_text_at_screenspace_topleft(g, txt, self.top_left);
    }

    pub fn current_choice(&self) -> Option<&T> {
        let idx = self.current_idx?;
        Some(&self.choices[idx].3)
    }

    // If there's no matching choice, be silent. The two callers don't care.
    pub fn mark_active(&mut self, action_key: Key) {
        for (key, _, ref mut active, _) in self.choices.iter_mut() {
            if Some(action_key) == *key {
                if *active {
                    panic!("Menu choice with key {:?} was already active", action_key);
                }
                *active = true;
                return;
            }
        }
    }

    pub fn mark_all_inactive(&mut self) {
        for (_, _, ref mut active, _) in self.choices.iter_mut() {
            *active = false;
        }
    }
}
