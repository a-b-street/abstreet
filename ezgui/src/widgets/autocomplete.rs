use crate::{
    Choice, EventCtx, GfxCtx, InputResult, Menu, ScreenDims, ScreenPt, TextBox, Widget, WidgetImpl,
    WidgetOutput,
};
use abstutil::MultiMap;

const NUM_SEARCH_RESULTS: usize = 10;

// TODO I don't even think we need to declare Clone...
// If multiple names map to the same data, all of the possible values will be returned
pub struct Autocomplete<T: Clone> {
    choices: Vec<(String, Vec<T>)>,

    tb: TextBox,
    menu: Menu<()>,

    current_line: String,
    chosen_values: Option<Vec<T>>,
}

impl<T: 'static + Clone + Ord> Autocomplete<T> {
    pub fn new(ctx: &mut EventCtx, raw_choices: Vec<(String, T)>) -> Widget {
        let mut grouped: MultiMap<String, T> = MultiMap::new();
        for (name, data) in raw_choices {
            grouped.insert(name, data);
        }
        let choices: Vec<(String, Vec<T>)> = grouped
            .consume()
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect();

        let mut a = Autocomplete {
            choices,

            tb: TextBox::new(ctx, 50, String::new(), true),
            menu: Menu::<()>::new(ctx, Vec::new()).take_menu(),

            current_line: String::new(),
            chosen_values: None,
        };
        a.recalc_menu(ctx);
        Widget::new(Box::new(a))
    }
}

impl<T: 'static + Clone> Autocomplete<T> {
    pub fn final_value(&self) -> Option<Vec<T>> {
        self.chosen_values.clone()
    }

    fn recalc_menu(&mut self, ctx: &mut EventCtx) {
        let mut choices = vec![Choice::new(
            format!("anything matching \"{}\"", self.current_line),
            (),
        )];
        let query = self.current_line.to_ascii_lowercase();
        for (name, _) in &self.choices {
            if name.to_ascii_lowercase().contains(&query) {
                choices.push(Choice::new(name, ()));
            }
            if choices.len() == NUM_SEARCH_RESULTS {
                break;
            }
        }
        // "anything matching" is silly if we've resolved to exactly one choice
        if choices.len() == 2 {
            choices.remove(0);
        }
        self.menu = Menu::new(ctx, choices).take_menu();
    }
}

impl<T: 'static + Clone> WidgetImpl for Autocomplete<T> {
    fn get_dims(&self) -> ScreenDims {
        let d1 = self.tb.get_dims();
        let d2 = self.menu.get_dims();
        ScreenDims::new(d1.width.max(d2.width), d1.height + d2.height)
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.tb.set_pos(top_left);
        self.menu.set_pos(ScreenPt::new(
            top_left.x,
            top_left.y + self.tb.get_dims().height,
        ));
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        assert!(self.chosen_values.is_none());

        self.tb.event(ctx, output);
        if self.tb.get_line() != self.current_line {
            self.current_line = self.tb.get_line();
            self.recalc_menu(ctx);
            output.redo_layout = true;
        } else {
            self.menu.event(ctx, output);
            match self.menu.state {
                InputResult::StillActive => {}
                // Ignore this and make sure the Composite has a quit control
                InputResult::Canceled => {
                    self.menu.state = InputResult::StillActive;
                }
                InputResult::Done(ref choice, _) => {
                    // Mutating choices is fine, because we're supposed to be consumed by the
                    // caller immediately after this.
                    if choice.starts_with("anything matching") {
                        let query = self.current_line.to_ascii_lowercase();
                        let mut matches = Vec::new();
                        for (name, choices) in self.choices.drain(..) {
                            if name.to_ascii_lowercase().contains(&query) {
                                matches.extend(choices);
                            }
                        }
                        self.chosen_values = Some(matches);
                    } else {
                        self.chosen_values = Some(
                            self.choices
                                .drain(..)
                                .find(|(name, _)| name == choice)
                                .unwrap()
                                .1,
                        );
                    }
                }
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.tb.draw(g);
        self.menu.draw(g);
    }
}
