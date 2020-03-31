use crate::abtest::setup::PickABTest;
use crate::app::App;
use crate::challenges::challenges_picker;
use crate::colors;
use crate::devtools::DevToolsMode;
use crate::game::{State, Transition};
use crate::managed::{Callback, ManagedGUIState, WrappedComposite, WrappedOutcome};
use crate::sandbox::{GameplayMode, SandboxMode, TutorialPointer};
use ezgui::{
    hotkey, hotkeys, Btn, Color, Composite, EventCtx, EventLoopMode, GfxCtx, Key, Line,
    RewriteColor, Text, Widget,
};
use geom::{Duration, Line, Pt2D, Speed};
use instant::Instant;
use map_model::{Map, MapEdits};
use rand::Rng;
use rand_xorshift::XorShiftRng;

pub struct TitleScreen {
    composite: WrappedComposite,
    screensaver: Screensaver,
    rng: XorShiftRng,
}

impl TitleScreen {
    pub fn new(ctx: &mut EventCtx, app: &App) -> TitleScreen {
        let mut rng = app.primary.current_flags.sim_flags.make_rng();
        TitleScreen {
            composite: WrappedComposite::new(
                Composite::new(
                    Widget::col(vec![
                        Widget::draw_svg(ctx, "../data/system/assets/pregame/logo.svg").margin(5),
                        // TODO Any key
                        // TODO The hover color is wacky
                        Btn::svg_def("../data/system/assets/pregame/start.svg")
                            .build(ctx, "start game", hotkeys(vec![Key::Space, Key::Enter]))
                            .margin(5),
                    ])
                    .bg(app.cs.get("grass"))
                    .outline(3.0, Color::BLACK)
                    .centered(),
                )
                .build(ctx),
            )
            .cb(
                "start game",
                Box::new(|ctx, app| Some(Transition::Replace(main_menu(ctx, app)))),
            ),
            screensaver: Screensaver::start_bounce(&mut rng, ctx, &app.primary.map),
            rng,
        }
    }
}

impl State for TitleScreen {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => t,
            Some(WrappedOutcome::Clicked(_)) => unreachable!(),
            None => {
                self.screensaver
                    .update(&mut self.rng, ctx, &app.primary.map);
                Transition::KeepWithMode(EventLoopMode::Animation)
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
    }
}

