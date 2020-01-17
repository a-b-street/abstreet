use crate::common::CommonState;
use crate::options::Options;
use crate::pregame::TitleScreen;
use crate::render::DrawOptions;
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::ui::{Flags, ShowEverything, UI};
use ezgui::{
    Canvas, Color, EventCtx, EventLoopMode, GfxCtx, HorizontalAlignment, Line, Text,
    VerticalAlignment, Wizard, GUI,
};
use geom::Polygon;

// This is the top-level of the GUI logic. This module should just manage interactions between the
// top-level game states.
pub struct Game {
    // A stack of states
    states: Vec<Box<dyn State>>,
    ui: UI,
}

impl Game {
    pub fn new(flags: Flags, opts: Options, mode: GameplayMode, ctx: &mut EventCtx) -> Game {
        let title = !opts.dev
            && !flags.sim_flags.load.contains("data/player/save")
            && !flags.sim_flags.load.contains("data/system/scenarios")
            && mode == GameplayMode::Freeform;
        let mut ui = UI::new(flags, opts, ctx, title);
        let states: Vec<Box<dyn State>> = if title {
            vec![Box::new(TitleScreen::new(ctx, &ui))]
        } else {
            vec![Box::new(SandboxMode::new(ctx, &mut ui, mode))]
        };
        Game { states, ui }
    }
}

impl GUI for Game {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        self.ui.per_obj.reset();

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

        let ev_mode = match transition {
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
            Transition::PopTwiceWithData(cb) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
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
            Transition::ApplyObjectAction(action) => {
                self.ui.per_obj.action_chosen(action);
                return EventLoopMode::InputOnly;
            }
            Transition::PushTwice(s1, s2) => {
                self.states
                    .last_mut()
                    .unwrap()
                    .on_suspend(ctx, &mut self.ui);
                self.states.push(s1);
                self.states.push(s2);
                return EventLoopMode::InputOnly;
            }
            _ => unreachable!(),
        };
        self.ui.per_obj.assert_chosen_used();
        ev_mode
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

        if self.ui.opts.dev && !g.is_screencap() {
            let mut txt = Text::from(Line("DEV"));
            txt.highlight_last_line(Color::RED);
            g.draw_blocking_text(
                &txt,
                (HorizontalAlignment::Right, VerticalAlignment::Bottom),
            );
        }

        /*println!(
            "----- {} uploads, {} draw calls -----",
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
    PopTwiceWithData(Box<dyn FnOnce(&mut Box<dyn State>, &mut UI, &mut EventCtx)>),
    Push(Box<dyn State>),
    Replace(Box<dyn State>),
    PopThenReplace(Box<dyn State>),
    Clear(Box<dyn State>),
    ApplyObjectAction(String),
    PushTwice(Box<dyn State>, Box<dyn State>),

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
        // Make it clear the map can't be interacted with right now.
        g.fork_screenspace();
        // TODO - OSD height
        g.draw_polygon(
            Color::BLACK.alpha(0.5),
            &Polygon::rectangle(g.canvas.window_width, g.canvas.window_height),
        );
        g.unfork();

        self.wizard.draw(g);
        // Still want to show hotkeys
        CommonState::draw_osd(g, ui, &None);
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
