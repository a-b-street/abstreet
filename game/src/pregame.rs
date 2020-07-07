use crate::app::App;
use crate::challenges::challenges_picker;
use crate::devtools::DevToolsMode;
use crate::edit::apply_map_edits;
use crate::game::{msg, DrawBaselayer, State, Transition};
use crate::sandbox::gameplay::Tutorial;
use crate::sandbox::{GameplayMode, SandboxMode};
use ezgui::{
    hotkey, hotkeys, Btn, Color, Composite, EventCtx, GfxCtx, Key, Line, Outcome, RewriteColor,
    Text, UpdateType, Widget,
};
use geom::{Duration, Line, Pt2D, Speed};
use instant::Instant;
use map_model::{Map, PermanentMapEdits};
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::collections::HashMap;

pub struct TitleScreen {
    composite: Composite,
    screensaver: Screensaver,
    rng: XorShiftRng,
}

impl TitleScreen {
    pub fn new(ctx: &mut EventCtx, app: &App) -> TitleScreen {
        let mut rng = app.primary.current_flags.sim_flags.make_rng();
        TitleScreen {
            composite: Composite::new(
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
                .bg(app.cs.grass)
                .padding(16)
                .outline(3.0, Color::BLACK)
                .centered(),
            )
            .build_custom(ctx),
            screensaver: Screensaver::start_bounce(&mut rng, ctx, &app.primary.map),
            rng,
        }
    }
}

impl State for TitleScreen {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "start game" => {
                    return Transition::Replace(MainMenu::new(ctx, app));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        self.screensaver
            .update(&mut self.rng, ctx, &app.primary.map);
        ctx.request_update(UpdateType::Game);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
    }
}

pub struct MainMenu {
    composite: Composite,
}

impl MainMenu {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let col = vec![
            Btn::svg_def("system/assets/pregame/quit.svg")
                .build(ctx, "quit", hotkey(Key::Escape))
                .align_left(),
            {
                let mut txt = Text::from(Line("A/B STREET").display_title());
                txt.add(Line("Created by Dustin Carlino and Yuwen Li"));
                txt.draw(ctx).centered_horiz()
            },
            Widget::row(vec![
                Btn::svg(
                    "system/assets/pregame/tutorial.svg",
                    RewriteColor::Change(Color::WHITE, app.cs.hovering),
                )
                .tooltip({
                    let mut txt = Text::tooltip(ctx, hotkey(Key::T), "Tutorial");
                    txt.add(Line("Learn how to play the game").small());
                    txt
                })
                .build(ctx, "Tutorial", hotkey(Key::T)),
                Btn::svg(
                    "system/assets/pregame/sandbox.svg",
                    RewriteColor::Change(Color::WHITE, app.cs.hovering),
                )
                .tooltip({
                    let mut txt = Text::tooltip(ctx, hotkey(Key::S), "Sandbox");
                    txt.add(Line("No goals, try out any idea here").small());
                    txt
                })
                .build(ctx, "Sandbox mode", hotkey(Key::S)),
                Btn::svg(
                    "system/assets/pregame/challenges.svg",
                    RewriteColor::Change(Color::WHITE, app.cs.hovering),
                )
                .tooltip({
                    let mut txt = Text::tooltip(ctx, hotkey(Key::C), "Challenges");
                    txt.add(Line("Fix specific problems").small());
                    txt
                })
                .build(ctx, "Challenges", hotkey(Key::C)),
            ])
            .centered(),
            Widget::row(vec![
                Btn::text_bg2("Community Proposals")
                    .tooltip({
                        let mut txt = Text::tooltip(ctx, hotkey(Key::P), "Community Proposals");
                        txt.add(Line("See existing ideas for improving traffic").small());
                        txt
                    })
                    .build_def(ctx, hotkey(Key::P)),
                Btn::text_bg2("Contribute parking data to OpenStreetMap")
                    .tooltip({
                        let mut txt = Text::tooltip(
                            ctx,
                            hotkey(Key::M),
                            "Contribute parking data to OpenStreetMap",
                        );
                        txt.add(Line("Improve parking data in OpenStreetMap").small());
                        txt
                    })
                    .build_def(ctx, hotkey(Key::M)),
                Btn::text_bg2("Internal Dev Tools").build_def(ctx, hotkey(Key::D)),
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
            composite: Composite::new(Widget::col(col).evenly_spaced())
                .exact_size_percent(90, 85)
                .build_custom(ctx),
        })
    }
}

