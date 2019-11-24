use crate::abtest::setup::PickABTest;
use crate::challenges::challenges_picker;
use crate::game::{State, Transition};
use crate::managed::ManagedGUIState;
use crate::mission::MissionEditMode;
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::tutorial::TutorialMode;
use crate::ui::UI;
use abstutil::elapsed_seconds;
use ezgui::{
    layout, Color, EventCtx, EventLoopMode, GfxCtx, JustDraw, Line, ScreenPt, Text, TextButton,
};
use geom::{Duration, Line, Pt2D, Speed};
use map_model::Map;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::time::Instant;

pub struct TitleScreen {
    logo: JustDraw,
    play_btn: TextButton,
    screensaver: Screensaver,
    rng: XorShiftRng,
}

impl TitleScreen {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> TitleScreen {
        let mut rng = ui.primary.current_flags.sim_flags.make_rng();
        TitleScreen {
            logo: JustDraw::image("assets/logo.png", ctx),
            // TODO that nicer font
            play_btn: TextButton::new(Text::from(Line("PLAY")), Color::BLUE, Color::ORANGE, ctx),
            screensaver: Screensaver::start_bounce(&mut rng, ctx, &ui.primary.map),
            rng,
        }
    }
}

impl State for TitleScreen {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        // TODO I'm betting that I'll wind up extracting the ManagedGUIState pattern to work along
        // with another state
        layout::stack_vertically(
            layout::ContainerOrientation::Centered,
            ctx,
            vec![&mut self.logo, &mut self.play_btn],
        );

        // TODO or any keypress
        self.play_btn.event(ctx);
        if self.play_btn.clicked() {
            return Transition::Replace(main_menu(ctx, ui));
        }

        self.screensaver.update(&mut self.rng, ctx, &ui.primary.map);

        Transition::KeepWithMode(EventLoopMode::Animation)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.logo.draw(g);
        self.play_btn.draw(g);
    }
}

pub fn main_menu(ctx: &EventCtx, ui: &UI) -> Box<dyn State> {
    let mut state = ManagedGUIState::builder();

    state.draw_text(
        Text::from(Line("A/B STREET").size(50)).no_bg(),
        ScreenPt::new(200.0, 100.0),
    );
    state.draw_text(
        Text::from(Line("Created by Dustin Carlino")).no_bg(),
        ScreenPt::new(250.0, 300.0),
    );
    state.draw_text(
        Text::from(Line("Choose your game")).no_bg(),
        ScreenPt::new(250.0, 500.0),
    );

    state.text_button(
        "TUTORIAL",
        Box::new(|ctx, _| Some(Transition::Push(Box::new(TutorialMode::new(ctx))))),
    );
    state.text_button(
        "SANDBOX",
        Box::new(|ctx, ui| {
            Some(Transition::Push(Box::new(SandboxMode::new(
                ctx,
                ui,
                GameplayMode::Freeform,
            ))))
        }),
    );
    state.text_button(
        "CHALLENGES",
        Box::new(|_, _| Some(Transition::Push(challenges_picker()))),
    );
    if ui.primary.current_flags.dev {
        state.text_button(
            "INTERNAL DEV TOOLS",
            Box::new(|ctx, _| Some(Transition::Push(Box::new(MissionEditMode::new(ctx))))),
        );
        state.text_button(
            "INTERNAL A/B TEST MODE",
            Box::new(|_, _| Some(Transition::Push(PickABTest::new()))),
        );
    }
    state.text_button(
        "About A/B Street",
        Box::new(|ctx, _| Some(Transition::Push(about(ctx)))),
    );
    state.text_button(
        "QUIT",
        Box::new(|_, _| {
            // TODO before_quit?
            std::process::exit(0);
        }),
    );
    state.build(ctx)
}

fn about(ctx: &EventCtx) -> Box<dyn State> {
    let mut state = ManagedGUIState::builder();

    let mut txt = Text::new().no_bg();
    txt.add(Line("A/B STREET").size(50));
    txt.add(Line("Created by Dustin Carlino"));
    txt.add(Line(""));
    txt.add(Line("Contact: dabreegster@gmail.com"));
    txt.add(Line("Project: http://github.com/dabreegster/abstreet"));
    txt.add(Line("Map data from OpenStreetMap and King County GIS"));
    // TODO Add more here
    txt.add(Line(
        "See full credits at https://github.com/dabreegster/abstreet#credits",
    ));
    // TODO centered
    state.draw_text(txt, ScreenPt::new(100.0, 100.0));

    state.text_button("BACK", Box::new(|_, _| Some(Transition::Pop)));

    state.build(ctx)
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
