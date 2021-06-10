//! A widgetry application splits its state into two pieces: global shared state that lasts for the
//! entire lifetime of the application, and a stack of smaller states, only one of which is active
//! at a time. For example, imagine an application to view a map. The shared state would include
//! the map and pre-rendered geometry for it. The individual states might start with a splash
//! screen or menu to choose a map, then a map viewer, then maybe a state to drill down into pieces
//! of the map.

use crate::{Canvas, Color, EventCtx, GfxCtx, Outcome, Panel};

/// Any data that should last the entire lifetime of the application should be stored in the struct
/// implementing this trait.
pub trait SharedAppState {
    /// Before `State::event` is called, call this.
    fn before_event(&mut self) {}
    /// When DrawBaselayer::DefaultDraw is called, run this.
    fn draw_default(&self, _: &mut GfxCtx) {}

    /// Will be called if `State::event` or `State::draw` panics.
    fn dump_before_abort(&self, _: &Canvas) {}
    /// Called before a normal exit, like window close
    fn before_quit(&self, _: &Canvas) {}

    /// If widgetry determines the video card is low on memory, this may be called. The application
    /// should make its best effort to delete any unused Drawables.
    fn free_memory(&mut self) {}
}

pub(crate) struct App<A: SharedAppState> {
    /// A stack of states
    pub(crate) states: Vec<Box<dyn State<A>>>,
    pub(crate) shared_app_state: A,
}

impl<A: SharedAppState> App<A> {
    pub(crate) fn event(&mut self, ctx: &mut EventCtx) {
        self.shared_app_state.before_event();

        let transition = self
            .states
            .last_mut()
            .unwrap()
            .event(ctx, &mut self.shared_app_state);
        if self.execute_transition(ctx, transition) {
            // Let the new state initialize with a fake event. Usually these just return
            // Transition::Keep, but nothing stops them from doing whatever. (For example, entering
            // tutorial mode immediately pushes on a Warper.) So just recurse.
            ctx.no_op_event(true, |ctx| self.event(ctx));
        }
    }

    pub(crate) fn draw(&self, g: &mut GfxCtx) {
        let state = self.states.last().unwrap();

        match state.draw_baselayer() {
            DrawBaselayer::DefaultDraw => {
                self.shared_app_state.draw_default(g);
            }
            DrawBaselayer::Custom => {}
            DrawBaselayer::PreviousState => {
                if self.states.len() >= 2 {
                    match self.states[self.states.len() - 2].draw_baselayer() {
                        DrawBaselayer::DefaultDraw => {
                            self.shared_app_state.draw_default(g);
                        }
                        DrawBaselayer::Custom => {}
                        // Don't recurse, but at least clear the screen, because the state is
                        // usually expecting the previous thing to happen.
                        DrawBaselayer::PreviousState => {
                            g.clear(Color::BLACK);
                        }
                    }

                    self.states[self.states.len() - 2].draw(g, &self.shared_app_state);
                } else {
                    // I'm not entirely sure why this happens, but crashing isn't ideal.
                    warn!(
                        "A state requested DrawBaselayer::PreviousState, but it's the only state \
                         on the stack!"
                    );
                    g.clear(Color::BLACK);
                }
            }
        }
        state.draw(g, &self.shared_app_state);
    }

    /// If true, then the top-most state on the stack needs to be "woken up" with a fake mouseover
    /// event.
    fn execute_transition(&mut self, ctx: &mut EventCtx, transition: Transition<A>) -> bool {
        match transition {
            Transition::Keep => false,
            Transition::KeepWithMouseover => true,
            Transition::Pop => {
                let mut state = self.states.pop().unwrap();
                state.on_destroy(ctx, &mut self.shared_app_state);
                if self.states.is_empty() {
                    if cfg!(target_arch = "wasm32") {
                        // Just kidding, don't actually leave.
                        self.states.push(state);
                    // TODO Once PopupMsg is lifted here, add an explanation
                    } else {
                        self.shared_app_state.before_quit(ctx.canvas);
                        std::process::exit(0);
                    }
                }
                true
            }
            Transition::ModifyState(cb) => {
                cb(
                    self.states.last_mut().unwrap(),
                    ctx,
                    &mut self.shared_app_state,
                );
                true
            }
            Transition::ReplaceWithData(cb) => {
                let mut last = self.states.pop().unwrap();
                last.on_destroy(ctx, &mut self.shared_app_state);
                let new_states = cb(last, ctx, &mut self.shared_app_state);
                self.states.extend(new_states);
                true
            }
            Transition::Push(state) => {
                self.states.push(state);
                true
            }
            Transition::Replace(state) => {
                self.states
                    .pop()
                    .unwrap()
                    .on_destroy(ctx, &mut self.shared_app_state);
                self.states.push(state);
                true
            }
            Transition::Clear(states) => {
                while !self.states.is_empty() {
                    self.states
                        .pop()
                        .unwrap()
                        .on_destroy(ctx, &mut self.shared_app_state);
                }
                self.states.extend(states);
                true
            }
            Transition::Multi(list) => {
                // Always wake-up just the last state remaining after the sequence
                for t in list {
                    self.execute_transition(ctx, t);
                }
                true
            }
        }
    }
}

