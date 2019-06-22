use crate::render::DrawOptions;
use crate::sandbox::SandboxMode;
use crate::splash_screen::SplashScreen;
use crate::state::{State, Transition};
use crate::ui::{EditorState, Flags, ShowEverything, UI};
use ezgui::{Canvas, EventCtx, EventLoopMode, GfxCtx, GUI};

// This is the top-level of the GUI logic. This module should just manage interactions between the
// top-level game states.
pub struct Game {
    // A stack of states
    pub states: Vec<Box<State>>,
    pub ui: UI,
}

// TODO Need to reset_sim() when entering Edit, Tutorial, Mission, or ABTest and when leaving
// Tutorial and ABTest. Expressing this manually right now is quite tedious; maybe having on_enter
// and on_exit would be cleaner.

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
        Game { states, ui }
    }

    fn save_editor_state(&self, canvas: &Canvas) {
        let state = EditorState {
            map_name: self.ui.primary.map.get_name().clone(),
            cam_x: canvas.cam_x,
            cam_y: canvas.cam_y,
            cam_zoom: canvas.cam_zoom,
        };
        // TODO maybe make state line up with the map, so loading from a new map doesn't break
        abstutil::write_json("../editor_state.json", &state)
            .expect("Saving editor_state.json failed");
        println!("Saved editor_state.json");
    }
}

impl GUI for Game {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        let (transition, evloop) = self.states.last_mut().unwrap().event(ctx, &mut self.ui);
        match transition {
            Transition::Keep => {}
            Transition::Pop => {
                self.states.pop();
                if self.states.is_empty() {
                    self.before_quit(ctx.canvas);
                    std::process::exit(0);
                }
            }
            Transition::Push(state) => {
                self.states.push(state);
            }
            Transition::Replace(state) => {
                self.states.pop();
                self.states.push(state);
            }
        }
        evloop
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
        /*if let Mode::ABTest(ref abtest) = self.mode {
            if let Some(ref s) = abtest.secondary {
                println!("Secondary sim:");
                s.sim.dump_before_abort();
            }
        }*/
        self.save_editor_state(canvas);
    }

    fn before_quit(&self, canvas: &Canvas) {
        self.save_editor_state(canvas);
        self.ui.cs.save();
        println!("Saved color_scheme.json");
    }

    fn profiling_enabled(&self) -> bool {
        self.ui.primary.current_flags.enable_profiler
    }
}