pub fn main_menu(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
    let col = vec![
        Btn::svg_def("../data/system/assets/pregame/quit.svg")
            .build(ctx, "quit", hotkey(Key::Escape))
            .align_left(),
        {
            let mut txt = Text::from(Line("A/B STREET").display_title());
            txt.add(Line("Created by Dustin Carlino"));
            txt.draw(ctx).centered_horiz()
        },
        Widget::row(vec![
            Btn::svg(
                "../data/system/assets/pregame/tutorial.svg",
                RewriteColor::Change(Color::WHITE, colors::HOVERING),
            )
            .tooltip({
                let mut txt = Text::tooltip(hotkey(Key::T), "Tutorial");
                txt.add(Line("Learn how to play the game").small());
                txt
            })
            .build(ctx, "Tutorial", hotkey(Key::T)),
            Btn::svg(
                "../data/system/assets/pregame/sandbox.svg",
                RewriteColor::Change(Color::WHITE, colors::HOVERING),
            )
            .tooltip({
                let mut txt = Text::tooltip(hotkey(Key::S), "Sandbox");
                txt.add(Line("No goals, try out any idea here").small());
                txt
            })
            .build(ctx, "Sandbox mode", hotkey(Key::S)),
            Btn::svg(
                "../data/system/assets/pregame/challenges.svg",
                RewriteColor::Change(Color::WHITE, colors::HOVERING),
            )
            .tooltip({
                let mut txt = Text::tooltip(hotkey(Key::C), "Challenges");
                txt.add(Line("Fix specific problems").small());
                txt
            })
            .build(ctx, "Challenges", hotkey(Key::C)),
            Btn::text_bg2("Community Proposals")
                .tooltip({
                    let mut txt = Text::tooltip(hotkey(Key::P), "Community Proposals");
                    txt.add(Line("See existing ideas for improving traffic").small());
                    txt
                })
                .build_def(ctx, hotkey(Key::P)),
        ])
        .centered(),
        if app.opts.dev {
            Widget::row(vec![
                Btn::text_bg2("Internal Dev Tools").build_def(ctx, hotkey(Key::M)),
                Btn::text_bg2("Internal A/B Test Mode").build_def(ctx, hotkey(Key::A)),
            ])
            .centered()
        } else {
            Widget::nothing()
        },
        Widget::col(vec![
            Btn::text_bg2("About A/B Street").build_def(ctx, None),
            built_info::time().draw(ctx),
        ])
        .centered(),
    ];

    let mut c = WrappedComposite::new(
        Composite::new(Widget::col(col).evenly_spaced())
            .exact_size_percent(90, 85)
            .build(ctx),
    )
    .cb(
        "quit",
        Box::new(|_, _| {
            // TODO before_quit?
            std::process::exit(0);
        }),
    )
    .cb(
        "Tutorial",
        Box::new(|ctx, app| {
            Some(Transition::Push(Box::new(SandboxMode::new(
                ctx,
                app,
                GameplayMode::Tutorial(
                    app.session
                        .tutorial
                        .as_ref()
                        .map(|tut| tut.current)
                        .unwrap_or(TutorialPointer::new(0, 0)),
                ),
            ))))
        }),
    )
    .cb(
        "Sandbox mode",
        Box::new(|ctx, app| {
            // We might've left with a synthetic map loaded.
            let map_path = if abstutil::list_all_objects(abstutil::path_all_maps())
                .contains(app.primary.map.get_name())
            {
                abstutil::path_map(app.primary.map.get_name())
            } else {
                abstutil::path_map("montlake")
            };
            let scenario = if abstutil::file_exists(abstutil::path_scenario(
                app.primary.map.get_name(),
                "weekday",
            )) {
                "weekday"
            } else {
                "random"
            };
            Some(Transition::Push(Box::new(SandboxMode::new(
                ctx,
                app,
                GameplayMode::PlayScenario(map_path, scenario.to_string()),
            ))))
        }),
    )
    .cb(
        "Challenges",
        Box::new(|ctx, app| Some(Transition::Push(challenges_picker(ctx, app)))),
    )
    .cb(
        "About A/B Street",
        Box::new(|ctx, _| Some(Transition::Push(about(ctx)))),
    )
    .cb(
        "Community Proposals",
        Box::new(|ctx, _| Some(Transition::Push(proposals_picker(ctx)))),
    );
    if app.opts.dev {
        c = c
            .cb(
                "Internal Dev Tools",
                Box::new(|ctx, _| Some(Transition::Push(DevToolsMode::new(ctx)))),
            )
            .cb(
                "Internal A/B Test Mode",
                Box::new(|_, _| Some(Transition::Push(PickABTest::new()))),
            );
    }
    ManagedGUIState::fullscreen(c)
}

fn about(ctx: &mut EventCtx) -> Box<dyn State> {
    let col = vec![
        Btn::svg_def("../data/system/assets/pregame/back.svg")
            .build(ctx, "back", hotkey(Key::Escape))
            .align_left(),
        {
            let mut txt = Text::new();
            txt.add(Line("A/B STREET").display_title());
            txt.add(Line("Created by Dustin Carlino, UX by Yuwen Li"));
            txt.add(Line(""));
            txt.add(Line("Contact: dabreegster@gmail.com"));
            txt.add(Line(
                "Project: http://github.com/dabreegster/abstreet (aliased by abstreet.org)",
            ));
            txt.add(Line("Map data from OpenStreetMap and King County GIS"));
            // TODO Add more here
            txt.add(Line(
                "See full credits at https://github.com/dabreegster/abstreet#credits",
            ));
            txt.add(Line(""));
            // TODO Word wrapping please?
            txt.add(Line(
                "Disclaimer: This game is based on imperfect data, heuristics ",
            ));
            txt.add(Line(
                "concocted under the influence of cold brew, a simplified traffic ",
            ));
            txt.add(Line(
                "simulation model, and a deeply flawed understanding of how much ",
            ));
            txt.add(Line(
                "articulated buses can bend around tight corners. Use this as a ",
            ));
            txt.add(Line(
                "conversation starter with your city government, not a final ",
            ));
            txt.add(Line(
                "decision maker. Any resemblance of in-game characters to real ",
            ));
            txt.add(Line(
                "people is probably coincidental, except for Pedestrian #42.",
            ));
            txt.add(Line("Have the appropriate amount of fun."));
            txt.draw(ctx).centered_horiz().align_vert_center()
        },
    ];

    ManagedGUIState::fullscreen(
        WrappedComposite::new(
            Composite::new(Widget::col(col))
                .exact_size_percent(90, 85)
                .build(ctx),
        )
        .cb("back", Box::new(|_, _| Some(Transition::Pop))),
    )
}