/// Before `State::draw` is called, draw something else.
pub enum DrawBaselayer {
    /// Call `SharedAppState::draw_default`.
    DefaultDraw,
    /// Don't draw anything.
    Custom,
    /// Call the previous state's `draw`. This won't recurse, even if that state specifies
    /// `PreviousState`.
    PreviousState,
}

/// A temporary state of an application. There's a stack of these, with the most recent being the
/// active one.
pub trait State<A>: downcast_rs::Downcast {
    /// Respond to a UI event, such as input or time passing.
    fn event(&mut self, ctx: &mut EventCtx, shared_app_state: &mut A) -> Transition<A>;
    /// Draw
    fn draw(&self, g: &mut GfxCtx, shared_app_state: &A);

    /// Specifies what to draw before draw()
    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::DefaultDraw
    }

    /// Before this state is popped or replaced, call this.
    fn on_destroy(&mut self, _: &mut EventCtx, _: &mut A) {}
    // We don't need an on_enter -- the constructor for the state can just do it.
}

downcast_rs::impl_downcast!(State<A>);

/// When a state responds to an event, it can specify some way to manipulate the stack of states.
pub enum Transition<A> {
    /// Don't do anything, keep the current state as the active one
    Keep,
    /// Keep the current state as the active one, but immediately call `event` again with a mouse
    /// moved event
    KeepWithMouseover,
    /// Destroy the current state, and resume from the previous one
    Pop,
    /// If a state needs to pass data back to its parent, use this. In the callback, you have to
    /// downcast the previous state to populate it with data.
    ModifyState(Box<dyn FnOnce(&mut Box<dyn State<A>>, &mut EventCtx, &mut A)>),
    // TODO This is like Replace + ModifyState, then returning a few Push's from the callback. Not
    // sure how to express it in terms of the others without complicating ModifyState everywhere.
    ReplaceWithData(
        Box<dyn FnOnce(Box<dyn State<A>>, &mut EventCtx, &mut A) -> Vec<Box<dyn State<A>>>>,
    ),
    /// Push a new active state on the top of the stack.
    Push(Box<dyn State<A>>),
    /// Replace the current state with a new one. Equivalent to Pop, then Push.
    Replace(Box<dyn State<A>>),
    /// Replace the entire stack of states with this stack.
    Clear(Vec<Box<dyn State<A>>>),
    /// Execute a sequence of transitions in order.
    Multi(Vec<Transition<A>>),
}

/// Many states fit a pattern of managing a single panel, handling mouseover events, and other
/// interactions on the map. Implementing this instead of `State` reduces some boilerplate.
pub trait SimpleState<A> {
    /// Called when something on the panel has been clicked. Since the action is just a string,
    /// the fallback case can just use `unreachable!()`.
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut A,
        action: &str,
        panel: &Panel,
    ) -> Transition<A>;
    /// Called when something on the panel has changed. If a transition is returned, stop handling
    /// the event and immediately apply the transition.
    fn panel_changed(
        &mut self,
        _: &mut EventCtx,
        _: &mut A,
        _: &mut Panel,
    ) -> Option<Transition<A>> {
        None
    }
    /// Called when the mouse has moved.
    fn on_mouseover(&mut self, _: &mut EventCtx, _: &mut A) {}
    /// If a panel `on_click` event didn't occur and `panel_changed` didn't return  transition, then
    /// call this to handle all other events.
    fn other_event(&mut self, _: &mut EventCtx, _: &mut A) -> Transition<A> {
        Transition::Keep
    }
    fn draw(&self, _: &mut GfxCtx, _: &A) {}
    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::DefaultDraw
    }
}

impl<A: 'static> dyn SimpleState<A> {
    pub fn new_state(panel: Panel, inner: Box<dyn SimpleState<A>>) -> Box<dyn State<A>> {
        Box::new(SimpleStateWrapper { panel, inner })
    }
}

pub struct SimpleStateWrapper<A> {
    panel: Panel,
    inner: Box<dyn SimpleState<A>>,
}

impl<A: 'static> State<A> for SimpleStateWrapper<A> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        if ctx.redo_mouseover() {
            self.inner.on_mouseover(ctx, app);
        }
        match self.panel.event(ctx) {
            Outcome::Clicked(action) => self.inner.on_click(ctx, app, &action, &self.panel),
            Outcome::Changed(_) => self
                .inner
                .panel_changed(ctx, app, &mut self.panel)
                .unwrap_or_else(|| self.inner.other_event(ctx, app)),
            Outcome::DragDropReordered(_, _, _) | Outcome::Nothing => {
                self.inner.other_event(ctx, app)
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        self.inner.draw(g, app);
        // Draw last
        self.panel.draw(g);
    }
    fn draw_baselayer(&self) -> DrawBaselayer {
        self.inner.draw_baselayer()
    }
}
