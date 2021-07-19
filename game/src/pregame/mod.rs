use instant::Instant;
use rand::Rng;
use rand_xorshift::XorShiftRng;

use abstio::{CityName, MapName};
use abstutil::Timer;
use geom::{Duration, Line, Pt2D, Speed};
use map_gui::tools::open_browser;
use sim::{AlertHandler, ScenarioGenerator, Sim, SimOptions};
use widgetry::{
    hotkeys, ButtonStyle, Color, ContentMode, DrawBaselayer, EdgeInsets, EventCtx, Font, GfxCtx,
    Image, Key, Line, Outcome, Panel, ScreenDims, State, Text, UpdateType, Widget,
};

use crate::app::{App, Transition};
use crate::challenges::ChallengesPicker;
use crate::devtools::DevToolsMode;
use crate::sandbox::gameplay::Tutorial;
use crate::sandbox::{GameplayMode, SandboxMode};

mod proposals;

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
        app.primary.sim = Sim::new(&app.primary.map, opts);
        ScenarioGenerator::small_run(&app.primary.map)
            .generate(&app.primary.map, &mut rng, &mut timer)
            .instantiate(&mut app.primary.sim, &app.primary.map, &mut rng, &mut timer);

        TitleScreen {
            panel: Panel::new_builder(
                Widget::col(vec![
                    Image::untinted("system/assets/pregame/logo.svg").into_widget(ctx),
                    // TODO that nicer font
                    // TODO Any key
                    ctx.style()
                        .btn_solid_primary
                        .text("Play")
                        .hotkey(hotkeys(vec![Key::Space, Key::Enter]))
                        .build_widget(ctx, "start game"),
                ])
                .bg(app.cs.dialog_bg)
                .padding(16)
                .outline((3.0, Color::BLACK))
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
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "start game" => {
                    app.primary.clear_sim();
                    return Transition::Replace(MainMenu::new_state(ctx));
                }
                _ => unreachable!(),
            }
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
    pub fn new_state(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let col = vec![
            {
                let mut txt = Text::from(Line("A/B STREET").display_title());
                txt.add_line("Created by Dustin Carlino, Yuwen Li, & Michael Kirk");
                txt.into_widget(ctx).centered_horiz()
            },
            Widget::row({
                let btn_builder = ButtonStyle::solid_dark_fg()
                    .btn()
                    .image_dims(ScreenDims::new(200.0, 100.0))
                    .font_size(40)
                    .font(Font::OverpassBold)
                    // CLEANUP: There's some baked in padding with our current assets which is
                    // probably not desirable, but we compensate for that here by applying
                    // unequal padding
                    .padding(EdgeInsets {
                        top: 40.0,
                        bottom: 40.0,
                        left: 20.0,
                        right: 20.0,
                    })
                    .image_content_mode(ContentMode::ScaleAspectFill)
                    .vertical();
                vec![
                    btn_builder
                        .clone()
                        .image_path("system/assets/pregame/tutorial.svg")
                        .label_text("Tutorial")
                        .tooltip({
                            let mut txt = Text::tooltip(ctx, Key::T, "Tutorial");
                            txt.add_line(Line("Learn how to play the game").small());
                            txt
                        })
                        .hotkey(Key::T)
                        .build_widget(ctx, "Tutorial"),
                    btn_builder
                        .clone()
                        .image_path("system/assets/pregame/sandbox.svg")
                        .label_text("Sandbox")
                        .tooltip({
                            let mut txt = Text::tooltip(ctx, Key::S, "Sandbox");
                            txt.add_line(Line("No goals, try out any idea here").small());
                            txt
                        })
                        .hotkey(Key::S)
                        .build_widget(ctx, "Sandbox mode"),
                    btn_builder
                        .clone()
                        .image_path("system/assets/pregame/challenges.svg")
                        .label_text("Challenge")
                        .tooltip({
                            let mut txt = Text::tooltip(ctx, Key::C, "Challenges");
                            txt.add_line(Line("Fix specific problems").small());
                            txt
                        })
                        .hotkey(Key::C)
                        .build_widget(ctx, "Challenges"),
                ]
            })
            .centered(),
            Widget::row(vec![
                ctx.style()
                    .btn_outline
                    .text("Community Proposals")
                    .tooltip({
                        let mut txt = Text::tooltip(ctx, Key::P, "Community Proposals");
                        txt.add_line(Line("See existing ideas for improving traffic").small());
                        txt
                    })
                    .hotkey(Key::P)
                    .build_widget(ctx, "Community Proposals"),
                ctx.style()
                    .btn_outline
                    .text("Internal Dev Tools")
                    .hotkey(Key::D)
                    .build_widget(ctx, "Internal Dev Tools"),
            ])
            .centered(),
            Widget::col(vec![
                Widget::row(vec![
                    ctx.style().btn_outline.text("About").build_def(ctx),
                    ctx.style().btn_outline.text("Feedback").build_def(ctx),
                ]),
                built_info::maybe_update(ctx),
            ])
            .centered(),
        ];

        Box::new(MainMenu {
            panel: Panel::new_builder(Widget::col(col).evenly_spaced())
                .exact_size_percent(90, 85)
                .build_custom(ctx),
        })
    }
}

