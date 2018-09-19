// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use piston::input::{Button, Event, Key, PressEvent};

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

    pub fn lines_to_display(&self) -> Vec<String> {
        // TODO dont copy
        let mut copy = self.choices.clone();
        copy[self.current_idx] = format!("---> {}", copy[self.current_idx]);
        copy
    }
}
