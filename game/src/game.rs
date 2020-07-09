use crate::app::{App, Flags, ShowEverything};
use crate::options::Options;
use crate::pregame::TitleScreen;
use crate::render::DrawOptions;
use crate::sandbox::{GameplayMode, SandboxMode};
use ezgui::{Canvas, Drawable, EventCtx, GfxCtx, Wizard, GUI};
use geom::Polygon;
use map_model::PermanentMapEdits;

// This is the top-level of the GUI logic. This module should just manage interactions between the
// top-level game states.
pub struct Game {
    // A stack of states
    states: Vec<Box<dyn State>>,
    app: App,
}

impl Game {
    pub fn new(
        flags: Flags,
        opts: Options,
        start_with_edits: Option<String>,
        maybe_mode: Option<GameplayMode>,
        ctx: &mut EventCtx,
    ) -> Game {
        let title = !opts.dev
            && !flags.sim_flags.load.contains("data/player/save")
            && !flags.sim_flags.load.contains("data/system/scenarios")
            && maybe_mode.is_none();
        let mut app = App::new(flags, opts, ctx, title);

        // Handle savestates
        let savestate = if app
            .primary
            .current_flags
            .sim_flags
            .load
            .starts_with("../data/player/saves/")
        {
            assert!(maybe_mode.is_none());
            Some(app.primary.clear_sim())
        } else {
            None
        };

        // Just apply this here, don't plumb to SimFlags or anything else. We recreate things using
        // these flags later, but we don't want to keep applying the same edits.
        if let Some(edits_name) = start_with_edits {
            // TODO Maybe loading screen
            let mut timer = abstutil::Timer::new("apply initial edits");
            let edits =
                map_model::MapEdits::load(&app.primary.map, &edits_name, &mut timer).unwrap();
            crate::edit::apply_map_edits(ctx, &mut app, edits);
            app.primary
                .map
                .recalculate_pathfinding_after_edits(&mut timer);
            app.primary.clear_sim();
        }

        let states: Vec<Box<dyn State>> = if title {
            vec![Box::new(TitleScreen::new(ctx, &mut app))]
        } else {
            // TODO We're assuming we never wind up starting freeform mode with a synthetic map
            let mode = maybe_mode.unwrap_or_else(|| {
                GameplayMode::Freeform(abstutil::path_map(app.primary.map.get_name()))
            });
            vec![Box::new(SandboxMode::new(ctx, &mut app, mode))]
        };
        if let Some(ss) = savestate {
            // TODO This is weird, we're left in Freeform mode with the wrong UI. Can't instantiate
            // PlayScenario without clobbering.
            app.primary.sim = ss;
        }
        Game { states, app }
    }
}

impl GUI for Game {
    fn event(&mut self, ctx: &mut EventCtx) {
        self.app.per_obj.reset();

        let transition = self.states.last_mut().unwrap().event(ctx, &mut self.app);
        // If we fall through, there's a new state that we need to wakeup.
        match transition {
            Transition::Keep => {
                return;
            }
            Transition::KeepWithMouseover => {}
            Transition::Pop => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.app);
                if self.states.is_empty() {
                    self.before_quit(ctx.canvas);
                    std::process::exit(0);
                }
            }
            Transition::PopWithData(cb) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.app);
                cb(self.states.last_mut().unwrap(), ctx, &mut self.app);
            }
            Transition::ReplaceWithData(cb) => {
                let mut last = self.states.pop().unwrap();
                last.on_destroy(ctx, &mut self.app);
                let new_states = cb(last, ctx, &mut self.app);
                self.states.extend(new_states);
            }
            Transition::KeepWithData(cb) => {
                cb(self.states.last_mut().unwrap(), ctx, &mut self.app);
            }
            Transition::PopTwice => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.app);
                self.states.pop().unwrap().on_destroy(ctx, &mut self.app);
            }
            Transition::Push(state) => {
                self.states.push(state);
            }
            Transition::Replace(state) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.app);
                self.states.push(state);
            }
            Transition::ReplaceThenPush(state1, state2) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.app);
                self.states.push(state1);
                self.states.push(state2);
            }
            Transition::PopThenReplace(state) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.app);
                assert!(!self.states.is_empty());
                self.states.pop().unwrap().on_destroy(ctx, &mut self.app);
                self.states.push(state);
            }
            Transition::PopThenReplaceThenPush(state1, state2) => {
                self.states.pop().unwrap().on_destroy(ctx, &mut self.app);
                assert!(!self.states.is_empty());
                self.states.pop().unwrap().on_destroy(ctx, &mut self.app);
                self.states.push(state1);
                self.states.push(state2);
            }
            Transition::Clear(states) => {
                while !self.states.is_empty() {
                    self.states.pop().unwrap().on_destroy(ctx, &mut self.app);
                }
                self.states.extend(states);
            }
            Transition::PushTwice(s1, s2) => {
                self.states.push(s1);
                self.states.push(s2);
            }
        };
        // Let the new state initialize with a fake event. Usually these just return
        // Transition::Keep, but nothing stops them from doing whatever. (For example, entering
        // tutorial mode immediately pushes on a Warper.) So just recurse.
        ctx.no_op_event(true, |ctx| self.event(ctx))
    }

    fn draw(&self, g: &mut GfxCtx) {
        let state = self.states.last().unwrap();

        match state.draw_baselayer() {
            DrawBaselayer::DefaultMap => {
                self.app.draw(
                    g,
                    DrawOptions::new(),
                    &self.app.primary.sim,
                    &ShowEverything::new(),
                );
            }
            DrawBaselayer::Custom => {}
            DrawBaselayer::PreviousState => {
                match self.states[self.states.len() - 2].draw_baselayer() {
                    DrawBaselayer::DefaultMap => {
                        self.app.draw(
                            g,
                            DrawOptions::new(),
                            &self.app.primary.sim,
                            &ShowEverything::new(),
                        );
                    }
                    DrawBaselayer::Custom => {}
                    // Nope, don't recurse
                    DrawBaselayer::PreviousState => {}
                }

                self.states[self.states.len() - 2].draw(g, &self.app);
            }
        }
        state.draw(g, &self.app);
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        println!();
        println!(
            "********************************************************************************"
        );
        canvas.save_camera_state(self.app.primary.map.get_name());
        println!(
            "Crash! Please report to https://github.com/dabreegster/abstreet/issues/ and include \
             all output.txt; at least everything starting from the stack trace above!"
        );

        println!();
        self.app.primary.sim.dump_before_abort();

        println!();
        println!("Camera:");
        println!(
            r#"{{ "cam_x": {}, "cam_y": {}, "cam_zoom": {} }}"#,
            canvas.cam_x, canvas.cam_y, canvas.cam_zoom
        );

        println!();
        if self.app.primary.map.get_edits().commands.is_empty() {
            println!("No edits");
        } else {
            println!("Edits:");
            println!(
                "{}",
                abstutil::to_json(&PermanentMapEdits::to_permanent(
                    self.app.primary.map.get_edits(),
                    &self.app.primary.map
                ))
            );
        }

        // Repeat, because it can be hard to see the top of the report if it's long
        println!();
        println!(
            "Crash! Please report to https://github.com/dabreegster/abstreet/issues/ and include \
             all output.txt; at least everything above here until the start of the report!"
        );
        println!(
            "********************************************************************************"
        );
    }

    fn before_quit(&self, canvas: &Canvas) {
        canvas.save_camera_state(self.app.primary.map.get_name());
    }
}

