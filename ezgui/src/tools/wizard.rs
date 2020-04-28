use crate::{
    hotkey, Btn, Color, Composite, EventCtx, GfxCtx, HorizontalAlignment, InputResult, Key, Line,
    Menu, MultiKey, Outcome, Text, VerticalAlignment, Widget,
};
use abstutil::Cloneable;
use std::collections::VecDeque;

pub struct Wizard {
    alive: bool,
    tb_comp: Option<Composite>,
    menu_comp: Option<Composite>,
    ack: Option<Composite>,

    // In the order of queries made
    confirmed_state: Vec<Box<dyn Cloneable>>,
}

impl Wizard {
    pub fn new() -> Wizard {
        Wizard {
            alive: true,
            tb_comp: None,
            menu_comp: None,
            ack: None,
            confirmed_state: Vec::new(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let Some(ref comp) = self.menu_comp {
            comp.draw(g);
        }
        if let Some(ref comp) = self.tb_comp {
            comp.draw(g);
        }
        if let Some(ref s) = self.ack {
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
        if let Some(ref comp) = self.menu_comp {
            let item: &R = comp
                .menu::<Box<dyn Cloneable>>("menu")
                .current_choice()
                .as_any()
                .downcast_ref::<R>()?;
            return Some(item);
        }
        None
    }

    fn input_with_text_box<R: Cloneable>(
        &mut self,
        query: &str,
        prefilled: Option<String>,
        parser: Box<dyn Fn(String) -> Option<R>>,
        ctx: &mut EventCtx,
    ) -> Option<R> {
        assert!(self.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if ctx.input.has_been_consumed() {
            return None;
        }

        if self.tb_comp.is_none() {
            self.tb_comp = Some(
                Composite::new(
                    Widget::col(vec![
                        Widget::row(vec![
                            Line(query).small_heading().draw(ctx),
                            Btn::text_fg("X")
                                .build(ctx, "quit", hotkey(Key::Escape))
                                .margin(5)
                                .align_right(),
                        ]),
                        Widget::text_entry(ctx, prefilled.unwrap_or_else(String::new), true)
                            .named("input"),
                        Btn::text_bg2("Done").build(ctx, "done", hotkey(Key::Enter)),
                    ])
                    .bg(ctx.style().panel_bg)
                    .outline(5.0, Color::WHITE)
                    .padding(5),
                )
                .build(ctx),
            );
        }

        assert!(self.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if ctx.input.has_been_consumed() {
            return None;
        }

        match self.tb_comp.as_mut().unwrap().event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "quit" => {
                    self.alive = false;
                    self.tb_comp = None;
                    return None;
                }
                "done" => {
                    let line = self.tb_comp.take().unwrap().text_box("input");
                    if let Some(result) = parser(line.clone()) {
                        Some(result)
                    } else {
                        println!("Invalid input {}", line);
                        None
                    }
                }
                _ => unreachable!(),
            },
            None => None,
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
        if let Some(obj) = self
            .wizard
            .input_with_text_box(query, prefilled, parser, self.ctx)
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

    pub fn choose_exact<R: 'static + Clone + Cloneable, F: FnOnce() -> Vec<Choice<R>>>(
        &mut self,
        (horiz, vert): (HorizontalAlignment, VerticalAlignment),
        query: Option<&str>,
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
        // wizard
        if self.wizard.ack.is_some() {
            match self.wizard.ack.as_mut().unwrap().event(self.ctx) {
                Some(Outcome::Clicked(x)) => match x.as_ref() {
                    "OK" => {
                        self.wizard.ack = None;
                        self.wizard.alive = false;
                    }
                    _ => unreachable!(),
                },
                None => {
                    return None;
                }
            }
        }

        if self.wizard.menu_comp.is_none() {
            let choices: Vec<Choice<R>> = choices_generator();
            if choices.is_empty() {
                let mut txt = if let Some(l) = query {
                    Text::from(Line(l).small_heading())
                } else {
                    Text::new()
                };
                txt.add(Line("No choices, never mind"));
                self.setup_ack(txt);
                return None;
            }
            let mut col = Vec::new();
            if let Some(l) = query {
                col.push(Line(l).small_heading().draw(self.ctx));
            }
            col.push(
                Menu::new(
                    self.ctx,
                    choices
                        .into_iter()
                        .map(|c| c.with_value(c.data.clone_box()))
                        .collect(),
                )
                .named("menu"),
            );
            self.wizard.menu_comp = Some(
                Composite::new(
                    Widget::row(vec![
                        Widget::col(col),
                        Btn::text_fg("X")
                            .build(self.ctx, "quit", hotkey(Key::Escape))
                            .margin(5),
                    ])
                    .bg(self.ctx.style().panel_bg)
                    .outline(5.0, Color::WHITE)
                    .padding(5),
                )
                .aligned(horiz, vert)
                .build(self.ctx),
            );
        }

        assert!(self.wizard.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if self.ctx.input.has_been_consumed() {
            return None;
        }

        match self.wizard.menu_comp.as_mut().unwrap().event(self.ctx) {
            Some(Outcome::Clicked(x)) if x == "quit" => {
                self.wizard.alive = false;
                self.wizard.menu_comp = None;
                return None;
            }
            _ => {}
        }

        let (result, destroy) = match self
            .wizard
            .menu_comp
            .as_ref()
            .unwrap()
            .menu::<Box<dyn Cloneable>>("menu")
            .state
        {
            InputResult::Canceled => {
                self.wizard.alive = false;
                (None, true)
            }
            InputResult::StillActive => (None, false),
            InputResult::Done(ref choice, ref item) => {
                self.wizard
                    .confirmed_state
                    .push(Box::new((choice.to_string(), item.clone())));
                let downcasted_item: &R = item.as_any().downcast_ref::<R>().unwrap();
                (Some((choice.to_string(), downcasted_item.clone())), true)
            }
        };
        if destroy {
            self.wizard.menu_comp = None;
        }
        result
    }

    pub fn choose<R: 'static + Clone + Cloneable, F: FnOnce() -> Vec<Choice<R>>>(
        &mut self,
        query: &str,
        choices_generator: F,
    ) -> Option<(String, R)> {
        self.choose_exact(
            (HorizontalAlignment::Center, VerticalAlignment::Center),
            Some(query),
            choices_generator,
        )
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

        if self.wizard.ack.is_none() {
            let mut txt = Text::from(Line(title).small_heading());
            for l in make_lines() {
                txt.add(Line(l));
            }
            self.setup_ack(txt);
        }
        match self.wizard.ack.as_mut().unwrap().event(self.ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "OK" => {
                    self.wizard.confirmed_state.push(Box::new(()));
                    self.wizard.ack = None;
                    Some(())
                }
                _ => unreachable!(),
            },
            None => None,
        }
    }

    fn setup_ack(&mut self, txt: Text) {
        assert!(self.wizard.ack.is_none());
        self.wizard.ack = Some(
            Composite::new(
                Widget::col(vec![
                    txt.draw(self.ctx),
                    Btn::text_bg2("OK")
                        .build(self.ctx, "OK", hotkey(Key::Enter))
                        .margin(5),
                ])
                .bg(self.ctx.style().panel_bg)
                .outline(10.0, Color::WHITE)
                .padding(10),
            )
            .build(self.ctx),
        );
    }

    // If the control flow through a wizard block needs to change, might need to call this.
    pub fn reset(&mut self) {
        assert!(self.wizard.tb_comp.is_none());
        assert!(self.wizard.menu_comp.is_none());
        assert!(self.wizard.ack.is_none());
        self.wizard.confirmed_state.clear();
    }
}

pub struct Choice<T> {
    pub label: String,
    pub data: T,
    pub(crate) hotkey: Option<MultiKey>,
    pub(crate) active: bool,
    pub(crate) tooltip: Option<String>,
}

impl<T> Choice<T> {
    pub fn new<S: Into<String>>(label: S, data: T) -> Choice<T> {
        Choice {
            label: label.into(),
            data,
            hotkey: None,
            active: true,
            tooltip: None,
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
        self.hotkey = hotkey(key);
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

    pub fn tooltip<I: Into<String>>(mut self, info: I) -> Choice<T> {
        self.tooltip = Some(info.into());
        self
    }

    pub(crate) fn with_value<X>(&self, data: X) -> Choice<X> {
        Choice {
            label: self.label.clone(),
            data,
            hotkey: self.hotkey.clone(),
            active: self.active,
            tooltip: self.tooltip.clone(),
        }
    }
}

impl Choice<String> {
    pub fn string(label: &str) -> Choice<String> {
        Choice::new(label.to_string(), label.to_string())
    }
}
