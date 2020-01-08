use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::{Button, Color, EventCtx, GfxCtx, Line, ManagedWidget, MultiKey, RewriteColor, Text};
use std::collections::HashMap;

pub type Callback = Box<dyn Fn(&mut EventCtx, &mut UI) -> Option<Transition>>;

pub enum Outcome {
    Transition(Transition),
    Clicked(String),
}

pub struct Composite {
    inner: ezgui::Composite,
    callbacks: HashMap<String, Callback>,
}

impl Composite {
    pub fn new(inner: ezgui::Composite) -> Composite {
        Composite {
            inner,
            callbacks: HashMap::new(),
        }
    }

    pub fn cb(mut self, action: &str, cb: Callback) -> Composite {
        if !self.inner.get_all_click_actions().contains(action) {
            panic!("No button produces action {}", action);
        }

        self.callbacks.insert(action.to_string(), cb);
        self
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Outcome> {
        match self.inner.event(ctx)? {
            ezgui::Outcome::Clicked(x) => {
                if let Some(ref cb) = self.callbacks.get(&x) {
                    let t = (cb)(ctx, ui)?;
                    Some(Outcome::Transition(t))
                } else {
                    Some(Outcome::Clicked(x))
                }
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.inner.draw(g);
    }
}

impl Composite {
    pub fn img_button(
        ctx: &EventCtx,
        filename: &str,
        hotkey: Option<MultiKey>,
        label: &str,
    ) -> ManagedWidget {
        ManagedWidget::btn(Button::rectangle_img(filename, hotkey, ctx, label))
    }

    pub fn svg_button(
        ctx: &EventCtx,
        filename: &str,
        tooltip: &str,
        hotkey: Option<MultiKey>,
    ) -> ManagedWidget {
        ManagedWidget::btn(Button::rectangle_svg(
            filename,
            tooltip,
            hotkey,
            RewriteColor::Change(Color::WHITE, Color::ORANGE),
            ctx,
        ))
    }

    pub fn text_button(ctx: &EventCtx, label: &str, hotkey: Option<MultiKey>) -> ManagedWidget {
        Composite::detailed_text_button(
            ctx,
            Text::from(Line(label).fg(Color::BLACK)),
            hotkey,
            label,
        )
    }

    pub fn detailed_text_button(
        ctx: &EventCtx,
        txt: Text,
        hotkey: Option<MultiKey>,
        label: &str,
    ) -> ManagedWidget {
        // TODO Default style. Lots of variations.
        ManagedWidget::btn(Button::text(
            txt,
            Color::WHITE,
            Color::ORANGE,
            hotkey,
            label,
            ctx,
        ))
    }
}

pub struct ManagedGUIState {
    composite: Composite,
}

impl ManagedGUIState {
    pub fn new(composite: Composite) -> Box<dyn State> {
        Box::new(ManagedGUIState { composite })
    }
}

impl State for ManagedGUIState {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        match self.composite.event(ctx, ui) {
            Some(Outcome::Transition(t)) => t,
            Some(Outcome::Clicked(x)) => panic!(
                "Can't have a button {} without a callback in ManagedGUIState",
                x
            ),
            None => Transition::Keep,
        }
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // Happens to be a nice background color too ;)
        g.clear(ui.cs.get("grass"));
        self.composite.draw(g);
    }
}