pub enum DrawBaselayer {
    DefaultMap,
    Custom,
    PreviousState,
}

pub trait State: downcast_rs::Downcast {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition;
    fn draw(&self, g: &mut GfxCtx, app: &App);

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::DefaultMap
    }

    // Before this state is popped or replaced, call this.
    fn on_destroy(&mut self, _: &mut EventCtx, _: &mut App) {}
    // We don't need an on_enter -- the constructor for the state can just do it.
}

impl dyn State {
    pub fn grey_out_map(g: &mut GfxCtx, app: &App) {
        // Make it clear the map can't be interacted with right now.
        g.fork_screenspace();
        // TODO - OSD height
        g.draw_polygon(
            app.cs.fade_map_dark,
            &Polygon::rectangle(g.canvas.window_width, g.canvas.window_height),
        );
        g.unfork();
    }
}

downcast_rs::impl_downcast!(State);

pub enum Transition {
    Keep,
    KeepWithMouseover,
    Pop,
    PopTwice,
    // If a state needs to pass data back to the parent, use this. Sadly, runtime type casting.
    PopWithData(Box<dyn FnOnce(&mut Box<dyn State>, &mut EventCtx, &mut App)>),
    KeepWithData(Box<dyn FnOnce(&mut Box<dyn State>, &mut EventCtx, &mut App)>),
    ReplaceWithData(
        Box<dyn FnOnce(Box<dyn State>, &mut EventCtx, &mut App) -> Vec<Box<dyn State>>>,
    ),
    Push(Box<dyn State>),
    Replace(Box<dyn State>),
    ReplaceThenPush(Box<dyn State>, Box<dyn State>),
    PopThenReplace(Box<dyn State>),
    PopThenReplaceThenPush(Box<dyn State>, Box<dyn State>),
    Clear(Vec<Box<dyn State>>),
    PushTwice(Box<dyn State>, Box<dyn State>),
}

pub struct WizardState {
    wizard: Wizard,
    // Returning None means stay in this WizardState
    cb: Box<dyn Fn(&mut Wizard, &mut EventCtx, &mut App) -> Option<Transition>>,
    // (unzoomed, zoomed)
    pub also_draw: Option<(Drawable, Drawable)>,
    pub custom_pop: Option<Transition>,
}

impl WizardState {
    pub fn new(
        cb: Box<dyn Fn(&mut Wizard, &mut EventCtx, &mut App) -> Option<Transition>>,
    ) -> Box<dyn State> {
        Box::new(WizardState {
            wizard: Wizard::new(),
            cb,
            also_draw: None,
            custom_pop: None,
        })
    }
}

impl State for WizardState {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if self.also_draw.is_some() {
            // TODO This should really be a separate option
            ctx.canvas_movement();
        }
        if let Some(t) = (self.cb)(&mut self.wizard, ctx, app) {
            return t;
        } else if self.wizard.aborted() {
            if let Some(t) = self.custom_pop.take() {
                return t;
            }
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if let Some((ref unzoomed, ref zoomed)) = self.also_draw {
            if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
                g.redraw(unzoomed);
            } else {
                g.redraw(zoomed);
            }
        } else {
            State::grey_out_map(g, app);
        }

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
