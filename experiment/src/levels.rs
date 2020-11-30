use abstutil::MapName;
use geom::{Duration, Speed};
use map_gui::tools::PopupMsg;
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
            title: "Level 1 - a small neighborhood",
            map: MapName::seattle("montlake"),
            start_depot: osm::OsmID::Way(osm::WayID(217700589)),
            minimap_zoom: 0,

            normal_speed: Speed::miles_per_hour(30.0),
            tired_speed: Speed::miles_per_hour(10.0),
            recharge_rate: 2000.0,
            max_energy: Duration::hours(3),
            upzone_rate: 100,
        },
        Config {
            title: "Level 2 - Magnolia",
            map: MapName::seattle("ballard"),
            start_depot: osm::OsmID::Way(osm::WayID(38655876)),
            minimap_zoom: 2,

            normal_speed: Speed::miles_per_hour(40.0),
            tired_speed: Speed::miles_per_hour(15.0),
            recharge_rate: 3000.0,
            max_energy: Duration::hours(3),
            upzone_rate: 150,
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
                    Btn::text_bg2("Instructions").build_def(ctx, None),
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
                "Instructions" => {
                    // TODO As I'm writing the range argument, I don't buy the hybrid motor.
                    // Wireless Tesla energy instead?
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "Instructions",
                        vec![
                            "It's Christmas Eve, so it's time for Santa to deliver presents in \
                             Seattle. 2020 has thoroughly squashed any remaining magic out of the \
                             world, and so your reindeer can't fly.",
                            "Luckily, Amazon sells a hybrid diesel/electric sleigh. It runs out \
                             of energy the farther away from your charging station you get.",
                            "You can only recharge on commercial buildings, where you won't wake \
                             up any sleeping children.",
                            "",
                            "Use the arrow keys to move around.",
                            "Pass over colored houses to make deliveries. Houses close to your \
                             charging station are colored green, and they become white, then red \
                             as they get farther away.",
                            "You get more points for delivering to buildings with multiple \
                             families inside.",
                            "",
                            "To recharge, stop on a commercial building and hold down the \
                             spacebar key. This resets your charging station.",
                            "",
                            "When you deliver enough presents, a little bit of magic is restored, \
                             and you can upzone buildings to make your job easier.",
                            "If you're having trouble delivering to houses far away from \
                             businesses, why not build a new grocery store where it might be \
                             needed?",
                        ],
                    ));
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
