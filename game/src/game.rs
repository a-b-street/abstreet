use crate::pregame::TitleScreen;
use crate::render::DrawOptions;
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::ui::{Flags, ShowEverything, UI};
use ezgui::{
    Canvas, Color, EventCtx, EventLoopMode, GfxCtx, HorizontalAlignment, Line, Text,
    VerticalAlignment, Wizard, GUI,
};

// This is the top-level of the GUI logic. This module should just manage interactions between the
// top-level game states.
pub struct Game {
    // A stack of states
    pub states: Vec<Box<dyn State>>,
    pub ui: UI,
}

impl Game {
    pub fn new(flags: Flags, ctx: &mut EventCtx) -> Game {
        let title = !flags.dev
            && !flags.sim_flags.load.contains("data/save")
            && !flags.sim_flags.load.contains("data/scenarios");
        let mut ui = UI::new(flags, ctx, title);
        let states: Vec<Box<dyn State>> = if title {
            vec![Box::new(TitleScreen::new(ctx, &ui))]
        } else {
            vec![Box::new(SandboxMode::new(
                ctx,
                &mut ui,
                GameplayMode::Freeform,
            ))]
        };
        Game { states, ui }
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
            Transition::PopThenReplace(state) => {
                Transition::PopThenReplaceWithMode(state, EventLoopMode::InputOnly)
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
                evmode
            }
            Transition::PopWithData(cb) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                cb(self.states.last_mut().unwrap(), &mut self.ui, ctx);
                EventLoopMode::InputOnly
            }
            Transition::PushWithMode(state, evmode) => {
                self.states
                    .last_mut()
                    .unwrap()
                    .on_suspend(ctx, &mut self.ui);
                self.states.push(state);
                evmode
            }
            Transition::ReplaceWithMode(state, evmode) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                self.states.push(state);
                evmode
            }
            Transition::PopThenReplaceWithMode(state, evmode) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                assert!(!self.states.is_empty());
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                self.states.push(state);
                evmode
            }
            Transition::Clear(state) => {
                while !self.states.is_empty() {
                    self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                }
                self.states.push(state);
                EventLoopMode::InputOnly
            }
            _ => unreachable!(),
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        let state = self.states.last().unwrap();

        if state.draw_default_ui() {
            self.ui.draw(
                g,
                DrawOptions::new(),
                &self.ui.primary.sim,
                &ShowEverything::new(),
            );
        }
        state.draw(g, &self.ui);

        if self.ui.primary.current_flags.dev {
            g.draw_blocking_text(
                &Text::from(Line("DEV")).bg(Color::RED),
                (HorizontalAlignment::Right, VerticalAlignment::Bottom),
            );
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
        canvas.save_camera_state(self.ui.primary.map.get_name());
    }

    fn before_quit(&self, canvas: &Canvas) {
        canvas.save_camera_state(self.ui.primary.map.get_name());
        self.ui.cs.save();
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

    // Before we push a new state on top of this one, call this.
    fn on_suspend(&mut self, _: &mut EventCtx, _: &mut UI) {}
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
    PopWithData(Box<dyn FnOnce(&mut Box<dyn State>, &mut UI, &mut EventCtx)>),
    Push(Box<dyn State>),
    Replace(Box<dyn State>),
    PopThenReplace(Box<dyn State>),
    Clear(Box<dyn State>),

    // These don't.
    KeepWithMode(EventLoopMode),
    PopWithMode(EventLoopMode),
    PushWithMode(Box<dyn State>, EventLoopMode),
    ReplaceWithMode(Box<dyn State>, EventLoopMode),
    PopThenReplaceWithMode(Box<dyn State>, EventLoopMode),
}

pub struct WizardState {
    wizard: Wizard,
    // Returning None means stay in this WizardState
    cb: Box<dyn Fn(&mut Wizard, &mut EventCtx, &mut UI) -> Option<Transition>>,
    pub draw_opts: DrawOptions,
}

impl WizardState {
    pub fn new(
        cb: Box<dyn Fn(&mut Wizard, &mut EventCtx, &mut UI) -> Option<Transition>>,
    ) -> Box<dyn State> {
        Box::new(WizardState {
            wizard: Wizard::new(),
            cb,
            draw_opts: DrawOptions::new(),
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

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        ui.draw(
            g,
            self.draw_opts.clone(),
            &ui.primary.sim,
            &ShowEverything::new(),
        );
        self.wizard.draw(g);
    }
}

// TODO Word wrap
pub fn msg<S: Into<String>>(title: &'static str, lines: Vec<S>) -> Box<dyn State> {
    let str_lines: Vec<String> = lines.into_iter().map(|l| l.into()).collect();
    WizardState::new(Box::new(move |wiz, ctx, _| {
        wiz.wrap(ctx).acknowledge(title, || str_lines.clone())?;
        Some(Transition::Pop)
    }))
}