impl State<App> for MainMenu {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Tutorial" => {
                    return Tutorial::start(ctx, app);
                }
                "Sandbox mode" => {
                    return Transition::Push(SandboxMode::simple_new(
                        app,
                        GameplayMode::PlayScenario(
                            app.primary.map.get_name().clone(),
                            default_scenario_for_map(app.primary.map.get_name()),
                            Vec::new(),
                        ),
                    ));
                }
                "Challenges" => {
                    return Transition::Push(ChallengesPicker::new_state(ctx, app));
                }
                "About" => {
                    return Transition::Push(About::new_state(ctx, app));
                }
                "Feedback" => {
                    open_browser("https://forms.gle/ocvbek1bTaZUr3k49");
                }
                "Community Proposals" => {
                    return Transition::Push(proposals::Proposals::new_state(ctx, app, None));
                }
                "Internal Dev Tools" => {
                    return Transition::Push(DevToolsMode::new_state(ctx, app));
                }
                "Download the new release" => {
                    open_browser("https://github.com/a-b-street/abstreet/releases");
                }
                _ => unreachable!(),
            }
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
    fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let col = vec![
            ctx.style()
                .btn_back("Home")
                .hotkey(Key::Escape)
                .build_widget(ctx, "back")
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
                .into_widget(ctx)
                .centered_horiz()
                .align_vert_center()
                .bg(app.cs.panel_bg)
                .padding(16)
            },
            ctx.style()
                .btn_outline
                .text("See full credits")
                .build_def(ctx)
                .centered_horiz(),
        ];

        Box::new(About {
            panel: Panel::new_builder(Widget::custom_col(col))
                .exact_size_percent(90, 85)
                .build_custom(ctx),
        })
    }
}

impl State<App> for About {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "back" => {
                    return Transition::Pop;
                }
                "See full credits" => {
                    open_browser("https://github.com/a-b-street/abstreet#credits");
                }
                _ => unreachable!(),
            }
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
                &mut app.primary.map,
                SIM_SPEED * dt,
                Duration::seconds(0.033),
                &mut app.primary.sim_cb,
            );
        }
    }
}

fn default_scenario_for_map(name: &MapName) -> String {
    if name.city == CityName::seattle()
        && abstio::file_exists(abstio::path_scenario(name, "weekday"))
    {
        return "weekday".to_string();
    }
    if name.city.country == "gb" {
        for x in ["background", "base_with_bg"] {
            if abstio::file_exists(abstio::path_scenario(name, x)) {
                return x.to_string();
            }
        }
    }
    "home_to_work".to_string()
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(unused)]
mod built_info {
    use super::*;

    include!(concat!(env!("OUT_DIR"), "/built.rs"));

    pub fn maybe_update(ctx: &mut EventCtx) -> Widget {
        let t = built::util::strptime(BUILT_TIME_UTC);

        let txt = Text::from(format!("This version built on {}", t.date().naive_local()))
            .into_widget(ctx);
        // Releases every Sunday... but sometimes we miss a week
        if (chrono::Utc::now() - t).num_days() > 15 {
            Widget::row(vec![
                txt.centered_vert(),
                ctx.style()
                    .btn_outline
                    .text("Download the new release")
                    .build_def(ctx),
            ])
        } else {
            txt
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod built_info {
    use super::*;

    pub fn maybe_update(_: &mut EventCtx) -> Widget {
        Widget::nothing()
    }
}
