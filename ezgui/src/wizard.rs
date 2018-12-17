use crate::menu::Menu;
use crate::{Canvas, GfxCtx, InputResult, Key, LogScroller, TextBox, UserInput};
use abstutil::Cloneable;
use log::warn;
use std::collections::VecDeque;

pub struct Wizard {
    alive: bool,
    tb: Option<TextBox>,
    menu: Option<Menu<Box<Cloneable>>>,
    log_scroller: Option<LogScroller>,

    // In the order of queries made
    confirmed_state: Vec<Box<Cloneable>>,
}

impl Wizard {
    pub fn new() -> Wizard {
        Wizard {
            alive: true,
            tb: None,
            menu: None,
            log_scroller: None,
            confirmed_state: Vec::new(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        if let Some(ref menu) = self.menu {
            menu.draw(g, canvas);
        }
        if let Some(ref tb) = self.tb {
            tb.draw(g, canvas);
        }
        if let Some(ref s) = self.log_scroller {
            s.draw(g, canvas);
        }
    }

    pub fn wrap<'a>(
        &'a mut self,
        input: &'a mut UserInput,
        canvas: &'a Canvas,
    ) -> WrappedWizard<'a> {
        assert!(self.alive);

        let ready_results = VecDeque::from(self.confirmed_state.clone());
        WrappedWizard {
            wizard: self,
            input,
            canvas,
            ready_results,
        }
    }

    pub fn aborted(&self) -> bool {
        !self.alive
    }

    // The caller can ask for any type at any time
    pub fn current_menu_choice<R: 'static + Cloneable>(&self) -> Option<&R> {
        if let Some(ref menu) = self.menu {
            let item: &R = menu.current_choice()?.as_any().downcast_ref::<R>()?;
            return Some(item);
        }
        None
    }

    fn input_with_text_box<R: Cloneable>(
        &mut self,
        query: &str,
        prefilled: Option<String>,
        input: &mut UserInput,
        parser: Box<Fn(String) -> Option<R>>,
    ) -> Option<R> {
        assert!(self.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if input.has_been_consumed() {
            return None;
        }

        if self.tb.is_none() {
            self.tb = Some(TextBox::new(query, prefilled));
        }

        match self.tb.as_mut().unwrap().event(input) {
            InputResult::StillActive => None,
            InputResult::Canceled => {
                self.alive = false;
                None
            }
            InputResult::Done(line, _) => {
                self.tb = None;
                if let Some(result) = parser(line.clone()) {
                    Some(result)
                } else {
                    warn!(target: "UI", "Invalid input {}", line);
                    None
                }
            }
        }
    }
}

// Lives only for one frame -- bundles up temporary things like UserInput and statefully serve
// prior results.
pub struct WrappedWizard<'a> {
    wizard: &'a mut Wizard,
    input: &'a mut UserInput,
    canvas: &'a Canvas,

    // The downcasts are safe iff the queries made to the wizard are deterministic.
    ready_results: VecDeque<Box<Cloneable>>,
}

impl<'a> WrappedWizard<'a> {
    pub fn input_something<R: 'static + Clone + Cloneable>(
        &mut self,
        query: &str,
        prefilled: Option<String>,
        parser: Box<Fn(String) -> Option<R>>,
    ) -> Option<R> {
        if !self.ready_results.is_empty() {
            let first = self.ready_results.pop_front().unwrap();
            let item: &R = first.as_any().downcast_ref::<R>().unwrap();
            return Some(item.clone());
        }
        if let Some(obj) = self
            .wizard
            .input_with_text_box(query, prefilled, self.input, parser)
        {
            self.wizard.confirmed_state.push(Box::new(obj.clone()));
            Some(obj)
        } else {
            None
        }
    }

    pub fn input_string(&mut self, query: &str) -> Option<String> {
        self.input_something(query, None, Box::new(Some))
    }

    pub fn input_string_prefilled(&mut self, query: &str, prefilled: String) -> Option<String> {
        self.input_something(query, Some(prefilled), Box::new(Some))
    }

    pub fn input_usize(&mut self, query: &str) -> Option<usize> {
        self.input_something(query, None, Box::new(|line| line.parse::<usize>().ok()))
    }

    pub fn input_usize_prefilled(&mut self, query: &str, prefilled: String) -> Option<usize> {
        self.input_something(
            query,
            Some(prefilled),
            Box::new(|line| line.parse::<usize>().ok()),
        )
    }

    pub fn input_percent(&mut self, query: &str) -> Option<f64> {
        self.input_something(
            query,
            None,
            Box::new(|line| {
                line.parse::<f64>().ok().and_then(|num| {
                    if num >= 0.0 && num <= 1.0 {
                        Some(num)
                    } else {
                        None
                    }
                })
            }),
        )
    }

    pub fn choose_something<R: 'static + Clone + Cloneable>(
        &mut self,
        query: &str,
        choices_generator: Box<Fn() -> Vec<(String, R)>>,
    ) -> Option<(String, R)> {
        if !self.ready_results.is_empty() {
            let first = self.ready_results.pop_front().unwrap();
            // We have to downcast twice! \o/
            let pair: &(String, Box<Cloneable>) = first
                .as_any()
                .downcast_ref::<(String, Box<Cloneable>)>()
                .unwrap();
            let item: &R = pair.1.as_any().downcast_ref::<R>().unwrap();
            return Some((pair.0.to_string(), item.clone()));
        }

        // If the menu was empty, wait for the user to acknowledge the text-box before aborting the
        // wizard.
        if self.wizard.log_scroller.is_some() {
            if self.wizard.log_scroller.as_mut().unwrap().event(self.input) {
                self.wizard.log_scroller = None;
                self.wizard.alive = false;
            }
            return None;
        }

        if self.wizard.menu.is_none() {
            let choices: Vec<(String, R)> = choices_generator();
            if choices.is_empty() {
                self.wizard.log_scroller = Some(LogScroller::new_from_lines(vec![format!(
                    "No choices for \"{}\", canceling wizard",
                    query
                )]));
                return None;
            }
            let boxed_choices: Vec<(Option<Key>, String, Box<Cloneable>)> = choices
                .iter()
                .map(|(s, item)| (None, s.to_string(), item.clone_box()))
                .collect();
            self.wizard.menu = Some(Menu::new(
                Some(query.to_string()),
                boxed_choices,
                self.canvas.center_to_map_pt(),
                self.canvas,
            ));
        }

        assert!(self.wizard.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if self.input.has_been_consumed() {
            return None;
        }

        let ev = self.input.use_event_directly().unwrap();
        match self.wizard.menu.as_mut().unwrap().event(ev, self.canvas) {
            InputResult::Canceled => {
                self.wizard.menu = None;
                self.wizard.alive = false;
                None
            }
            InputResult::StillActive => None,
            InputResult::Done(choice, item) => {
                self.wizard.menu = None;
                self.wizard
                    .confirmed_state
                    .push(Box::new((choice.to_string(), item.clone())));
                let downcasted_item: &R = item.as_any().downcast_ref::<R>().unwrap();
                Some((choice, downcasted_item.clone()))
            }
        }
    }

    pub fn choose_string(&mut self, query: &str, choices: Vec<&str>) -> Option<String> {
        // Clone the choices outside of the closure to get around the fact that choices_generator's
        // lifetime isn't correctly specified.
        let copied_choices: Vec<(String, ())> =
            choices.into_iter().map(|s| (s.to_string(), ())).collect();
        self.choose_something(query, Box::new(move || copied_choices.clone()))
            .map(|(s, _)| s)
    }
}
