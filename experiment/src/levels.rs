use abstutil::MapName;
use geom::{Duration, Speed};
use map_gui::SimpleApp;
use map_model::osm;
use widgetry::{
    Btn, DrawBaselayer, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, Text, Transition,
    Widget,
};

pub struct Config {
    pub title: &'static str,
    pub map: MapName,
    pub start_depot: osm::OsmID,
    pub minimap_zoom: usize,

    pub normal_speed: Speed,
    pub tired_speed: Speed,
    pub recharge_rate: f64,
    pub max_energy: Duration,
    pub upzone_rate: usize,
}

// TODO Like Challenge::all; cache with lazy static?
fn all_levels() -> Vec<Config> {
    vec![
        Config {
            title: "Level 1",
            map: MapName::seattle("montlake"),
            start_depot: osm::OsmID::Way(osm::WayID(217700589)),
            minimap_zoom: 0,

            normal_speed: Speed::miles_per_hour(30.0),
            tired_speed: Speed::miles_per_hour(10.0),
            recharge_rate: 1000.0,
            max_energy: Duration::minutes(90),
            upzone_rate: 30_000,
        },
        Config {
            title: "Level 2 - Magnolia",
            map: MapName::seattle("ballard"),
            start_depot: osm::OsmID::Way(osm::WayID(38655876)),
            minimap_zoom: 2,

            normal_speed: Speed::miles_per_hour(40.0),
            tired_speed: Speed::miles_per_hour(15.0),
            recharge_rate: 2000.0,
            max_energy: Duration::minutes(120),
            upzone_rate: 30_000,
        },
    ]
}

pub struct TitleScreen {
    panel: Panel,
}

impl TitleScreen {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn State<SimpleApp>> {
        let levels = all_levels();

        Box::new(TitleScreen {
            panel: Panel::new(
                Widget::col(vec![
                    Btn::svg_def("system/assets/pregame/quit.svg")
                        .build(ctx, "quit", Key::Escape)
                        .align_left(),
                    {
                        let mut txt = Text::from(Line("15 minute Santa").display_title());
                        txt.add(Line("An experiment"));
                        txt.draw(ctx).centered_horiz()
                    },
                    Widget::row(
                        levels
                            .into_iter()
                            .map(|lvl| Btn::text_bg2(lvl.title).build_def(ctx, None))
                            .collect(),
                    ),
                ])
                .evenly_spaced(),
            )
            .exact_size_percent(90, 85)
            .build_custom(ctx),
        })
    }
}

impl State<SimpleApp> for TitleScreen {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut SimpleApp) -> Transition<SimpleApp> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "quit" => {
                    std::process::exit(0);
                }
                x => {
                    for lvl in all_levels() {
                        if x == lvl.title {
                            return Transition::Push(crate::game::Game::new(ctx, app, lvl));
                        }
                    }
                    panic!("Unknown action {}", x);
                }
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &SimpleApp) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
    }
}
