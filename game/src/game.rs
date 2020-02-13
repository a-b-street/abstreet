use crate::common::CommonState;
use crate::options::Options;
use crate::pregame::TitleScreen;
use crate::render::DrawOptions;
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::ui::{Flags, ShowEverything, UI};
use abstutil::Timer;
use ezgui::{Canvas, Color, Drawable, EventCtx, EventLoopMode, GfxCtx, TextureType, Wizard, GUI};
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
        ctx.set_textures(
            vec![
                ("assets/pregame/challenges.png", TextureType::Stretch),
                ("assets/pregame/logo.png", TextureType::Stretch),
            ],
            &mut Timer::new("load textures"),
        );
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

        let transition = self.states.last_mut().unwrap().event(ctx, &mut self.ui);
        // If we fall through, there's a new state that we need to wakeup.
        match transition {
            Transition::Keep => {
                self.ui.per_obj.assert_chosen_used();
                return EventLoopMode::InputOnly;
            }
            Transition::KeepWithMode(evmode) => {
                self.ui.per_obj.assert_chosen_used();
                return evmode;
            }
            Transition::Pop => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                if self.states.is_empty() {
                    self.before_quit(ctx.canvas);
                    std::process::exit(0);
                }
            }
            Transition::PopWithData(cb) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                cb(self.states.last_mut().unwrap(), &mut self.ui, ctx);
            }
            Transition::PopTwice => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
            }
            Transition::Push(state) => {
                self.states
                    .last_mut()
                    .unwrap()
                    .on_suspend(ctx, &mut self.ui);
                self.states.push(state);
            }
            Transition::Replace(state) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                self.states.push(state);
            }
            Transition::PopThenReplace(state) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                assert!(!self.states.is_empty());
                self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                self.states.push(state);
            }
            Transition::Clear(states) => {
                while !self.states.is_empty() {
                    self.states.pop().unwrap().on_destroy(ctx, &mut self.ui);
                }
                self.states.extend(states);
            }
            Transition::ApplyObjectAction(action) => {
                self.ui.per_obj.action_chosen(action);
                // Immediately go trigger the action. Things'll break unless current_selection
                // remains the same, so DON'T redo mouseover.
                return ctx.no_op_event(false, |ctx| self.event(ctx));
            }
            Transition::PushTwice(s1, s2) => {
                self.states
                    .last_mut()
                    .unwrap()
                    .on_suspend(ctx, &mut self.ui);
                self.states.push(s1);
                self.states.push(s2);
                self.ui.per_obj.assert_chosen_used();
                return EventLoopMode::InputOnly;
            }
        };
        self.ui.per_obj.assert_chosen_used();
        // Let the new state initialize with a fake event. Usually these just return
        // Transition::Keep, but nothing stops them from doing whatever. (For example, entering
        // tutorial mode immediately pushes on a Warper.) So just recurse.
        ctx.no_op_event(true, |ctx| self.event(ctx))
    }

    fn draw(&self, g: &mut GfxCtx) {
        let state = self.states.last().unwrap();

        match state.draw_baselayer() {
            DrawBaselayer::DefaultMap => {
                self.ui.draw(
                    g,
                    DrawOptions::new(),
                    &self.ui.primary.sim,
                    &ShowEverything::new(),
                );
            }
            DrawBaselayer::Custom => {}
            DrawBaselayer::PreviousState => {
                match self.states[self.states.len() - 2].draw_baselayer() {
                    DrawBaselayer::DefaultMap => {
                        self.ui.draw(
                            g,
                            DrawOptions::new(),
                            &self.ui.primary.sim,
                            &ShowEverything::new(),
                        );
                    }
                    DrawBaselayer::Custom => {}
                    // Nope, don't recurse
                    DrawBaselayer::PreviousState => {}
                }

                self.states[self.states.len() - 2].draw(g, &self.ui);
            }
        }
        state.draw(g, &self.ui);
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

pub enum DrawBaselayer {
    DefaultMap,
    Custom,
    PreviousState,
}

pub trait State: downcast_rs::Downcast {
    // Logically this returns Transition, but since EventLoopMode is almost always
    // InputOnly, the variations are encoded by Transition.
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition;
    fn draw(&self, g: &mut GfxCtx, ui: &UI);

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::DefaultMap
    }

    // Before we push a new state on top of this one, call this.
    fn on_suspend(&mut self, _: &mut EventCtx, _: &mut UI) {}
    // Before this state is popped or replaced, call this.
    fn on_destroy(&mut self, _: &mut EventCtx, _: &mut UI) {}
    // We don't need an on_enter -- the constructor for the state can just do it.
}

impl dyn State {
    pub fn grey_out_map(g: &mut GfxCtx) {
        // Make it clear the map can't be interacted with right now.
        g.fork_screenspace();
        // TODO - OSD height
        g.draw_polygon(
            Color::BLACK.alpha(0.5),
            &Polygon::rectangle(g.canvas.window_width, g.canvas.window_height),
        );
        g.unfork();
    }
}

downcast_rs::impl_downcast!(State);

pub enum Transition {
    Keep,
    KeepWithMode(EventLoopMode),
    Pop,
    PopTwice,
    // If a state needs to pass data back to the parent, use this. Sadly, runtime type casting.
    PopWithData(Box<dyn FnOnce(&mut Box<dyn State>, &mut UI, &mut EventCtx)>),
    Push(Box<dyn State>),
    Replace(Box<dyn State>),
    PopThenReplace(Box<dyn State>),
    Clear(Vec<Box<dyn State>>),
    ApplyObjectAction(String),
    PushTwice(Box<dyn State>, Box<dyn State>),
}

pub struct WizardState {
    wizard: Wizard,
    // Returning None means stay in this WizardState
    cb: Box<dyn Fn(&mut Wizard, &mut EventCtx, &mut UI) -> Option<Transition>>,
    pub also_draw: Option<Drawable>,
}

impl WizardState {
    pub fn new(
        cb: Box<dyn Fn(&mut Wizard, &mut EventCtx, &mut UI) -> Option<Transition>>,
    ) -> Box<dyn State> {
        Box::new(WizardState {
            wizard: Wizard::new(),
            cb,
            also_draw: None,
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

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // TODO This shouldn't get greyed out, but I think the weird z-ordering of screen-space
        // right now is messing this up.
        if let Some(ref d) = self.also_draw {
            g.redraw(d);
        }

        State::grey_out_map(g);

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
