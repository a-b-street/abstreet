use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::{
    Button, Color, Composite, EventCtx, GfxCtx, Line, ManagedWidget, MultiKey, Outcome,
    RewriteColor, Text,
};
use std::collections::HashMap;

pub type Callback = Box<dyn Fn(&mut EventCtx, &mut UI) -> Option<Transition>>;

pub enum WrappedOutcome {
    Transition(Transition),
    Clicked(String),
}

pub struct WrappedComposite {
    pub inner: Composite,
    callbacks: HashMap<String, Callback>,
}

impl WrappedComposite {
    pub fn new(inner: Composite) -> WrappedComposite {
        WrappedComposite {
            inner,
            callbacks: HashMap::new(),
        }
    }

    pub fn cb(mut self, action: &str, cb: Callback) -> WrappedComposite {
        if !self.inner.get_all_click_actions().contains(action) {
            panic!("No button produces action {}", action);
        }

        self.callbacks.insert(action.to_string(), cb);
        self
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<WrappedOutcome> {
        match self.inner.event(ctx)? {
            Outcome::Clicked(x) => {
                if let Some(ref cb) = self.callbacks.get(&x) {
                    let t = (cb)(ctx, ui)?;
                    Some(WrappedOutcome::Transition(t))
                } else {
                    Some(WrappedOutcome::Clicked(x))
                }
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.inner.draw(g);
    }
}

impl WrappedComposite {
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

    pub fn nice_text_button(
        ctx: &EventCtx,
        txt: Text,
        hotkey: Option<MultiKey>,
        label: &str,
    ) -> ManagedWidget {
        ManagedWidget::btn(Button::text_no_bg(
            txt.clone(),
            txt.change_fg(Color::ORANGE),
            hotkey,
            label,
            ctx,
        ))
        .outline(2.0, Color::WHITE)
    }

    pub fn text_button(ctx: &EventCtx, label: &str, hotkey: Option<MultiKey>) -> ManagedWidget {
        WrappedComposite::detailed_text_button(
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
    composite: WrappedComposite,
    fullscreen: bool,
}

impl ManagedGUIState {
    pub fn fullscreen(composite: WrappedComposite) -> Box<dyn State> {
        Box::new(ManagedGUIState {
            composite,
            fullscreen: true,
        })
    }

    pub fn over_map(composite: WrappedComposite) -> Box<dyn State> {
        Box::new(ManagedGUIState {
            composite,
            fullscreen: false,
        })
    }
}

impl State for ManagedGUIState {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        match self.composite.event(ctx, ui) {
            Some(WrappedOutcome::Transition(t)) => t,
            Some(WrappedOutcome::Clicked(x)) => panic!(
                "Can't have a button {} without a callback in ManagedGUIState",
                x
            ),
            None => Transition::Keep,
        }
    }

    fn draw_default_ui(&self) -> bool {
        !self.fullscreen
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if self.fullscreen {
            // Happens to be a nice background color too ;)
            g.clear(ui.cs.get("grass"));
        }
        self.composite.draw(g);
        // Still want to show hotkeys
        CommonState::draw_osd(g, ui, &None);
    }
}
