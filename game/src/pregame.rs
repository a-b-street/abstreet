use crate::abtest::setup::PickABTest;
use crate::challenges::challenges_picker;
use crate::game::{State, Transition};
use crate::managed::{LayoutStyle, ManagedGUIState, ManagedWidget};
use crate::mission::MissionEditMode;
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::tutorial::TutorialMode;
use crate::ui::UI;
use ezgui::{
    hotkey, layout, Button, Color, EventCtx, EventLoopMode, GfxCtx, JustDraw, Key, Line, Text,
};
use geom::{Duration, Line, Pt2D, Speed};
use map_model::Map;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::time::Instant;

pub struct TitleScreen {
    logo: JustDraw,
    play_btn: Button,
    screensaver: Screensaver,
    rng: XorShiftRng,
}

impl TitleScreen {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> TitleScreen {
        let mut rng = ui.primary.current_flags.sim_flags.make_rng();
        TitleScreen {
            logo: JustDraw::image("assets/pregame/logo.png", ctx),
            // TODO that nicer font
            // TODO Any key
            play_btn: Button::text(
                Text::from(Line("PLAY")),
                Color::BLUE,
                Color::ORANGE,
                hotkey(Key::Space),
                "start game",
                ctx,
            ),
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
    let mut col = Vec::new();

    col.push(ManagedWidget::Row(
        LayoutStyle::Neutral,
        vec![
            ManagedWidget::svg_button(
                ctx,
                "assets/pregame/quit.svg",
                "quit",
                hotkey(Key::Escape),
                Box::new(|_, _| {
                    // TODO before_quit?
                    std::process::exit(0);
                }),
            ),
            ManagedWidget::draw_text(ctx, Text::from(Line("A/B STREET").size(50))),
        ],
    ));

    col.push(ManagedWidget::draw_text(
        ctx,
        Text::from(Line("Created by Dustin Carlino")),
    ));
    col.push(ManagedWidget::draw_text(
        ctx,
        Text::from(Line("Choose your game")),
    ));

    col.push(ManagedWidget::Row(
        LayoutStyle::Centered,
        vec![
            ManagedWidget::svg_button(
                ctx,
                "assets/pregame/tutorial.svg",
                "Tutorial",
                hotkey(Key::T),
                Box::new(|ctx, _| Some(Transition::Push(Box::new(TutorialMode::new(ctx))))),
            ),
            ManagedWidget::svg_button(
                ctx,
                "assets/pregame/sandbox.svg",
                "Sandbox mode",
                hotkey(Key::S),
                Box::new(|ctx, ui| {
                    Some(Transition::Push(Box::new(SandboxMode::new(
                        ctx,
                        ui,
                        GameplayMode::Freeform,
                    ))))
                }),
            ),
            ManagedWidget::img_button(
                ctx,
                "assets/pregame/challenges.png",
                hotkey(Key::C),
                Box::new(|ctx, _| Some(Transition::Push(challenges_picker(ctx)))),
            ),
        ],
    ));
    if ui.opts.dev {
        col.push(ManagedWidget::Row(
            LayoutStyle::Centered,
            vec![
                ManagedWidget::text_button(
                    ctx,
                    "INTERNAL DEV TOOLS",
                    hotkey(Key::M),
                    Box::new(|ctx, _| Some(Transition::Push(Box::new(MissionEditMode::new(ctx))))),
                ),
                ManagedWidget::text_button(
                    ctx,
                    "INTERNAL A/B TEST MODE",
                    hotkey(Key::A),
                    Box::new(|_, _| Some(Transition::Push(PickABTest::new()))),
                ),
            ],
        ));
    }
    col.push(ManagedWidget::text_button(
        ctx,
        "About A/B Street",
        None,
        Box::new(|ctx, _| Some(Transition::Push(about(ctx)))),
    ));
    ManagedGUIState::new(ManagedWidget::Column(LayoutStyle::Centered, col))
}

fn about(ctx: &EventCtx) -> Box<dyn State> {
    let mut row = Vec::new();

    row.push(ManagedWidget::svg_button(
        ctx,
        "assets/pregame/back.svg",
        "back",
        hotkey(Key::Escape),
        Box::new(|_, _| Some(Transition::Pop)),
    ));

    let mut txt = Text::new();
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
    row.push(ManagedWidget::draw_text(ctx, txt));

    ManagedGUIState::new(ManagedWidget::Row(LayoutStyle::Neutral, row))
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
            let dist_along = Duration::realtime_elapsed(self.started) * SPEED;
            if dist_along < self.line.length() {
                ctx.canvas
                    .center_on_map_pt(self.line.dist_along(dist_along));
            } else {
                *self = Screensaver::start_bounce(rng, ctx, map)
            }
        }
    }
}
