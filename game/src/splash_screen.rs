use crate::abtest::setup::PickABTest;
use crate::challenges::challenges_picker;
use crate::game::{State, Transition, WizardState};
use crate::mission::MissionEditMode;
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::tutorial::TutorialMode;
use crate::ui::UI;
use abstutil::elapsed_seconds;
use ezgui::{Canvas, Choice, EventCtx, EventLoopMode, GfxCtx, Key, UserInput, Wizard};
use geom::{Duration, Line, Pt2D, Speed};
use map_model::Map;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::time::Instant;

pub struct SplashScreen {
    wizard: Wizard,
    maybe_screensaver: Option<(Screensaver, XorShiftRng)>,
}

impl SplashScreen {
    pub fn new_without_screensaver() -> SplashScreen {
        SplashScreen {
            wizard: Wizard::new(),
            maybe_screensaver: None,
        }
    }

    pub fn new_with_screensaver(ctx: &mut EventCtx, ui: &UI) -> SplashScreen {
        let mut rng = ui.primary.current_flags.sim_flags.make_rng();
        SplashScreen {
            wizard: Wizard::new(),
            maybe_screensaver: Some((
                Screensaver::start_bounce(&mut rng, ctx.canvas, &ui.primary.map),
                rng,
            )),
        }
    }
}

impl State for SplashScreen {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if let Some((ref mut screensaver, ref mut rng)) = self.maybe_screensaver {
            screensaver.update(rng, ctx.input, ctx.canvas, &ui.primary.map);
        }

        let evmode = if self.maybe_screensaver.is_some() {
            EventLoopMode::Animation
        } else {
            EventLoopMode::InputOnly
        };

        if let Some(t) = splash_screen(&mut self.wizard, ctx, ui) {
            t
        } else if self.wizard.aborted() {
            Transition::PopWithMode(evmode)
        } else {
            Transition::KeepWithMode(evmode)
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }

    fn on_suspend(&mut self, _: &mut EventCtx, _: &mut UI) {
        self.wizard.reset();
        self.maybe_screensaver = None;
    }
}

const SPEED: Speed = Speed::const_meters_per_second(20.0);

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

fn splash_screen(raw_wizard: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let mut wizard = raw_wizard.wrap(ctx);
    let sandbox = "Sandbox mode";
    let challenge = "Challenge mode";
    let abtest = "A/B Test Mode (internal/unfinished)";
    let tutorial = "Tutorial (unfinished)";
    let mission = "Internal developer tools";
    let about = "About";
    let quit = "Quit";

    let dev = ui.primary.current_flags.dev;

    match wizard
        .choose("Welcome to A/B Street!", || {
            vec![
                Some(Choice::new(sandbox, ()).key(Key::S)),
                Some(Choice::new(challenge, ()).key(Key::C)),
                if dev {
                    Some(Choice::new(abtest, ()).key(Key::A))
                } else {
                    None
                },
                if dev {
                    Some(Choice::new(tutorial, ()).key(Key::T))
                } else {
                    None
                },
                if dev {
                    Some(Choice::new(mission, ()).key(Key::M))
                } else {
                    None
                },
                Some(Choice::new(about, ())),
                Some(Choice::new(quit, ())),
            ]
            .into_iter()
            .flatten()
            .collect()
        })?
        .0
        .as_str()
    {
        x if x == sandbox => Some(Transition::Push(Box::new(SandboxMode::new(
            ctx,
            ui,
            GameplayMode::Freeform,
        )))),
        x if x == challenge => Some(Transition::Push(challenges_picker())),
        x if x == abtest => Some(Transition::Push(PickABTest::new())),
        x if x == tutorial => Some(Transition::Push(Box::new(TutorialMode::new(ctx)))),
        x if x == mission => Some(Transition::Push(Box::new(MissionEditMode::new(ctx)))),
        x if x == about => Some(Transition::Push(WizardState::new(Box::new(
            |wiz, ctx, _| {
                wiz.wrap(ctx).acknowledge("About A/B Street", || {
                    vec![
                        "Author: Dustin Carlino (dabreegster@gmail.com)",
                        "http://github.com/dabreegster/abstreet",
                        "Map data from OpenStreetMap and King County GIS",
                        "",
                        "Press ENTER to continue",
                    ]
                })?;
                Some(Transition::Pop)
            },
        )))),
        x if x == quit => Some(Transition::Pop),
        _ => unreachable!(),
    }
}
