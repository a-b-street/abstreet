use crate::state::{DefaultUIState, Flags, UIState};
use crate::ui::UI;
use abstutil::elapsed_seconds;
use ezgui::{
    Canvas, EventCtx, EventLoopMode, GfxCtx, ModalMenu, Prerender, TopMenu, UserInput, Wizard,
    WrappedWizard, GUI,
};
use geom::{Duration, Line, Pt2D, Speed};
use map_model::Map;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::time::Instant;

pub struct GameState {
    mode: Mode,
    ui: UI<DefaultUIState>,
}

enum Mode {
    SplashScreen(Wizard, Screensaver, XorShiftRng),
    Playing,
}

impl GameState {
    pub fn new(flags: Flags, canvas: &mut Canvas, prerender: &Prerender) -> GameState {
        let mut rng = flags.sim_flags.make_rng();
        let ui = UI::new(DefaultUIState::new(flags, prerender, true), canvas);
        let screensaver =
            Screensaver::start_bounce(&mut rng, canvas, &ui.state.get_state().primary.map);
        GameState {
            mode: Mode::SplashScreen(Wizard::new(), screensaver, rng),
            ui,
        }
    }
}

impl GUI for GameState {
    // TODO Don't display this unless mode is Playing! But that probably means we have to drag the
    // management of more ezgui state here.
    fn top_menu(&self, canvas: &Canvas) -> Option<TopMenu> {
        self.ui.top_menu(canvas)
    }

    fn modal_menus(&self) -> Vec<ModalMenu> {
        self.ui.modal_menus()
    }

    fn event(&mut self, mut ctx: EventCtx) -> EventLoopMode {
        match self.mode {
            Mode::SplashScreen(ref mut wizard, ref mut screensaver, ref mut rng) => {
                screensaver.update(
                    rng,
                    &mut ctx.input,
                    &mut ctx.canvas,
                    &self.ui.state.get_state().primary.map,
                );

                if splash_screen(wizard.wrap(&mut ctx.input, ctx.canvas), &mut self.ui).is_some() {
                    self.mode = Mode::Playing;
                }
                EventLoopMode::Animation
            }
            Mode::Playing => self.ui.event(ctx),
        }
    }

    fn draw(&self, g: &mut GfxCtx, screencap: bool) -> Option<String> {
        match self.mode {
            Mode::SplashScreen(ref wizard, _, _) => {
                // Draw the map in the background.
                self.ui.draw(g, screencap);

                wizard.draw(g);
                None
            }
            Mode::Playing => self.ui.draw(g, screencap),
        }
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        self.ui.dump_before_abort(canvas);
    }

    fn before_quit(&self, canvas: &Canvas) {
        self.ui.before_quit(canvas);
    }

    fn profiling_enabled(&self) -> bool {
        self.ui.profiling_enabled()
    }
}

fn splash_screen(mut wizard: WrappedWizard, ui: &mut UI<DefaultUIState>) -> Option<()> {
    let play = "Play";
    let quit = "Quit";
    match wizard
        .choose_string("Welcome to A/B Street!", vec![play, quit])?
        .as_str()
    {
        x if x == play => Some(()),
        x if x == quit => {
            ui.before_quit(wizard.canvas);
            std::process::exit(0);
        }
        _ => unreachable!(),
    }
}

const SPEED: Speed = Speed::const_meters_per_second(50.0);

// TODO I'd prefer to store the XorShiftRng in here, but it makes borrow checking without some
// boilerplate hard.
struct Screensaver {
    line: Line,
    started: Instant,
}

impl Screensaver {
    fn start_bounce(rng: &mut XorShiftRng, canvas: &mut Canvas, map: &Map) -> Screensaver {
        let at = canvas.center_to_map_pt();
        let bounds = map.get_bounds();
        // TODO Ideally bounce off the edge of the map
        let goto = Pt2D::new(
            rng.gen_range(0.0, bounds.max_x),
            rng.gen_range(0.0, bounds.max_y),
        );

        canvas.cam_zoom = 10.0;
        canvas.center_on_map_pt(at);

        Screensaver {
            line: Line::new(at, goto),
            started: Instant::now(),
        }
    }

    fn update(
        &mut self,
        rng: &mut XorShiftRng,
        input: &mut UserInput,
        canvas: &mut Canvas,
        map: &Map,
    ) {
        if input.nonblocking_is_update_event() {
            input.use_update_event();
            let dist_along = Duration::seconds(elapsed_seconds(self.started)) * SPEED;
            if dist_along < self.line.length() {
                canvas.center_on_map_pt(self.line.dist_along(dist_along));
            } else {
                *self = Screensaver::start_bounce(rng, canvas, map)
            }
        }
    }
}
