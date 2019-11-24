use crate::abtest::setup::PickABTest;
use crate::challenges::challenges_picker;
use crate::game::{State, Transition, WizardState};
use crate::mission::MissionEditMode;
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::tutorial::TutorialMode;
use crate::ui::UI;
use abstutil::elapsed_seconds;
use ezgui::{
    layout, Choice, Color, EventCtx, EventLoopMode, GfxCtx, Key, Line, MultiText, ScreenPt, Text,
    TextButton, Wizard,
};
use geom::{Duration, Line, Pt2D, Speed};
use map_model::Map;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::time::Instant;

pub struct TitleScreen {
    play_btn: TextButton,
    screensaver: Screensaver,
    rng: XorShiftRng,
}

impl TitleScreen {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> TitleScreen {
        let mut rng = ui.primary.current_flags.sim_flags.make_rng();
        TitleScreen {
            // TODO logo
            // TODO that nicer font
            play_btn: TextButton::new(Text::from(Line("PLAY")), Color::BLUE, Color::ORANGE, ctx),
            screensaver: Screensaver::start_bounce(&mut rng, ctx, &ui.primary.map),
            rng,
        }
    }
}

impl State for TitleScreen {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        layout::stack_vertically(
            layout::ContainerOrientation::Centered,
            ctx,
            vec![&mut self.play_btn],
        );

        // TODO or any keypress
        self.play_btn.event(ctx);
        if self.play_btn.clicked() {
            return Transition::Replace(Box::new(SplashScreen::new()));
            //return Transition::Replace(Box::new(MainMenu::new(ctx)));
        }

        self.screensaver.update(&mut self.rng, ctx, &ui.primary.map);

        Transition::KeepWithMode(EventLoopMode::Animation)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.play_btn.draw(g);
    }
}

pub struct MainMenu {
    txts: MultiText,
    // TODO Icon with text? Nah, just an image button on a rounded rectangle
    tutorial_btn: TextButton,
    sandbox_btn: TextButton,
    challenges_btn: TextButton,
    about_btn: TextButton,
}

impl MainMenu {
    fn new(ctx: &EventCtx) -> MainMenu {
        let mut txts = MultiText::new();
        // TODO Lots wrong here.
        txts.add(
            Text::from(Line("A/B STREET").size(50)).no_bg(),
            ScreenPt::new(200.0, 100.0),
        );
        txts.add(
            Text::from(Line("Created by Dustin Carlino")).no_bg(),
            ScreenPt::new(250.0, 300.0),
        );
        txts.add(
            Text::from(Line("Choose your game")).no_bg(),
            ScreenPt::new(250.0, 500.0),
        );

        MainMenu {
            txts,
            tutorial_btn: TextButton::new(
                Text::from(Line("TUTORIAL").fg(Color::BLACK)),
                Color::WHITE,
                Color::ORANGE,
                ctx,
            ),
            sandbox_btn: TextButton::new(
                Text::from(Line("SANDBOX").fg(Color::BLACK)),
                Color::WHITE,
                Color::ORANGE,
                ctx,
            ),
            challenges_btn: TextButton::new(
                Text::from(Line("CHALLENGES").fg(Color::BLACK)),
                Color::WHITE,
                Color::ORANGE,
                ctx,
            ),
            // TODO No background at all...
            about_btn: TextButton::new(
                Text::from(Line("About A/B Street").fg(Color::WHITE)),
                Color::BLACK,
                Color::ORANGE,
                ctx,
            ),
        }
    }
}

impl State for MainMenu {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut UI) -> Transition {
        // TODO How do we want to express layouting?
        layout::stack_vertically(
            layout::ContainerOrientation::Centered,
            ctx,
            vec![
                &mut self.tutorial_btn,
                &mut self.sandbox_btn,
                &mut self.challenges_btn,
                &mut self.about_btn,
            ],
        );

        self.tutorial_btn.event(ctx);
        self.sandbox_btn.event(ctx);
        self.challenges_btn.event(ctx);
        self.about_btn.event(ctx);

        // TODO transitions
        // TODO Hotkeys? How to communicate?

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.txts.draw(g);
        self.tutorial_btn.draw(g);
        self.sandbox_btn.draw(g);
        self.challenges_btn.draw(g);
        self.about_btn.draw(g);
    }
}

pub struct SplashScreen {
    wizard: Wizard,
}

impl SplashScreen {
    pub fn new() -> SplashScreen {
        SplashScreen {
            wizard: Wizard::new(),
        }
    }
}

impl State for SplashScreen {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if let Some(t) = splash_screen(&mut self.wizard, ctx, ui) {
            t
        } else if self.wizard.aborted() {
            Transition::Pop
        } else {
            Transition::Keep
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }

    fn on_suspend(&mut self, _: &mut EventCtx, _: &mut UI) {
        self.wizard = Wizard::new();
    }
}

const SPEED: Speed = Speed::const_meters_per_second(20.0);

struct Screensaver {
    line: Line,
    started: Instant,
}

impl Screensaver {
    fn start_bounce(rng: &mut XorShiftRng, ctx: &mut EventCtx, map: &Map) -> Screensaver {
        let at = ctx.canvas.center_to_map_pt();
        let bounds = map.get_bounds();
        // TODO Ideally bounce off the edge of the map
        let goto = Pt2D::new(
            rng.gen_range(0.0, bounds.max_x),
            rng.gen_range(0.0, bounds.max_y),
        );

        ctx.canvas.cam_zoom = 10.0;
        ctx.canvas.center_on_map_pt(at);

        Screensaver {
            line: Line::new(at, goto),
            started: Instant::now(),
        }
    }

    fn update(&mut self, rng: &mut XorShiftRng, ctx: &mut EventCtx, map: &Map) {
        if ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();
            let dist_along = Duration::seconds(elapsed_seconds(self.started)) * SPEED;
            if dist_along < self.line.length() {
                ctx.canvas
                    .center_on_map_pt(self.line.dist_along(dist_along));
            } else {
                *self = Screensaver::start_bounce(rng, ctx, map)
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
