use crate::app::App;
use crate::game::{DrawBaselayer, State, Transition};
use ezgui::{
    hotkey, Btn, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, MultiKey, Outcome,
    Text, VerticalAlignment, Widget,
};
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

impl WrappedComposite {
    // Always includes a built-in "X" quit option
    pub fn quick_menu<I: Into<String>>(
        ctx: &mut EventCtx,
        app: &App,
        title: I,
        info: Vec<String>,
        actions: Vec<(Option<MultiKey>, &str)>,
    ) -> Composite {
        Composite::new(
            Widget::col(vec![
                Widget::row(vec![
                    Line(title.into()).small_heading().draw(ctx),
                    Btn::text_fg("X")
                        .build_def(ctx, hotkey(Key::Escape))
                        .align_right(),
                ]),
                {
                    let mut txt = Text::new();
                    for l in info {
                        txt.add(Line(l));
                    }
                    txt.draw(ctx)
                },
                Widget::row(
                    actions
                        .into_iter()
                        .map(|(key, action)| Btn::text_fg(action).build_def(ctx, key))
                        .collect(),
                )
                .flex_wrap(ctx, 60),
            ])
            .padding(10)
            .bg(app.cs.panel_bg),
        )
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx)
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
