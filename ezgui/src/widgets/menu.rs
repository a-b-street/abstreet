use crate::screen_geom::ScreenRectangle;
use crate::{text, Canvas, Event, GfxCtx, InputResult, Key, ScreenPt, Text};

// Stores some associated data with each choice
pub struct Menu<T: Clone> {
    prompt: Text,
    // The bool is whether this choice is active or not
    choices: Vec<(Option<Key>, String, bool, T)>,
    current_idx: Option<usize>,
    mouse_in_bounds: bool,
    keys_enabled: bool,
    hideable: bool,
    hidden: bool,
    pos: Position,
    geom: Geometry,
}

struct Geometry {
    row_height: f64,
    top_left: ScreenPt,
    first_choice_row: ScreenRectangle,
    total_height: f64,
}

#[derive(Clone)]
pub enum Position {
    ScreenCenter,
    SomeCornerAt(ScreenPt),
    TopRightOfScreen,
}

impl Position {
    fn geometry<T>(
        &self,
        canvas: &Canvas,
        prompt: Text,
        choices: &Vec<(Option<Key>, String, bool, T)>,
    ) -> Geometry {
        // This is actually a constant, effectively...
        let row_height = canvas.line_height(text::FONT_SIZE);

        let mut txt = prompt;
        let (_, prompt_height) = canvas.text_dims(&txt);
        for (hotkey, choice, _, _) in choices {
            if let Some(key) = hotkey {
                txt.add_line(format!("{} - {}", key.describe(), choice));
            } else {
                txt.add_line(choice.to_string());
            }
        }
        let (total_width, total_height) = canvas.text_dims(&txt);

        let top_left = match self {
            Position::SomeCornerAt(pt) => {
                // TODO Ideally also avoid covered canvas areas (modal menu)
                if pt.x + total_width < canvas.window_width {
                    // pt.x is the left corner
                    if pt.y + total_height < canvas.window_height {
                        // pt.y is the top corner
                        *pt
                    } else {
                        // pt.y is the bottom corner
                        ScreenPt::new(pt.x, pt.y - total_height)
                    }
                } else {
                    // pt.x is the right corner
                    if pt.y + total_height < canvas.window_height {
                        // pt.y is the top corner
                        ScreenPt::new(pt.x - total_width, pt.y)
                    } else {
                        // pt.y is the bottom corner
                        ScreenPt::new(pt.x - total_width, pt.y - total_height)
                    }
                }
            }
            Position::ScreenCenter => {
                let mut pt = canvas.center_to_screen_pt();
                pt.x -= total_width / 2.0;
                pt.y -= total_height / 2.0;
                pt
            }
            Position::TopRightOfScreen => ScreenPt::new(canvas.window_width - total_width, 0.0),
        };
        Geometry {
            row_height,
            top_left,
            first_choice_row: ScreenRectangle {
                x1: top_left.x,
                y1: top_left.y + prompt_height,
                x2: top_left.x + total_width,
                y2: top_left.y + prompt_height + row_height,
            },
            total_height,
        }
    }
}

impl<T: Clone> Menu<T> {
    pub fn new(
        prompt: Text,
        raw_choices: Vec<(Option<Key>, String, T)>,
        keys_enabled: bool,
        hideable: bool,
        pos: Position,
        canvas: &Canvas,
    ) -> Menu<T> {
        if raw_choices.is_empty() {
            panic!("Can't create a menu without choices for {:?}", prompt);
        }
        // TODO Make sure hotkeys aren't used twice.
        // All choices start active.
        let choices = raw_choices
            .into_iter()
            .map(|(key, choice, data)| (key, choice, true, data))
            .collect();
        let geom = pos.geometry(canvas, prompt.clone(), &choices);

        Menu {
            prompt,
            choices,
            current_idx: if keys_enabled { Some(0) } else { None },
            keys_enabled,
            // TODO Bit of a hack, but eh.
            mouse_in_bounds: !keys_enabled,
            pos,
            hideable,
            hidden: false,
            geom,
        }
    }