fn proposals_picker(ctx: &mut EventCtx) -> Box<dyn State> {
    let mut cbs: Vec<(String, Callback)> = Vec::new();
    let mut buttons: Vec<Widget> = Vec::new();
    for map_name in abstutil::list_all_objects(abstutil::path_all_maps()) {
        for (_, edits) in
            abstutil::load_all_objects::<MapEdits>(abstutil::path_all_edits(&map_name))
        {
            if !edits.proposal_description.is_empty() {
                let mut txt = Text::new();
                for l in &edits.proposal_description {
                    txt.add(Line(l));
                }
                let path = abstutil::path_edits(&edits.map_name, &edits.edits_name);
                buttons.push(Btn::custom_text_fg(txt).build(ctx, &path, None));
                cbs.push((
                    path,
                    Box::new(move |ctx, app| {
                        // TODO apply edits
                        Some(Transition::Push(Box::new(SandboxMode::new(
                            ctx,
                            app,
                            GameplayMode::PlayScenario(
                                abstutil::path_map(&edits.map_name),
                                "weekday".to_string(),
                            ),
                        ))))
                    }),
                ));
            }
        }
    }

    let mut c = WrappedComposite::new(
        Composite::new(
            Widget::col(vec![
                Btn::svg_def("../data/system/assets/pregame/back.svg")
                    .build(ctx, "back", hotkey(Key::Escape))
                    .align_left(),
                {
                    let mut txt = Text::from(Line("A/B STREET").display_title());
                    txt.add(Line("PROPOSALS").big_heading_styled());
                    txt.add(Line(""));
                    txt.add(Line(
                        "These are proposed changes to Seattle made by community members.",
                    ));
                    txt.add(Line("Contact dabreegster@gmail.com to add your idea here!"));
                    txt.draw(ctx).centered_horiz().bg(colors::PANEL_BG)
                },
                Widget::row(buttons)
                    .flex_wrap(ctx, 80)
                    .bg(colors::PANEL_BG)
                    .padding(10),
            ])
            .evenly_spaced(),
        )
        .exact_size_percent(90, 85)
        .build(ctx),
    )
    .cb("back", Box::new(|_, _| Some(Transition::Pop)));
    for (name, cb) in cbs {
        c = c.cb(&name, cb);
    }
    ManagedGUIState::fullscreen(c)
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
        if ctx.input.nonblocking_is_update_event().is_some() {
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

#[cfg(not(target_arch = "wasm32"))]
#[allow(unused)]
mod built_info {
    use ezgui::{Color, Line, Text};

    include!(concat!(env!("OUT_DIR"), "/built.rs"));

    pub fn time() -> Text {
        let t = built::util::strptime(BUILT_TIME_UTC);

        let mut txt = Text::from(Line(format!("Built on {}", t.date().naive_local())));
        // Releases every Sunday
        if (chrono::Utc::now() - t).num_days() > 8 {
            txt.append(Line(format!(" (get the new release from abstreet.org)")).fg(Color::RED));
        }
        txt
    }
}

#[cfg(target_arch = "wasm32")]
mod built_info {
    pub fn time() -> ezgui::Text {
        ezgui::Text::new()
    }
}
