use crate::app::App;
use crate::game::{DrawBaselayer, State, Transition};
use ezgui::{Composite, EventCtx, GfxCtx, Outcome};
use std::collections::HashMap;

pub type Callback = Box<dyn Fn(&mut EventCtx, &mut App) -> Option<Transition>>;

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

    pub fn cb(self, action: &str, cb: Callback) -> WrappedComposite {
        if !self.inner.get_all_click_actions().contains(action) {
            panic!("No button produces action {}", action);
        }
        self.maybe_cb(action, cb)
    }

    pub fn maybe_cb(mut self, action: &str, cb: Callback) -> WrappedComposite {
        self.callbacks.insert(action.to_string(), cb);
        self
    }

    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<WrappedOutcome> {
        match self.inner.event(ctx)? {
            Outcome::Clicked(x) => {
                if let Some(ref cb) = self.callbacks.get(&x) {
                    let t = (cb)(ctx, app)?;
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

pub struct ManagedGUIState {
    composite: WrappedComposite,
}

impl ManagedGUIState {
    pub fn fullscreen(composite: WrappedComposite) -> Box<dyn State> {
        Box::new(ManagedGUIState { composite })
    }
}

impl State for ManagedGUIState {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => {
                return t;
            }
            Some(WrappedOutcome::Clicked(x)) => panic!(
                "Can't have a button {} without a callback in ManagedGUIState",
                x
            ),
            None => {}
        }
        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // Happens to be a nice background color too ;)
        g.clear(app.cs.grass);
        self.composite.draw(g);
    }
}
