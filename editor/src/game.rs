use crate::render::DrawOptions;
use crate::sandbox::SandboxMode;
use crate::splash_screen::SplashScreen;
use crate::ui::{Flags, ShowEverything, UI};
use ezgui::{Canvas, EventCtx, EventLoopMode, GfxCtx, Wizard, GUI};

// This is the top-level of the GUI logic. This module should just manage interactions between the
// top-level game states.
pub struct Game {
    // A stack of states
    pub states: Vec<Box<State>>,
    pub ui: UI,

    idx_draw_base: Option<usize>,
}

impl Game {
    pub fn new(flags: Flags, ctx: &mut EventCtx) -> Game {
        let splash = !flags.no_splash
            && !format!("{}", flags.sim_flags.load.display()).contains("data/save");
        let ui = UI::new(flags, ctx, splash);
        let states: Vec<Box<State>> = if splash {
            vec![Box::new(SplashScreen::new_with_screensaver(ctx, &ui))]
        } else {
            vec![
                Box::new(SplashScreen::new_without_screensaver()),
                Box::new(SandboxMode::new(ctx)),
            ]
        };
        let idx_draw_base = if states.last().as_ref().unwrap().draw_as_base_for_substates() {
            Some(states.len() - 1)
        } else {
            None
        };
        Game {
            states,
            ui,
            idx_draw_base,
        }
    }
}

impl GUI for Game {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        // First rewrite the transitions to explicitly have EventLoopMode, to avoid duplicated
        // code.
        let transition = match self.states.last_mut().unwrap().event(ctx, &mut self.ui) {
            Transition::Keep => Transition::KeepWithMode(EventLoopMode::InputOnly),
            Transition::Pop => Transition::PopWithMode(EventLoopMode::InputOnly),
            Transition::Push(state) => Transition::PushWithMode(state, EventLoopMode::InputOnly),
            Transition::Replace(state) => {
                Transition::ReplaceWithMode(state, EventLoopMode::InputOnly)
            }
            x => x,
        };

        match transition {
            Transition::KeepWithMode(evmode) => evmode,
            Transition::PopWithMode(evmode) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                if self.states.is_empty() {
                    self.before_quit(ctx.canvas);
                    std::process::exit(0);
                }
                if self.idx_draw_base == Some(self.states.len()) {
                    self.idx_draw_base = None;
                }
                evmode
            }
            Transition::PopWithData(cb) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                cb(self.states.last_mut().unwrap(), &mut self.ui, ctx);
                if self.idx_draw_base == Some(self.states.len()) {
                    self.idx_draw_base = None;
                }
                EventLoopMode::InputOnly
            }
            Transition::PushWithMode(state, evmode) => {
                self.states.last_mut().unwrap().on_suspend(&mut self.ui);
                if self.idx_draw_base.is_some() {
                    assert!(!state.draw_as_base_for_substates());
                    assert!(state.draw_default_ui());
                } else if state.draw_as_base_for_substates() {
                    assert!(!state.draw_default_ui());
                    self.idx_draw_base = Some(self.states.len());
                }
                self.states.push(state);
                evmode
            }
            Transition::ReplaceWithMode(state, evmode) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                if self.idx_draw_base == Some(self.states.len()) {
                    self.idx_draw_base = None;
                }

                if self.idx_draw_base.is_some() {
                    assert!(!state.draw_as_base_for_substates());
                    assert!(state.draw_default_ui());
                } else if state.draw_as_base_for_substates() {
                    assert!(!state.draw_default_ui());
                    self.idx_draw_base = Some(self.states.len());
                }
                self.states.push(state);
                evmode
            }
            _ => unreachable!(),
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        let state = self.states.last().unwrap();

        if let Some(idx) = self.idx_draw_base {
            self.states[idx].draw(g, &self.ui);
            if idx != self.states.len() - 1 {
                state.draw(g, &self.ui);
            }
        } else if state.draw_default_ui() {
            self.ui.draw(
                g,
                DrawOptions::new(),
                &self.ui.primary.sim,
                &ShowEverything::new(),
            );
            state.draw(g, &self.ui);
        } else {
            state.draw(g, &self.ui);
        }

        /*println!(
            "{} uploads, {} draw calls",
            g.get_num_uploads(),
            g.num_draw_calls
        );*/
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        println!(
            "********************************************************************************"
        );
        println!("UI broke! Primary sim:");
        self.ui.primary.sim.dump_before_abort();
        if let Some(ref s) = self.ui.secondary {
            println!("Secondary sim:");
            s.sim.dump_before_abort();
        }
        self.ui.save_editor_state(canvas);
    }

    fn before_quit(&self, canvas: &Canvas) {
        self.ui.save_editor_state(canvas);
        self.ui.cs.save();
        println!("Saved data/color_scheme.json");
    }

    fn profiling_enabled(&self) -> bool {
        self.ui.primary.current_flags.enable_profiler
    }
}

pub trait State: downcast_rs::Downcast {
    // Logically this returns Transition, but since EventLoopMode is almost always
    // InputOnly, the variations are encoded by Transition.
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition;
    fn draw(&self, g: &mut GfxCtx, ui: &UI);

    fn draw_default_ui(&self) -> bool {
        true
    }
    fn draw_as_base_for_substates(&self) -> bool {
        false
    }

    // Before we push a new state on top of this one, call this.
    fn on_suspend(&mut self, _: &mut UI) {}
    // Before this state is popped or replaced, call this.
    fn on_destroy(&mut self, _: &mut EventCtx, _: &mut UI) {}
    // We don't need an on_enter -- the constructor for the state can just do it.
}

downcast_rs::impl_downcast!(State);

pub enum Transition {
    // These variants imply EventLoopMode::InputOnly.
    Keep,
    Pop,
    // If a state needs to pass data back to the parent, use this. Sadly, runtime type casting.
    PopWithData(Box<FnOnce(&mut Box<State>, &mut UI, &mut EventCtx)>),
    Push(Box<State>),
    Replace(Box<State>),

    // These don't.
    KeepWithMode(EventLoopMode),
    PopWithMode(EventLoopMode),
    PushWithMode(Box<State>, EventLoopMode),
    ReplaceWithMode(Box<State>, EventLoopMode),
}

pub struct WizardState {
    wizard: Wizard,
    // Returning None means stay in this WizardState
    cb: Box<Fn(&mut Wizard, &mut EventCtx, &mut UI) -> Option<Transition>>,
}

impl WizardState {
    pub fn new(
        cb: Box<Fn(&mut Wizard, &mut EventCtx, &mut UI) -> Option<Transition>>,
    ) -> Box<State> {
        Box::new(WizardState {
            wizard: Wizard::new(),
            cb,
        })
    }
}

impl State for WizardState {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        ctx.canvas.handle_event(ctx.input);
        if let Some(t) = (self.cb)(&mut self.wizard, ctx, ui) {
            return t;
        } else if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}
