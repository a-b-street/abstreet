use piston::input::{Button, Event, Key, PressEvent};
use TextOSD;

pub enum MenuResult {
    Canceled,
    StillActive,
    Done(String),
}

pub struct Menu {
    choices: Vec<String>,
    current_idx: usize,
}

impl Menu {
    pub fn new(choices: Vec<String>) -> Menu {
        Menu {
            choices,
            current_idx: 0,
        }
    }

    // TODO take UserInput
    pub fn event(&mut self, ev: &Event) -> MenuResult {
        if let Some(Button::Keyboard(Key::Escape)) = ev.press_args() {
            return MenuResult::Canceled;
        }

        if let Some(Button::Keyboard(Key::Return)) = ev.press_args() {
            return MenuResult::Done(self.choices[self.current_idx].clone());
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

        MenuResult::StillActive
    }

    // TODO different API... handle menus bigger than the screen, actually do scroll. maybe always
    // display one size for the menu, just dont fill everything out
    pub fn get_osd(&self) -> TextOSD {
        let mut osd = TextOSD::new();
        for (idx, line) in self.choices.iter().enumerate() {
            if self.current_idx == idx {
                osd.add_highlighted_line(line.clone());
            } else {
                osd.add_line(line.clone());
            }
        }
        osd
    }

    pub fn current_choice(&self) -> &String {
        &self.choices[self.current_idx]
    }
}
