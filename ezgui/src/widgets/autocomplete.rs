use crate::{
    Choice, EventCtx, GfxCtx, InputResult, Menu, ScreenDims, ScreenPt, TextBox, Widget, WidgetImpl,
    WidgetOutput,
};
use simsearch::SimSearch;
use std::collections::HashMap;

const NUM_SEARCH_RESULTS: usize = 10;

// TODO I don't even think we need to declare Clone...
pub struct Autocomplete<T: Clone> {
    choices: HashMap<String, Vec<T>>,
    // Maps index to choice
    search_map: Vec<String>,
    search: SimSearch<usize>,

    tb: TextBox,
    menu: Menu<()>,

    current_line: String,
    chosen_values: Option<Vec<T>>,
}

impl<T: 'static + Clone> Autocomplete<T> {
    // If multiple names map to the same data, all of the possible values will be returned
    pub fn new(ctx: &mut EventCtx, raw_choices: Vec<(String, T)>) -> Widget {
        let mut choices = HashMap::new();
        for (name, data) in raw_choices {
            choices.entry(name).or_insert_with(Vec::new).push(data);
        }

        let mut search_map = Vec::new();
        let mut search = SimSearch::new();
        for name in choices.keys() {
            search.insert(search_map.len(), name);
            search_map.push(name.to_string());
        }

        let mut a = Autocomplete {
            choices,
            search_map,
            search,

            tb: TextBox::new(ctx, 50, String::new(), true),
            menu: Menu::<()>::new(ctx, Vec::new()).take_menu(),

            current_line: String::new(),
            chosen_values: None,
        };
        a.recalc_menu(ctx);
        Widget::new(Box::new(a))
    }

    pub fn final_value(&self) -> Option<Vec<T>> {
        self.chosen_values.clone()
    }

    fn recalc_menu(&mut self, ctx: &mut EventCtx) {
        let mut indices = self.search.search(&self.current_line);
        if indices.is_empty() {
            indices = (0..NUM_SEARCH_RESULTS.min(self.search_map.len())).collect();
        }
        let mut choices = indices
            .into_iter()
            .take(NUM_SEARCH_RESULTS)
            .map(|idx| Choice::new(&self.search_map[idx], ()))
            .collect::<Vec<_>>();
        choices.insert(
            0,
            Choice::new(format!("anything matching \"{}\"", self.current_line), ()),
        );
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
                InputResult::Done(ref name, _) => {
                    // Mutating choices is fine, because we're supposed to be consumed by the
                    // caller immediately after this.
                    if name.starts_with("anything matching") {
                        let mut matches = Vec::new();
                        for (name, choices) in self.choices.drain() {
                            if name
                                .to_ascii_lowercase()
                                .contains(&self.current_line.to_ascii_lowercase())
                            {
                                matches.extend(choices);
                            }
                        }
                        self.chosen_values = Some(matches);
                    } else {
                        self.chosen_values = Some(self.choices.remove(name).unwrap());
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
