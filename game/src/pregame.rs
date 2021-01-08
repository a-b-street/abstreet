use std::collections::HashMap;

use instant::Instant;
use rand::Rng;
use rand_xorshift::XorShiftRng;

use abstutil::Timer;
use geom::{Duration, Line, Percent, Pt2D, Speed};
use map_gui::load::MapLoader;
use map_gui::tools::{open_browser, PopupMsg};
use map_model::PermanentMapEdits;
use sim::{AlertHandler, ScenarioGenerator, Sim, SimOptions};
use widgetry::{
    hotkeys, Btn, Color, DrawBaselayer, EventCtx, GfxCtx, Key, Line, Outcome, Panel, RewriteColor,
    State, Text, UpdateType, Widget,
};

use crate::app::{App, Transition};
use crate::challenges::ChallengesPicker;
use crate::devtools::DevToolsMode;
use crate::edit::apply_map_edits;
use crate::sandbox::gameplay::Tutorial;
use crate::sandbox::{GameplayMode, SandboxMode};

pub struct TitleScreen {
    panel: Panel,
    screensaver: Screensaver,
    rng: XorShiftRng,
}

impl TitleScreen {
    pub fn new(ctx: &mut EventCtx, app: &mut App) -> TitleScreen {
        let mut rng = app.primary.current_flags.sim_flags.make_rng();
        let mut timer = Timer::new("screensaver traffic");
        let mut opts = SimOptions::new("screensaver");
        opts.alerts = AlertHandler::Silence;
        app.primary.sim = Sim::new(&app.primary.map, opts, &mut timer);
        ScenarioGenerator::small_run(&app.primary.map)
            .generate(&app.primary.map, &mut rng, &mut timer)
            .instantiate(&mut app.primary.sim, &app.primary.map, &mut rng, &mut timer);

        TitleScreen {
            panel: Panel::new(
                Widget::col(vec![
                    Widget::draw_svg(ctx, "system/assets/pregame/logo.svg"),
                    // TODO that nicer font
                    // TODO Any key
                    Btn::text_bg2("PLAY").build(
                        ctx,
                        "start game",
                        hotkeys(vec![Key::Space, Key::Enter]),
                    ),
                ])
                .bg(app.cs.dialog_bg)
                .padding(16)
                .outline(3.0, Color::BLACK)
                .centered(),
            )
            .build_custom(ctx),
            screensaver: Screensaver::bounce(ctx, app, &mut rng),
            rng,
        }
    }
}

impl State<App> for TitleScreen {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "start game" => {
                    app.primary.clear_sim();
                    return Transition::Replace(MainMenu::new(ctx, app));
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        self.screensaver.update(&mut self.rng, ctx, app);
        ctx.request_update(UpdateType::Game);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}

pub struct MainMenu {
    panel: Panel,
}

impl MainMenu {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let col = vec![
            Btn::svg_def("system/assets/pregame/quit.svg")
                .build(ctx, "quit", Key::Escape)
                .align_left(),
            {
                let mut txt = Text::from(Line("A/B STREET").display_title());
                txt.add(Line("Created by Dustin Carlino, Yuwen Li, & Michael Kirk"));
                txt.draw(ctx).centered_horiz()
            },
            Widget::row(vec![
                Btn::svg(
                    "system/assets/pregame/tutorial.svg",
                    RewriteColor::Change(Color::WHITE, app.cs.hovering),
                )
                .tooltip({
                    let mut txt = Text::tooltip(ctx, Key::T, "Tutorial");
                    txt.add(Line("Learn how to play the game").small());
                    txt
                })
                .build(ctx, "Tutorial", Key::T),
                Btn::svg(
                    "system/assets/pregame/sandbox.svg",
                    RewriteColor::Change(Color::WHITE, app.cs.hovering),
                )
                .tooltip({
                    let mut txt = Text::tooltip(ctx, Key::S, "Sandbox");
                    txt.add(Line("No goals, try out any idea here").small());
                    txt
                })
                .build(ctx, "Sandbox mode", Key::S),
                Btn::svg(
                    "system/assets/pregame/challenges.svg",
                    RewriteColor::Change(Color::WHITE, app.cs.hovering),
                )
                .tooltip({
                    let mut txt = Text::tooltip(ctx, Key::C, "Challenges");
                    txt.add(Line("Fix specific problems").small());
                    txt
                })
                .build(ctx, "Challenges", Key::C),
            ])
            .centered(),
            Widget::row(vec![
                Btn::text_bg2("Community Proposals")
                    .tooltip({
                        let mut txt = Text::tooltip(ctx, Key::P, "Community Proposals");
                        txt.add(Line("See existing ideas for improving traffic").small());
                        txt
                    })
                    .build_def(ctx, Key::P),
                Btn::text_bg2("Internal Dev Tools").build_def(ctx, Key::D),
            ])
            .centered(),
            Widget::col(vec![
                Widget::row(vec![
                    Btn::text_bg2("About").build_def(ctx, None),
                    Btn::text_bg2("Feedback").build_def(ctx, None),
                ]),
                built_info::time().draw(ctx),
            ])
            .centered(),
        ];

