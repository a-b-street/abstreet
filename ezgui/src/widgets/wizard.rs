use crate::widgets::log_scroller::LogScroller;
use crate::widgets::text_box::TextBox;
use crate::widgets::PopupMenu;
use crate::{
    layout, Canvas, EventCtx, GfxCtx, InputResult, Key, MultiKey, SliderWithTextBox, Text,
    UserInput,
};
use abstutil::Cloneable;
use geom::Duration;
use std::collections::VecDeque;

pub struct Wizard {
    alive: bool,
    tb: Option<TextBox>,
    menu: Option<PopupMenu<Box<dyn Cloneable>>>,
    log_scroller: Option<LogScroller>,
    slider: Option<SliderWithTextBox>,

    // In the order of queries made
    confirmed_state: Vec<Box<dyn Cloneable>>,
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
            let item: &R = menu.current_choice().as_any().downcast_ref::<R>()?;
            return Some(item);
        }
        None
    }

    fn input_with_text_box<R: Cloneable>(
        &mut self,
        query: &str,
        prefilled: Option<String>,
        input: &mut UserInput,
        parser: Box<dyn Fn(String) -> Option<R>>,
        canvas: &Canvas,
    ) -> Option<R> {
        assert!(self.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if input.has_been_consumed() {
            return None;
        }

        if self.tb.is_none() {
            self.tb = Some(TextBox::new(query, prefilled, canvas));
        }
        layout::stack_vertically(
            layout::ContainerOrientation::Centered,
            canvas,
            vec![self.tb.as_mut().unwrap()],
        );

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
    ready_results: VecDeque<Box<dyn Cloneable>>,
}

impl<'a, 'b> WrappedWizard<'a, 'b> {
    pub fn input_something<R: 'static + Clone + Cloneable>(
        &mut self,
        query: &str,
        prefilled: Option<String>,
        parser: Box<dyn Fn(String) -> Option<R>>,
    ) -> Option<R> {
        if !self.ready_results.is_empty() {
            let first = self.ready_results.pop_front().unwrap();
            let item: &R = first.as_any().downcast_ref::<R>().unwrap();
            return Some(item.clone());
        }
        if let Some(obj) = self.wizard.input_with_text_box(
            query,
            prefilled,
            self.ctx.input,
            parser,
            self.ctx.canvas,
        ) {
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

    pub fn choose<R: 'static + Clone + Cloneable, F: FnOnce() -> Vec<Choice<R>>>(
        &mut self,
        query: &str,
        choices_generator: F,
    ) -> Option<(String, R)> {
        if !self.ready_results.is_empty() {
            let first = self.ready_results.pop_front().unwrap();
            // We have to downcast twice! \o/
            let pair: &(String, Box<dyn Cloneable>) = first
                .as_any()
                .downcast_ref::<(String, Box<dyn Cloneable>)>()
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
            let choices: Vec<Choice<R>> = choices_generator();
            if choices.is_empty() {
                self.wizard.log_scroller = Some(LogScroller::new(
                    "Wizard".to_string(),
                    vec![format!("No choices for \"{}\", canceling wizard", query)],
                ));
                return None;
            }
            self.wizard.menu = Some(PopupMenu::new(
                Text::prompt(query),
                choices
                    .into_iter()
                    .map(|c| Choice {
                        label: c.label,
                        data: c.data.clone_box(),
                        hotkey: c.hotkey,
                        active: c.active,
                    })
                    .collect(),
                self.ctx,
                false,
            ));
        }

        assert!(self.wizard.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if self.ctx.input.has_been_consumed() {
            return None;
        }

        match self.wizard.menu.as_mut().unwrap().event(self.ctx) {
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

    pub fn choose_string<S: Into<String>, F: Fn() -> Vec<S>>(
        &mut self,
        query: &str,
        choices_generator: F,
    ) -> Option<String> {
        self.choose(query, || {
            choices_generator()
                .into_iter()
                .map(|s| Choice::new(s, ()))
                .collect()
        })
        .map(|(s, _)| s)
    }

    pub fn aborted(&self) -> bool {
        self.wizard.aborted()
    }

    pub fn abort(&mut self) {
        self.wizard.alive = false;
    }

    pub fn acknowledge<S: Into<String>, F: Fn() -> Vec<S>>(
        &mut self,
        title: &str,
        make_lines: F,
    ) -> Option<()> {
        if !self.ready_results.is_empty() {
            self.ready_results.pop_front();
            return Some(());
        }

        if self.wizard.log_scroller.is_none() {
            self.wizard.log_scroller = Some(LogScroller::new(
                title.to_string(),
                make_lines().into_iter().map(|l| l.into()).collect(),
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
            Some(())
        } else {
            None
        }
    }

    // If the control flow through a wizard block needs to change, might need to call this.
    pub fn reset(&mut self) {
        assert!(self.wizard.tb.is_none());
        assert!(self.wizard.menu.is_none());
        assert!(self.wizard.log_scroller.is_none());
        assert!(self.wizard.slider.is_none());
        self.wizard.confirmed_state.clear();
    }
}

pub struct Choice<T: Clone> {
    pub(crate) label: String,
    pub data: T,
    pub(crate) hotkey: Option<MultiKey>,
    pub(crate) active: bool,
}

impl<T: Clone> Choice<T> {
    pub fn new<S: Into<String>>(label: S, data: T) -> Choice<T> {
        Choice {
            label: label.into(),
            data,
            hotkey: None,
            active: true,
        }
    }

    pub fn from(tuples: Vec<(String, T)>) -> Vec<Choice<T>> {
        tuples
            .into_iter()
            .map(|(label, data)| Choice::new(label, data))
            .collect()
    }

    pub fn key(mut self, key: Key) -> Choice<T> {
        assert_eq!(self.hotkey, None);
        self.hotkey = Some(MultiKey { key, lctrl: false });
        self
    }

    pub fn multikey(mut self, mk: Option<MultiKey>) -> Choice<T> {
        self.hotkey = mk;
        self
    }

    pub fn active(mut self, active: bool) -> Choice<T> {
        self.active = active;
        self
    }
}
