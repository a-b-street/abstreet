use crate::app::App;
use crate::colors;
use crate::game::{DrawBaselayer, State, Transition};
use ezgui::{
    hotkey, Button, Color, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    ManagedWidget, MultiKey, Outcome, RewriteColor, Text, VerticalAlignment,
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
            RewriteColor::Change(Color::WHITE, colors::HOVERING),
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
            txt.change_fg(colors::HOVERING),
            hotkey,
            label,
            true,
            ctx,
        ))
        .outline(2.0, Color::WHITE)
    }

    pub fn text_button(ctx: &EventCtx, label: &str, hotkey: Option<MultiKey>) -> ManagedWidget {
        WrappedComposite::nice_text_button(ctx, Text::from(Line(label)), hotkey, label)
    }

    pub fn text_bg_button(ctx: &EventCtx, label: &str, hotkey: Option<MultiKey>) -> ManagedWidget {
        ManagedWidget::btn(Button::text_bg(
            Text::from(Line(label).fg(Color::BLACK)),
            Color::WHITE,
            colors::HOVERING,
            hotkey,
            label,
            ctx,
        ))
    }

    // Always includes a built-in "X" quit option
    pub fn quick_menu<I: Into<String>>(
        ctx: &mut EventCtx,
        title: I,
        info: Vec<String>,
        actions: Vec<(Option<MultiKey>, &str)>,
    ) -> Composite {
        Composite::new(
            ManagedWidget::col(vec![
                ManagedWidget::row(vec![
                    ManagedWidget::draw_text(ctx, Text::from(Line(title.into()).roboto_bold())),
                    WrappedComposite::text_button(ctx, "X", hotkey(Key::Escape)).align_right(),
                ]),
                ManagedWidget::draw_text(ctx, {
                    let mut txt = Text::new();
                    for l in info {
                        txt.add(Line(l));
                    }
                    txt
                }),
                ManagedWidget::row(
                    actions
                        .into_iter()
                        .map(|(key, action)| WrappedComposite::text_button(ctx, action, key))
                        .collect(),
                )
                .flex_wrap(ctx, 60),
            ])
            .padding(10)
            .bg(colors::PANEL_BG),
        )
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx)
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
        if !self.fullscreen && self.composite.inner.clicked_outside(ctx) {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        if self.fullscreen {
            DrawBaselayer::Custom
        } else {
            DrawBaselayer::PreviousState
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if self.fullscreen {
            // Happens to be a nice background color too ;)
            g.clear(app.cs.get("grass"));
        } else {
            State::grey_out_map(g);
        }

        self.composite.draw(g);
    }
}
