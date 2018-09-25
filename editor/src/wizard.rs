use abstutil;
use ezgui::{Canvas, GfxCtx, InputResult, Menu, TextBox, UserInput};
use geom::Polygon;
use map_model::Map;
use sim::{Neighborhood, Tick};
use std::any::Any;
use std::collections::VecDeque;

pub struct Wizard {
    alive: bool,
    tb: Option<TextBox>,
    string_menu: Option<Menu<()>>,
    neighborhood_menu: Option<Menu<Neighborhood>>,

    // In the order of queries made
    confirmed_state: Vec<Box<Cloneable>>,
}

impl Wizard {
    pub fn new() -> Wizard {
        Wizard {
            alive: true,
            tb: None,
            string_menu: None,
            neighborhood_menu: None,
            confirmed_state: Vec::new(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        if let Some(ref menu) = self.string_menu {
            menu.draw(g, canvas);
        }
        if let Some(ref menu) = self.neighborhood_menu {
            menu.draw(g, canvas);
            g.draw_polygon(
                [0.0, 0.0, 1.0, 0.6],
                &Polygon::new(&menu.current_choice().points),
            );
        }
        if let Some(ref tb) = self.tb {
            tb.draw(g, canvas);
        }
    }

    pub fn wrap<'a>(&'a mut self, input: &'a mut UserInput, map: &'a Map) -> WrappedWizard<'a> {
        assert!(self.alive);

        let ready_results = VecDeque::from(self.confirmed_state.clone());
        WrappedWizard {
            wizard: self,
            input,
            map,
            ready_results,
        }
    }

    pub fn aborted(&self) -> bool {
        !self.alive
    }

    fn input_with_text_box<R: Cloneable>(
        &mut self,
        query: &str,
        input: &mut UserInput,
        parser: Box<Fn(String) -> Option<R>>,
    ) -> Option<R> {
        assert!(self.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if input.has_been_consumed() {
            return None;
        }

        if self.tb.is_none() {
            self.tb = Some(TextBox::new(query));
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
                    warn!("Invalid input {}", line);
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
    // TODO a workflow needs the map name. fine?
    pub map: &'a Map,

    // The downcasts are safe iff the queries made to the wizard are deterministic.
    ready_results: VecDeque<Box<Cloneable>>,
}

impl<'a> WrappedWizard<'a> {
    fn input_something<R: 'static + Clone + Cloneable>(
        &mut self,
        query: &str,
        parser: Box<Fn(String) -> Option<R>>,
    ) -> Option<R> {
        if !self.ready_results.is_empty() {
            let first = self.ready_results.pop_front().unwrap();
            let item: &R = first.as_any().downcast_ref::<R>().unwrap();
            return Some(item.clone());
        }
        if let Some(obj) = self.wizard.input_with_text_box(query, self.input, parser) {
            self.wizard.confirmed_state.push(Box::new(obj.clone()));
            Some(obj)
        } else {
            None
        }
    }

    pub fn input_string(&mut self, query: &str) -> Option<String> {
        self.input_something(query, Box::new(|line| Some(line)))
    }

    pub fn input_usize(&mut self, query: &str) -> Option<usize> {
        self.input_something(query, Box::new(|line| line.parse::<usize>().ok()))
    }

    pub fn input_tick(&mut self, query: &str) -> Option<Tick> {
        self.input_something(query, Box::new(|line| Tick::parse(&line)))
    }

    pub fn input_percent(&mut self, query: &str) -> Option<f64> {
        self.input_something(
            query,
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

    pub fn choose(&mut self, query: &str, choices: Vec<&str>) -> Option<String> {
        if !self.ready_results.is_empty() {
            return Some(
                self.ready_results
                    .pop_front()
                    .unwrap()
                    .as_any()
                    .downcast_ref::<String>()
                    .unwrap()
                    .clone(),
            );
        }

        if self.wizard.string_menu.is_none() {
            self.wizard.string_menu = Some(Menu::new(
                query,
                choices.into_iter().map(|s| (s.to_string(), ())).collect(),
            ));
        }

        if let Some((choice, _)) = input_with_menu(
            &mut self.wizard.string_menu,
            &mut self.wizard.alive,
            self.input,
        ) {
            self.wizard.confirmed_state.push(Box::new(choice.clone()));
            Some(choice)
        } else {
            None
        }
    }

    pub fn choose_neighborhood(&mut self, query: &str) -> Option<String> {
        if !self.ready_results.is_empty() {
            return Some(
                self.ready_results
                    .pop_front()
                    .unwrap()
                    .as_any()
                    .downcast_ref::<String>()
                    .unwrap()
                    .clone(),
            );
        }

        if self.wizard.neighborhood_menu.is_none() {
            self.wizard.neighborhood_menu = Some(Menu::new(
                query,
                abstutil::load_all_objects("neighborhoods", self.map.get_name()),
            ));
        }

        if let Some((name, _)) = input_with_menu(
            &mut self.wizard.neighborhood_menu,
            &mut self.wizard.alive,
            self.input,
        ) {
            self.wizard.confirmed_state.push(Box::new(name.clone()));
            Some(name)
        } else {
            None
        }
    }
}

// The caller initializes the menu, if needed. Pass in Option that must be Some().
// Bit weird to be a free function, but need to borrow a different menu and also the alive bit.
fn input_with_menu<T: Clone>(
    menu: &mut Option<Menu<T>>,
    alive: &mut bool,
    input: &mut UserInput,
) -> Option<(String, T)> {
    assert!(*alive);

    // Otherwise, we try to use one event for two inputs potentially
    if input.has_been_consumed() {
        return None;
    }

    match menu.as_mut().unwrap().event(input) {
        InputResult::Canceled => {
            *menu = None;
            *alive = false;
            None
        }
        InputResult::StillActive => None,
        InputResult::Done(name, poly) => {
            *menu = None;
            Some((name, poly))
        }
    }
}

// Trick to make a cloneable Any from
// https://stackoverflow.com/questions/30353462/how-to-clone-a-struct-storing-a-boxed-trait-object/30353928#30353928.

trait Cloneable: CloneableImpl {}

trait CloneableImpl {
    fn clone_box(&self) -> Box<Cloneable>;
    fn as_any(&self) -> &Any;
}

impl<T> CloneableImpl for T
where
    T: 'static + Cloneable + Clone,
{
    fn clone_box(&self) -> Box<Cloneable> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &Any {
        self
    }
}

impl Clone for Box<Cloneable> {
    fn clone(&self) -> Box<Cloneable> {
        self.clone_box()
    }
}

impl Cloneable for String {}
impl Cloneable for usize {}
impl Cloneable for Tick {}
impl Cloneable for f64 {}
