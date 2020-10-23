// TODO Lotsa docs needed

use crate::{Canvas, EventCtx, GfxCtx};

pub trait SharedAppState {
    fn before_event(&mut self) {}
    fn draw_default(&self, _: &mut GfxCtx) {}

    // Will be called if event or draw panics.
    fn dump_before_abort(&self, _: &Canvas) {}
    // Only before a normal exit, like window close
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
                    // Nope, don't recurse
                    DrawBaselayer::PreviousState => {}
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
                self.states
                    .pop()
                    .unwrap()
                    .on_destroy(ctx, &mut self.shared_app_state);
                if self.states.is_empty() {
                    self.shared_app_state.before_quit(ctx.canvas);
                    std::process::exit(0);
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

pub enum DrawBaselayer {
    DefaultDraw,
    Custom,
    PreviousState,
}

pub trait State<A>: downcast_rs::Downcast {
    fn event(&mut self, ctx: &mut EventCtx, shared_app_state: &mut A) -> Transition<A>;
    fn draw(&self, g: &mut GfxCtx, shared_app_state: &A);

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::DefaultDraw
    }

    /// Before this state is popped or replaced, call this.
    fn on_destroy(&mut self, _: &mut EventCtx, _: &mut A) {}
    // We don't need an on_enter -- the constructor for the state can just do it.
}

downcast_rs::impl_downcast!(State<A>);

pub enum Transition<A> {
    Keep,
    KeepWithMouseover,
    Pop,
    /// If a state needs to pass data back to the parent, use this. Sadly, runtime type casting.
    ModifyState(Box<dyn FnOnce(&mut Box<dyn State<A>>, &mut EventCtx, &mut A)>),
    // TODO This is like Replace + ModifyState, then returning a few Push's from the callback. Not
    // sure how to express it in terms of the others without complicating ModifyState everywhere.
    ReplaceWithData(
        Box<dyn FnOnce(Box<dyn State<A>>, &mut EventCtx, &mut A) -> Vec<Box<dyn State<A>>>>,
    ),
    Push(Box<dyn State<A>>),
    Replace(Box<dyn State<A>>),
    Clear(Vec<Box<dyn State<A>>>),
    Multi(Vec<Transition<A>>),
}