    pub fn event(&mut self, ev: Event, canvas: &Canvas) -> InputResult<T> {
        if !self.hidden {
            // Handle the mouse
            if ev == Event::LeftMouseButtonDown {
                if let Some(i) = self.current_idx {
                    let (_, choice, active, data) = self.choices[i].clone();
                    if active && self.mouse_in_bounds {
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
                if !canvas.is_dragging() {
                    for i in 0..self.choices.len() {
                        if self.choices[i].2
                            && self
                                .geom
                                .first_choice_row
                                .translate(0.0, (i as f64) * self.geom.row_height)
                                .contains(pt)
                        {
                            self.current_idx = Some(i);
                            self.mouse_in_bounds = true;
                            return InputResult::StillActive;
                        }
                    }
                    self.mouse_in_bounds = false;
                    if !self.keys_enabled {
                        self.current_idx = None;
                    }
                    return InputResult::StillActive;
                }
            }

            // Handle keys
            if self.keys_enabled {
                let idx = self.current_idx.unwrap();
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
        }

        if self.hideable {
            if ev == Event::KeyPress(Key::Tab) {
                if self.hidden {
                    self.hidden = false;
                } else {
                    self.hidden = true;
                    self.current_idx = None;
                }
                self.recalculate_geom(canvas);
            }
        }

        if let Event::KeyPress(key) = ev {
            for (maybe_key, choice, active, data) in &self.choices {
                if *active && Some(key) == *maybe_key {
                    return InputResult::Done(choice.to_string(), data.clone());
                }
            }
        }

        // This is always an option, but do this last, in case Escape is a hotkey of a menu choice.
        if ev == Event::KeyPress(Key::Escape) {
            return InputResult::Canceled;
        }

        if let Event::WindowResized(_, _) = ev {
            self.recalculate_geom(canvas);
        }

        InputResult::StillActive
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        let mut txt = self.prompt.clone();
        if !self.hidden {
            for (idx, (hotkey, choice, active, _)) in self.choices.iter().enumerate() {
                let bg = if Some(idx) == self.current_idx {
                    Some(text::SELECTED_COLOR)
                } else {
                    None
                };
                if *active {
                    if let Some(key) = hotkey {
                        txt.add_styled_line(key.describe(), Some(text::HOTKEY_COLOR), bg, None);
                        txt.append(format!(" - {}", choice), None);
                    } else {
                        txt.add_styled_line(choice.to_string(), None, bg, None);
                    }
                } else {
                    if let Some(key) = hotkey {
                        txt.add_styled_line(
                            format!("{} - {}", key.describe(), choice),
                            Some(text::INACTIVE_CHOICE_COLOR),
                            bg,
                            None,
                        );
                    } else {
                        txt.add_styled_line(
                            choice.to_string(),
                            Some(text::INACTIVE_CHOICE_COLOR),
                            bg,
                            None,
                        );
                    }
                }
            }
        }
        g.canvas.mark_covered_area(ScreenRectangle {
            x1: self.geom.top_left.x,
            y1: self.geom.top_left.y,
            x2: self.geom.first_choice_row.x2,
            y2: self.geom.top_left.y + self.geom.total_height,
        });
        g.draw_text_at_screenspace_topleft(&txt, self.geom.top_left);
    }

    pub fn current_choice(&self) -> Option<&T> {
        let idx = self.current_idx?;
        Some(&self.choices[idx].3)
    }

    pub fn active_choices(&self) -> Vec<&T> {
        self.choices
            .iter()
            .filter_map(|(_, _, active, data)| if *active { Some(data) } else { None })
            .collect()
    }

    // If there's no matching choice, be silent. The two callers don't care.
    pub fn mark_active(&mut self, choice: &str) {
        for (_, action, ref mut active, _) in self.choices.iter_mut() {
            if choice == action {
                if *active {
                    panic!("Menu choice for {} was already active", choice);
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

    pub fn change_prompt(&mut self, prompt: Text, canvas: &Canvas) {
        self.prompt = prompt;
        self.recalculate_geom(canvas);
    }

    pub fn get_bottom_left(&self) -> ScreenPt {
        ScreenPt::new(
            self.geom.top_left.x,
            self.geom.top_left.y + self.geom.total_height,
        )
    }

    fn recalculate_geom(&mut self, canvas: &Canvas) {
        if self.hidden {
            self.geom = self
                .pos
                .geometry::<()>(canvas, self.prompt.clone(), &Vec::new());
        } else {
            self.geom = self
                .pos
                .geometry(canvas, self.prompt.clone(), &self.choices);
        }
    }
}
