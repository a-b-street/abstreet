use crate::widgets::text_box::TextBox;
use crate::widgets::{Menu, Position};
use crate::{
    EventCtx, GfxCtx, InputResult, LogScroller, MultiKey, SliderWithTextBox, Text, UserInput,
};
use abstutil::Cloneable;
use geom::Duration;
use std::collections::VecDeque;

pub struct Wizard {
    alive: bool,
    tb: Option<TextBox>,
    menu: Option<Menu<Box<Cloneable>>>,
    log_scroller: Option<LogScroller>,
    slider: Option<SliderWithTextBox>,

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
            slider: None,
            confirmed_state: Vec::new(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let Some(ref menu) = self.menu {
            menu.draw(g);
        }
        if let Some(ref tb) = self.tb {
            tb.draw(g);
        }
        if let Some(ref s) = self.log_scroller {
            s.draw(g);
        }
        if let Some(ref s) = self.slider {
            s.draw(g);
        }
    }

    pub fn wrap<'a, 'b>(&'a mut self, ctx: &'a mut EventCtx<'b>) -> WrappedWizard<'a, 'b> {
        assert!(self.alive);

        let ready_results = VecDeque::from(self.confirmed_state.clone());
        WrappedWizard {
            wizard: self,
            ctx,
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

    pub fn reset(&mut self) {
        assert!(self.tb.is_none());
        assert!(self.menu.is_none());
        assert!(self.log_scroller.is_none());
        assert!(self.slider.is_none());
        self.confirmed_state.clear();
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
                    println!("Invalid input {}", line);
                    None
                }
            }
        }
    }

    fn input_time_slider(
        &mut self,
        query: &str,
        low: Duration,
        high: Duration,
        ctx: &mut EventCtx,
    ) -> Option<Duration> {
        assert!(self.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if ctx.input.has_been_consumed() {
            return None;
        }

        if self.slider.is_none() {
            self.slider = Some(SliderWithTextBox::new(query, low, high, ctx.canvas));
        }

        match self.slider.as_mut().unwrap().event(ctx) {
            InputResult::StillActive => None,
            InputResult::Canceled => {
                self.alive = false;
                None
            }
            InputResult::Done(_, result) => {
                self.slider = None;
                Some(result)
            }
        }
    }
}

// Lives only for one frame -- bundles up temporary things like UserInput and statefully serve
// prior results.
pub struct WrappedWizard<'a, 'b> {
    wizard: &'a mut Wizard,
    ctx: &'a mut EventCtx<'b>,

    // The downcasts are safe iff the queries made to the wizard are deterministic.
    ready_results: VecDeque<Box<Cloneable>>,
}

impl<'a, 'b> WrappedWizard<'a, 'b> {
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
            .input_with_text_box(query, prefilled, self.ctx.input, parser)
        {
            self.wizard.confirmed_state.push(Box::new(obj.clone()));
            Some(obj)
        } else {
            None
        }
    }

    pub fn input_time_slider(
        &mut self,
        query: &str,
        low: Duration,
        high: Duration,
    ) -> Option<Duration> {
        if !self.ready_results.is_empty() {
            let first = self.ready_results.pop_front().unwrap();
            // TODO Simplify?
            let item: &Duration = first.as_any().downcast_ref::<Duration>().unwrap();
            return Some(*item);
        }
        if let Some(obj) = self.wizard.input_time_slider(query, low, high, self.ctx) {
            self.wizard.confirmed_state.push(Box::new(obj));
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
        choices_generator: Box<Fn() -> Vec<(Option<MultiKey>, String, R)>>,
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
            if self
                .wizard
                .log_scroller
                .as_mut()
                .unwrap()
                .event(self.ctx.input)
            {
                self.wizard.log_scroller = None;
                self.wizard.alive = false;
            }
            return None;
        }

        if self.wizard.menu.is_none() {
            let choices: Vec<(Option<MultiKey>, String, R)> = choices_generator();
            if choices.is_empty() {
                self.wizard.log_scroller = Some(LogScroller::new(
                    "Wizard".to_string(),
                    vec![format!("No choices for \"{}\", canceling wizard", query)],
                ));
                return None;
            }
            let boxed_choices: Vec<(Option<MultiKey>, String, Box<Cloneable>)> = choices
                .into_iter()
                .map(|(multikey, s, item)| (multikey, s, item.clone_box()))
                .collect();
            self.wizard.menu = Some(Menu::new(
                Text::prompt(query),
                vec![boxed_choices],
                true,
                false,
                Position::ScreenCenter,
                self.ctx.canvas,
            ));
        }

        assert!(self.wizard.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if self.ctx.input.has_been_consumed() {
            return None;
        }

        let ev = self.ctx.input.use_event_directly().unwrap();
        match self
            .wizard
            .menu
            .as_mut()
            .unwrap()
            .event(ev, self.ctx.canvas)
        {
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

    pub fn choose_something_no_keys<R: 'static + Clone + Cloneable>(
        &mut self,
        query: &str,
        choices_generator: Box<Fn() -> Vec<(String, R)>>,
    ) -> Option<(String, R)> {
        let wrapped_generator = Box::new(move || {
            choices_generator()
                .into_iter()
                .map(|(name, data)| (None, name, data))
                .collect()
        });
        self.choose_something(query, wrapped_generator)
    }

    pub fn choose_string(&mut self, query: &str, choices: Vec<&str>) -> Option<String> {
        // Clone the choices outside of the closure to get around the fact that choices_generator's
        // lifetime isn't correctly specified.
        let copied_choices: Vec<(Option<MultiKey>, String, ())> = choices
            .into_iter()
            .map(|s| (None, s.to_string(), ()))
            .collect();
        self.choose_something(query, Box::new(move || copied_choices.clone()))
            .map(|(s, _)| s)
    }

    pub fn choose_string_hotkeys(
        &mut self,
        query: &str,
        choices: Vec<(Option<MultiKey>, &str)>,
    ) -> Option<String> {
        // Clone the choices outside of the closure to get around the fact that choices_generator's
        // lifetime isn't correctly specified.
        let copied_choices: Vec<(Option<MultiKey>, String, ())> = choices
            .into_iter()
            .map(|(multikey, s)| (multikey, s.to_string(), ()))
            .collect();
        self.choose_something(query, Box::new(move || copied_choices.clone()))
            .map(|(s, _)| s)
    }

    pub fn aborted(&self) -> bool {
        self.wizard.aborted()
    }

    pub fn abort(&mut self) {
        self.wizard.alive = false;
    }

    // Note this will abort the wizard once done!
    pub fn acknowledge(&mut self, title: &str, lines: Vec<&str>) -> bool {
        if !self.ready_results.is_empty() {
            self.ready_results.pop_front();
            return true;
        }

        if self.wizard.log_scroller.is_none() {
            self.wizard.log_scroller = Some(LogScroller::new(
                title.to_string(),
                lines.into_iter().map(|l| l.to_string()).collect(),
            ));
        }
        if self
            .wizard
            .log_scroller
            .as_mut()
            .unwrap()
            .event(self.ctx.input)
        {
            self.wizard.confirmed_state.push(Box::new(()));
            self.wizard.log_scroller = None;
            true
        } else {
            false
        }
    }
}