        Box::new(MainMenu {
            panel: Panel::new(Widget::col(col).evenly_spaced())
                .exact_size_percent(90, 85)
                .build_custom(ctx),
        })
    }
}

impl State<App> for MainMenu {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "quit" => {
                    return Transition::Pop;
                }
                "Tutorial" => {
                    return Tutorial::start(ctx, app);
                }
                "Sandbox mode" => {
                    let scenario = if abstio::file_exists(abstio::path_scenario(
                        app.primary.map.get_name(),
                        "weekday",
                    )) {
                        "weekday"
                    } else {
                        "home_to_work"
                    };
                    return Transition::Push(SandboxMode::simple_new(
                        ctx,
                        app,
                        GameplayMode::PlayScenario(
                            app.primary.map.get_name().clone(),
                            scenario.to_string(),
                            Vec::new(),
                        ),
                    ));
                }
                "Challenges" => {
                    return Transition::Push(ChallengesPicker::new(ctx, app));
                }
                "About" => {
                    return Transition::Push(About::new(ctx, app));
                }
                "Feedback" => {
                    open_browser("https://forms.gle/ocvbek1bTaZUr3k49".to_string());
                }
                "Community Proposals" => {
                    return Transition::Push(Proposals::new(ctx, app, None));
                }
                "Internal Dev Tools" => {
                    return Transition::Push(DevToolsMode::new(ctx, app));
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
    }
}

struct About {
    panel: Panel,
}

impl About {
    fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let col = vec![
            Btn::svg_def("system/assets/pregame/back.svg")
                .build(ctx, "back", Key::Escape)
                .align_left(),
            {
                Text::from_multiline(vec![
                    Line("A/B STREET").display_title(),
                    Line("Created by Dustin Carlino, Yuwen Li, & Michael Kirk"),
                    Line("Character art by Holly Hansel"),
                    Line(""),
                    Line(
                        "Data from OpenStreetMap, King County GIS, and Puget Sound Regional \
                         Council",
                    ),
                    Line(""),
                    Line(
                        "Disclaimer: This game is based on imperfect data, heuristics concocted \
                         under the influence of cold brew, a simplified traffic simulation model, \
                         and a deeply flawed understanding of how much articulated buses can bend \
                         around tight corners. Use this as a conversation starter with your city \
                         government, not a final decision maker. Any resemblance of in-game \
                         characters to real people is probably coincidental, unless of course you \
                         stumble across the elusive \"Dustin Bikelino\". Have the appropriate \
                         amount of fun.",
                    ),
                ])
                .wrap_to_pct(ctx, 50)
                .draw(ctx)
                .centered_horiz()
                .align_vert_center()
                .bg(app.cs.panel_bg)
                .padding(16)
            },
            Btn::text_bg2("See full credits")
                .build_def(ctx, None)
                .centered_horiz(),
        ];

        Box::new(About {
            panel: Panel::new(Widget::custom_col(col))
                .exact_size_percent(90, 85)
                .build_custom(ctx),
        })
    }
}

impl State<App> for About {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "back" => {
                    return Transition::Pop;
                }
                "See full credits" => {
                    open_browser("https://github.com/dabreegster/abstreet#credits".to_string());
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
    }
}

struct Proposals {
    panel: Panel,
    proposals: HashMap<String, PermanentMapEdits>,
    current: Option<String>,
}

impl Proposals {
    fn new(ctx: &mut EventCtx, app: &App, current: Option<String>) -> Box<dyn State<App>> {
        let mut proposals = HashMap::new();
        let mut buttons = Vec::new();
        let mut current_tab = Vec::new();
        // If a proposal has fallen out of date, it'll be skipped with an error logged. Since these
        // are under version control, much more likely to notice when they break (or we could add a
        // step to data/regen.sh).
        for (name, edits) in
            abstio::load_all_objects::<PermanentMapEdits>(abstio::path("system/proposals"))
        {
            if current == Some(name.clone()) {
                let mut txt = Text::new();
                txt.add(Line(&edits.proposal_description[0]).small_heading());
                for l in edits.proposal_description.iter().skip(1) {
                    txt.add(Line(l));
                }
                current_tab.push(
                    txt.wrap_to_pct(ctx, 70)
                        .draw(ctx)
                        .margin_below(15)
                        .margin_above(15),
                );

                if edits.proposal_link.is_some() {
                    current_tab.push(
                        Btn::text_bg2("Read detailed write-up")
                            .build_def(ctx, None)
                            .margin_below(10),
                    );
                }
                current_tab.push(Btn::text_bg2("Try out this proposal").build_def(ctx, None));

                buttons.push(Btn::text_bg2(&edits.proposal_description[0]).inactive(ctx));
            } else {
                buttons.push(
                    Btn::text_bg2(&edits.proposal_description[0])
                        .no_tooltip()
                        .build(ctx, &name, None)
                        .margin_below(10),
                );
            }

            proposals.insert(name, edits);
        }

        let mut col = vec![
            {
                let mut txt = Text::from(Line("A/B STREET").display_title());
                txt.add(Line("PROPOSALS").big_heading_styled());
                txt.add(Line(""));
                txt.add(Line(
                    "These are proposed changes to Seattle made by community members.",
                ));
                txt.add(Line("Contact dabreegster@gmail.com to add your idea here!"));
                txt.draw(ctx).centered_horiz().margin_below(20)
            },
            Widget::custom_row(buttons).flex_wrap(ctx, Percent::int(80)),
        ];
        col.extend(current_tab);

        Box::new(Proposals {
            proposals,
            panel: Panel::new(Widget::custom_col(vec![
                Btn::svg_def("system/assets/pregame/back.svg")
                    .build(ctx, "back", Key::Escape)
                    .align_left()
                    .margin_below(20),
                Widget::col(col).bg(app.cs.panel_bg).padding(16),
            ]))
            .exact_size_percent(90, 85)
            .build_custom(ctx),
            current,
        })
    }
}

