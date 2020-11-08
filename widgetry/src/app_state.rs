//! A widgetry application splits its state into two pieces: global shared state that lasts for the
//! entire lifetime of the application, and a stack of smaller states, only one of which is active
//! at a time. For example, imagine an application to view a map. The shared state would include
//! the map and pre-rendered geometry for it. The individual states might start with a splash
//! screen or menu to choose a map, then a map viewer, then maybe a state to drill down into pieces
//! of the map.

use crate::{Canvas, Color, EventCtx, GfxCtx};

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
                match self.states[self.states.len() - 2].draw_baselayer() {
                    DrawBaselayer::DefaultDraw => {
                        self.shared_app_state.draw_default(g);
                    }
                    DrawBaselayer::Custom => {}
                    // Don't recurse, but at least clear the screen, because the state is usually
                    // expecting the previous thing to happen.
                    DrawBaselayer::PreviousState => {
                        g.clear(Color::BLACK);
                    }
                }

                self.states[self.states.len() - 2].draw(g, &self.shared_app_state);
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
