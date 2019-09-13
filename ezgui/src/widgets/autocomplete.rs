use crate::{text, Event, GfxCtx, InputResult, Key, Line, Text, UserInput, CENTERED};
use simsearch::SimSearch;
use std::collections::{BTreeMap, HashSet};
use std::hash::Hash;

const NUM_SEARCH_RESULTS: usize = 5;

pub struct Autocomplete<T: Clone + Hash + Eq> {
    prompt: String,
    choices: BTreeMap<String, HashSet<T>>,
    // Maps index to choice
    search_map: Vec<String>,
    search: SimSearch<usize>,

    line: String,
    cursor_x: usize,
    shift_pressed: bool,
    current_results: Vec<usize>,
    cursor_y: usize,
}

impl<T: Clone + Hash + Eq> Autocomplete<T> {
    pub fn new(prompt: &str, choices_list: Vec<(String, T)>) -> Autocomplete<T> {
        let mut choices = BTreeMap::new();
        for (name, data) in choices_list {
            if !choices.contains_key(&name) {
                choices.insert(name.clone(), HashSet::new());
            }
            choices.get_mut(&name).unwrap().insert(data);
        }
        let mut search_map = Vec::new();
        let mut search = SimSearch::new();
        let mut current_results = Vec::new();
        for (idx, name) in choices.keys().enumerate() {
            search_map.push(name.to_string());
            search.insert(idx, name);
            if idx < NUM_SEARCH_RESULTS {
                current_results.push(idx);
            }
        }

        Autocomplete {
            prompt: prompt.to_string(),
            choices,
            search_map,
            search,

            line: String::new(),
            cursor_x: 0,
            shift_pressed: false,
            current_results,
            cursor_y: 0,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        let mut txt = Text::prompt(&self.prompt);

        txt.add(Line(&self.line[0..self.cursor_x]));
        if self.cursor_x < self.line.len() {
            // TODO This "cursor" looks awful!
            txt.append_all(vec![
                Line("|").fg(text::SELECTED_COLOR),
                Line(&self.line[self.cursor_x..=self.cursor_x]),
                Line(&self.line[self.cursor_x + 1..]),
            ]);
        } else {
            txt.append(Line("|").fg(text::SELECTED_COLOR));
        }

        for (idx, id) in self.current_results.iter().enumerate() {
            if idx == self.cursor_y {
                txt.add_highlighted(Line(&self.search_map[*id]), text::SELECTED_COLOR);
            } else {
                txt.add(Line(&self.search_map[*id]));
            }
        }

        g.draw_blocking_text(&txt, CENTERED);
    }

    pub fn event(&mut self, input: &mut UserInput) -> InputResult<HashSet<T>> {
        let maybe_ev = input.use_event_directly();
        if maybe_ev.is_none() {
            return InputResult::StillActive;
        }
        let ev = maybe_ev.unwrap();

        if ev == Event::KeyPress(Key::Escape) {
            return InputResult::Canceled;
        } else if ev == Event::KeyPress(Key::Enter) {
            if self.current_results.is_empty() {
                return InputResult::Canceled;
            }
            let name = &self.search_map[self.current_results[self.cursor_y]];
            return InputResult::Done(name.to_string(), self.choices.remove(name).unwrap());
        } else if ev == Event::KeyPress(Key::LeftShift) {
            self.shift_pressed = true;
        } else if ev == Event::KeyRelease(Key::LeftShift) {
            self.shift_pressed = false;
        } else if ev == Event::KeyPress(Key::LeftArrow) {
            if self.cursor_x > 0 {
                self.cursor_x -= 1;
            }
        } else if ev == Event::KeyPress(Key::RightArrow) {
            self.cursor_x = (self.cursor_x + 1).min(self.line.len());
        } else if ev == Event::KeyPress(Key::UpArrow) {
            if self.cursor_y > 0 {
                self.cursor_y -= 1;
            }
        } else if ev == Event::KeyPress(Key::DownArrow) {
            self.cursor_y = (self.cursor_y + 1).min(self.current_results.len() - 1);
        } else if ev == Event::KeyPress(Key::Backspace) {
            if self.cursor_x > 0 {
                self.line.remove(self.cursor_x - 1);
                self.cursor_x -= 1;

                self.current_results = self.search.search(&self.line);
                self.current_results.truncate(NUM_SEARCH_RESULTS);
                self.cursor_y = 0;
            }
        } else if let Event::KeyPress(key) = ev {
            if let Some(c) = key.to_char(self.shift_pressed) {
                self.line.insert(self.cursor_x, c);
                self.cursor_x += 1;

                self.current_results = self.search.search(&self.line);
                self.current_results.truncate(NUM_SEARCH_RESULTS);
                self.cursor_y = 0;
            }
        };
        InputResult::StillActive
    }
}