impl State<App> for Proposals {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "back" => {
                    return Transition::Pop;
                }
                "Try out this proposal" => {
                    let edits = self.proposals[self.current.as_ref().unwrap()].clone();

                    return Transition::Push(MapLoader::new(
                        ctx,
                        app,
                        edits.map_name.clone(),
                        Box::new(move |ctx, app| {
                            // Apply edits before setting up the sandbox, for simplicity
                            let maybe_err = ctx.loading_screen("apply edits", |ctx, mut timer| {
                                match edits.to_edits(&app.primary.map) {
                                    Ok(edits) => {
                                        apply_map_edits(ctx, app, edits);
                                        app.primary
                                            .map
                                            .recalculate_pathfinding_after_edits(&mut timer);
                                        None
                                    }
                                    Err(err) => Some(err),
                                }
                            });
                            if let Some(err) = maybe_err {
                                Transition::Replace(PopupMsg::new(
                                    ctx,
                                    "Can't load proposal",
                                    vec![err.to_string()],
                                ))
                            } else {
                                app.primary.layer =
                                    Some(Box::new(crate::layer::map::Static::edits(ctx, app)));
                                Transition::Replace(SandboxMode::simple_new(
                                    ctx,
                                    app,
                                    GameplayMode::PlayScenario(
                                        app.primary.map.get_name().clone(),
                                        "weekday".to_string(),
                                        Vec::new(),
                                    ),
                                ))
                            }
                        }),
                    ));
                }
                "Read detailed write-up" => {
                    open_browser(
                        self.proposals[self.current.as_ref().unwrap()]
                            .proposal_link
                            .clone()
                            .unwrap(),
                    );
                }
                x => {
                    return Transition::Replace(Proposals::new(ctx, app, Some(x.to_string())));
                }
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
    }
}

struct Screensaver {
    line: Line,
    started: Instant,
}

impl Screensaver {
    fn bounce(ctx: &mut EventCtx, app: &mut App, rng: &mut XorShiftRng) -> Screensaver {
        let at = ctx.canvas.center_to_map_pt();
        let bounds = app.primary.map.get_bounds();
        let line = loop {
            let goto = Pt2D::new(
                rng.gen_range(0.0..bounds.max_x),
                rng.gen_range(0.0..bounds.max_y),
            );
            if let Some(l) = Line::new(at, goto) {
                break l;
            }
        };
        ctx.canvas.cam_zoom = 10.0;

        Screensaver {
            line,
            started: Instant::now(),
        }
    }

    fn update(&mut self, rng: &mut XorShiftRng, ctx: &mut EventCtx, app: &mut App) {
        const SIM_SPEED: f64 = 3.0;
        const PAN_SPEED: Speed = Speed::const_meters_per_second(20.0);

        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();
            if let Some(pt) = self
                .line
                .dist_along(Duration::realtime_elapsed(self.started) * PAN_SPEED)
            {
                ctx.canvas.center_on_map_pt(pt);
            } else {
                *self = Screensaver::bounce(ctx, app, rng);
            }
            app.primary.sim.time_limited_step(
                &app.primary.map,
                SIM_SPEED * dt,
                Duration::seconds(0.033),
                &mut app.primary.sim_cb,
            );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(unused)]
mod built_info {
    use widgetry::{Color, DrawBaselayer, Line, State, Text};

    include!(concat!(env!("OUT_DIR"), "/built.rs"));

    pub fn time() -> Text {
        let t = built::util::strptime(BUILT_TIME_UTC);

        let mut txt = Text::from(Line(format!(
            "This version built on {}",
            t.date().naive_local()
        )));
        // Releases every Sunday
        if (chrono::Utc::now() - t).num_days() > 8 {
            txt.append(Line(format!(" (get the new release from abstreet.org)")).fg(Color::RED));
        }
        txt
    }
}

#[cfg(target_arch = "wasm32")]
mod built_info {
    pub fn time() -> widgetry::Text {
        widgetry::Text::new()
    }
}
