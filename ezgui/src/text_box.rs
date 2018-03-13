// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use piston::input::{Button, Event, Key, PressEvent, ReleaseEvent};

// TODO right now, only a single line

pub struct TextBox {
    // TODO A rope would be cool.
    pub line: String,
    cursor_x: usize,
    shift_pressed: bool,
}

impl TextBox {
    pub fn new() -> TextBox {
        TextBox {
            line: String::from(""),
            cursor_x: 0,
            shift_pressed: false,
        }
    }

    // Returns true if the user confirmed their entry.
    pub fn event(&mut self, ev: &Event) -> bool {
        // Done?
        if let Some(Button::Keyboard(Key::Return)) = ev.press_args() {
            return true;
        }

        // Key state tracking
        if let Some(Button::Keyboard(Key::LShift)) = ev.press_args() {
            self.shift_pressed = true;
        }
        if let Some(Button::Keyboard(Key::LShift)) = ev.release_args() {
            self.shift_pressed = false;
        }

        // Cursor movement
        if let Some(Button::Keyboard(Key::Left)) = ev.press_args() {
            if self.cursor_x > 0 {
                self.cursor_x -= 1;
            }
        }
        if let Some(Button::Keyboard(Key::Right)) = ev.press_args() {
            self.cursor_x = (self.cursor_x + 1).min(self.line.len());
        }

        // Backspace
        if let Some(Button::Keyboard(Key::Backspace)) = ev.press_args() {
            if self.cursor_x > 0 {
                self.line.remove(self.cursor_x - 1);
                self.cursor_x -= 1;
            }
        }

        // Insert
        if let Some(Button::Keyboard(key)) = ev.press_args() {
            let new_char = match key {
                Key::Space => Some(' '),
                Key::A if self.shift_pressed => Some('A'),
                Key::B if self.shift_pressed => Some('B'),
                Key::C if self.shift_pressed => Some('C'),
                Key::D if self.shift_pressed => Some('D'),
                Key::E if self.shift_pressed => Some('E'),
                Key::F if self.shift_pressed => Some('F'),
                Key::G if self.shift_pressed => Some('G'),
                Key::H if self.shift_pressed => Some('H'),
                Key::I if self.shift_pressed => Some('I'),
                Key::J if self.shift_pressed => Some('J'),
                Key::K if self.shift_pressed => Some('K'),
                Key::L if self.shift_pressed => Some('L'),
                Key::M if self.shift_pressed => Some('M'),
                Key::N if self.shift_pressed => Some('N'),
                Key::O if self.shift_pressed => Some('O'),
                Key::P if self.shift_pressed => Some('P'),
                Key::Q if self.shift_pressed => Some('Q'),
                Key::R if self.shift_pressed => Some('R'),
                Key::S if self.shift_pressed => Some('S'),
                Key::T if self.shift_pressed => Some('T'),
                Key::U if self.shift_pressed => Some('U'),
                Key::V if self.shift_pressed => Some('V'),
                Key::W if self.shift_pressed => Some('W'),
                Key::X if self.shift_pressed => Some('X'),
                Key::Y if self.shift_pressed => Some('Y'),
                Key::Z if self.shift_pressed => Some('Z'),
                Key::A => Some('a'),
                Key::B => Some('b'),
                Key::C => Some('c'),
                Key::D => Some('d'),
                Key::E => Some('e'),
                Key::F => Some('f'),
                Key::G => Some('g'),
                Key::H => Some('h'),
                Key::I => Some('i'),
                Key::J => Some('j'),
                Key::K => Some('k'),
                Key::L => Some('l'),
                Key::M => Some('m'),
                Key::N => Some('n'),
                Key::O => Some('o'),
                Key::P => Some('p'),
                Key::Q => Some('q'),
                Key::R => Some('r'),
                Key::S => Some('s'),
                Key::T => Some('t'),
                Key::U => Some('u'),
                Key::V => Some('v'),
                Key::W => Some('w'),
                Key::X => Some('x'),
                Key::Y => Some('y'),
                Key::Z => Some('z'),
                _ => None,
            };
            if let Some(c) = new_char {
                self.line.insert(self.cursor_x, c);
                self.cursor_x += 1;
            }
        }
        false
    }
}