impl State for MainMenu {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "quit" => {
                    // TODO before_quit?
                    std::process::exit(0);
                }
                "Tutorial" => {
                    return Tutorial::start(ctx, app);
                }
                "Sandbox mode" => {
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
                    return Transition::Push(Box::new(SandboxMode::new(
                        ctx,
                        app,
                        GameplayMode::PlayScenario(map_path, scenario.to_string(), Vec::new()),
                    )));
                }
                "Challenges" => {
                    return Transition::Push(challenges_picker(ctx, app));
                }
                "About" => {
                    return Transition::Push(About::new(ctx, app));
                }
                "Feedback" => {
                    // cargo fmt tries to remove this
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let _ = webbrowser::open("https://forms.gle/ocvbek1bTaZUr3k49");
                    }
                }
                "Community Proposals" => {
                    return Transition::Push(Proposals::new(ctx, app, None));
                }
                "Contribute parking data to OpenStreetMap" => {
                    return Transition::Push(crate::devtools::mapping::ParkingMapper::new(
                        ctx, app,
                    ));
                }
                "Internal Dev Tools" => {
                    return Transition::Push(DevToolsMode::new(ctx, app));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.grass);
        self.composite.draw(g);
    }
}

struct About {
    composite: Composite,
}

impl About {
    fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let col = vec![
            Btn::svg_def("system/assets/pregame/back.svg")
                .build(ctx, "back", hotkey(Key::Escape))
                .align_left(),
            {
                Text::from_multiline(vec![
                    Line("A/B STREET").display_title(),
                    Line("Created by Dustin Carlino, UX by Yuwen Li"),
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
            composite: Composite::new(Widget::custom_col(col))
                .exact_size_percent(90, 85)
                .build_custom(ctx),
        })
    }
}

impl State for About {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "back" => {
                    return Transition::Pop;
                }
                "See full credits" => {
                    // cargo fmt tries to remove this
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let _ = webbrowser::open("https://github.com/dabreegster/abstreet#credits");
                    }
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.grass);
        self.composite.draw(g);
    }
}

struct Proposals {
    composite: Composite,
    proposals: HashMap<String, PermanentMapEdits>,
    current: Option<String>,
}

impl Proposals {
    fn new(ctx: &mut EventCtx, app: &App, current: Option<String>) -> Box<dyn State> {
        let mut proposals = HashMap::new();
        let mut buttons = Vec::new();
        let mut current_tab = Vec::new();
        for (name, edits) in
            abstutil::load_all_objects::<PermanentMapEdits>("system/proposals".to_string())
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
                        .tooltip(Text::new())
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
            Widget::custom_row(buttons).flex_wrap(ctx, 80),
        ];
        col.extend(current_tab);

        Box::new(Proposals {
            proposals,
            composite: Composite::new(Widget::custom_col(vec![
                Btn::svg_def("system/assets/pregame/back.svg")
                    .build(ctx, "back", hotkey(Key::Escape))
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

impl State for Proposals {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "back" => {
                    return Transition::Pop;
                }
                "Try out this proposal" => {
                    let edits = &self.proposals[self.current.as_ref().unwrap()];
                    // Apply edits before setting up the sandbox, for simplicity
                    let map_name = edits.map_name.clone();
                    let edits = edits.clone();
                    let maybe_err = ctx.loading_screen("apply edits", |ctx, mut timer| {
                        if &edits.map_name != app.primary.map.get_name() {
                            app.switch_map(ctx, abstutil::path_map(&edits.map_name));
                        }
                        match PermanentMapEdits::from_permanent(edits, &app.primary.map) {
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
                        return Transition::Push(msg("Can't load proposal", vec![err]));
                    } else {
                        app.layer = Some(Box::new(crate::layer::map::Static::edits(ctx, app)));
                        return Transition::Push(Box::new(SandboxMode::new(
                            ctx,
                            app,
                            GameplayMode::PlayScenario(
                                abstutil::path_map(&map_name),
                                "weekday".to_string(),
                                Vec::new(),
                            ),
                        )));
                    }
                }
                "Read detailed write-up" => {
                    let link = self.proposals[self.current.as_ref().unwrap()]
                        .proposal_link
                        .as_ref()
                        .unwrap();
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let _ = webbrowser::open(link);
                    }
                }
                x => {
                    return Transition::Replace(Proposals::new(ctx, app, Some(x.to_string())));
                }
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.grass);
        self.composite.draw(g);
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
    pub fn time() -> ezgui::Text {
        ezgui::Text::new()
    }
}
