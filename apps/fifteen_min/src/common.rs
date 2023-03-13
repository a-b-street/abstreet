use std::str::FromStr;

use abstutil::MultiMap;
use geom::Distance;
use map_gui::tools::{CityPicker, Navigator};
use map_gui::ID;
use map_model::connectivity::WalkingOptions;
use map_model::{AmenityType, BuildingID};
use widgetry::tools::{ColorLegend, PopupMsg};
use widgetry::{
    lctrl, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Panel,
    Text, Toggle, Transition, VerticalAlignment, Widget,
};

use crate::isochrone::{Isochrone, MovementOptions, Options};
use crate::App;

pub enum Mode {
    SingleStart,
    StartFromAmenity,
    ScoreHomes,
}

pub fn build_panel(ctx: &mut EventCtx, app: &App, mode: Mode, contents: Widget) -> Panel {
    fn current_mode(ctx: &mut EventCtx, name: &str) -> Widget {
        ctx.style()
            .btn_solid_primary
            .text(name)
            .disabled(true)
            .build_def(ctx)
    }

    let rows = vec![
        map_gui::tools::app_header(ctx, app, "15-minute neighborhood explorer"),
        Widget::row(vec![
            ctx.style().btn_outline.text("About").build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Sketch bus route (experimental)")
                .build_def(ctx),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/search.svg")
                .hotkey(lctrl(Key::F))
                .build_widget(ctx, "search"),
        ]),
        Widget::horiz_separator(ctx, 1.0).margin_above(10),
        Widget::row(vec![
            if matches!(mode, Mode::SingleStart { .. }) {
                current_mode(ctx, "Start from a building")
            } else {
                ctx.style()
                    .btn_outline
                    .text("Start from a building")
                    .build_def(ctx)
            },
            if matches!(mode, Mode::StartFromAmenity { .. }) {
                current_mode(ctx, "Start from an amenity")
            } else {
                ctx.style()
                    .btn_outline
                    .text("Start from an amenity")
                    .build_def(ctx)
            },
            if matches!(mode, Mode::ScoreHomes { .. }) {
                current_mode(ctx, "Score homes by access")
            } else {
                ctx.style()
                    .btn_outline
                    .text("Score homes by access")
                    .build_def(ctx)
            },
        ]),
        contents.named("contents"),
        Widget::horiz_separator(ctx, 1.0).margin_above(10),
        options_to_controls(ctx, &app.session),
    ];

    Panel::new_builder(Widget::col(rows))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
        .build(ctx)
}

pub fn on_click(ctx: &mut EventCtx, app: &App, x: &str) -> Transition<App> {
    match x {
        "Sketch bus route (experimental)" => {
            return Transition::Push(crate::bus::BusExperiment::new_state(ctx, app));
        }
        "Home" => {
            return Transition::Clear(vec![map_gui::tools::TitleScreen::new_state(
                ctx,
                app,
                map_gui::tools::Executable::FifteenMin,
                Box::new(|ctx, app, _| crate::single_start::SingleStart::random_start(ctx, app)),
            )]);
        }
        "change map" => {
            return Transition::Push(CityPicker::new_state(
                ctx,
                app,
                Box::new(|ctx, app| {
                    Transition::Multi(vec![
                        Transition::Pop,
                        Transition::Replace(crate::single_start::SingleStart::random_start(
                            ctx, app,
                        )),
                    ])
                }),
            ));
        }
        "About" => {
            return Transition::Push(PopupMsg::new_state(
                ctx,
                "15-minute neighborhood explorer",
                vec![
                    "What if you could access most of your daily needs with a 15-minute \
                             walk or bike ride from your house?",
                    "Wouldn't it be nice to not rely on a climate unfriendly motor \
                             vehicle and get stuck in traffic for these simple errands?",
                    "Different cities around the world are talking about what design and \
                             policy changes could lead to 15-minute neighborhoods.",
                    "This tool lets you see what commercial amenities are near you right \
                             now, using data from OpenStreetMap.",
                    "",
                    "Note that sidewalks and crosswalks are assumed on most roads.",
                    "Especially around North Seattle, many roads lack sidewalks and \
                             aren't safe for some people to use.",
                    "We're working to improve the accuracy of the map.",
                ],
            ));
        }
        "search" => {
            return Transition::Push(Navigator::new_state(ctx, app));
        }
        "Start from a building" => {
            return Transition::Replace(crate::single_start::SingleStart::random_start(ctx, app));
        }
        "Start from an amenity" => {
            return Transition::Replace(crate::from_amenity::FromAmenity::random_amenity(ctx, app));
        }
        "Score homes by access" => {
            return Transition::Push(crate::score_homes::ScoreHomes::new_state(
                ctx,
                app,
                Vec::new(),
            ));
        }
        _ => panic!("Unhandled click {x}"),
    }
}

fn options_to_controls(ctx: &mut EventCtx, opts: &Options) -> Widget {
    let mut rows = vec![Toggle::choice(
        ctx,
        "walking / biking",
        "walking",
        "biking",
        None,
        match opts.movement {
            MovementOptions::Walking(_) => true,
            MovementOptions::Biking => false,
        },
    )];
    match opts.movement {
        MovementOptions::Walking(ref opts) => {
            rows.push(Toggle::switch(
                ctx,
                "Allow walking on the shoulder of the road without a sidewalk",
                None,
                opts.allow_shoulders,
            ));
            rows.push(Widget::dropdown(
                ctx,
                "speed",
                opts.walking_speed,
                WalkingOptions::common_speeds()
                    .into_iter()
                    .map(|(label, speed)| Choice::new(label, speed))
                    .collect(),
            ));

            rows.push(ColorLegend::row(ctx, Color::BLUE, "unwalkable roads"));
        }
        MovementOptions::Biking => {}
    }
    Widget::col(rows).section(ctx)
}

pub fn options_from_controls(panel: &Panel) -> MovementOptions {
    if panel.is_checked("walking / biking") {
        MovementOptions::Walking(WalkingOptions {
            allow_shoulders: panel
                .maybe_is_checked("Allow walking on the shoulder of the road without a sidewalk")
                .unwrap_or(true),
            walking_speed: panel
                .maybe_dropdown_value("speed")
                .unwrap_or_else(WalkingOptions::default_speed),
        })
    } else {
        MovementOptions::Biking
    }
}

pub struct HoverOnBuilding {
    pub tooltip: Text,
    pub drawn_route: Drawable,
}
/// (building, scale factor)
pub type HoverKey = (BuildingID, f64);

impl HoverOnBuilding {
    pub fn key(ctx: &EventCtx, app: &App) -> Option<HoverKey> {
        match app.mouseover_unzoomed_buildings(ctx) {
            Some(ID::Building(b)) => {
                let scale_factor = if ctx.canvas.is_zoomed() { 1.0 } else { 10.0 };
                Some((b, scale_factor))
            }
            _ => None,
        }
    }

    pub fn value(
        ctx: &mut EventCtx,
        app: &App,
        key: HoverKey,
        isochrone: &Isochrone,
    ) -> HoverOnBuilding {
        debug!("Calculating route for {:?}", key);

        let (hover_id, scale_factor) = key;
        let mut batch = GeomBatch::new();
        if let Some(polyline) = isochrone
            .path_to(&app.map, hover_id)
            .and_then(|path| path.trace(&app.map))
        {
            let dashed_lines = polyline.dashed_lines(
                Distance::meters(0.75 * scale_factor),
                Distance::meters(1.0 * scale_factor),
                Distance::meters(0.4 * scale_factor),
            );
            batch.extend(Color::BLACK, dashed_lines);
        }

        HoverOnBuilding {
            tooltip: if let Some(time) = isochrone.time_to_reach_building.get(&hover_id) {
                Text::from(format!("{} away", time))
            } else {
                Text::from("This is more than 15 minutes away")
            },
            drawn_route: ctx.upload(batch),
        }
    }
}

pub struct HoverOnCategory {
    // TODO Try using Cached?
    state: Option<(AmenityType, Drawable)>,
    color: Color,
}

impl HoverOnCategory {
    pub fn new(color: Color) -> Self {
        Self { state: None, color }
    }

    pub fn update_on_mouse_move(
        &mut self,
        ctx: &EventCtx,
        app: &App,
        panel: &Panel,
        amenities_reachable: &MultiMap<AmenityType, BuildingID>,
    ) {
        let key = panel
            .currently_hovering()
            .and_then(|x| x.strip_prefix("businesses: "));
        if let Some(category) = key {
            let category = AmenityType::from_str(category).unwrap();
            if self
                .state
                .as_ref()
                .map(|(cat, _)| *cat != category)
                .unwrap_or(true)
            {
                let mut batch = GeomBatch::new();
                for b in amenities_reachable.get(category) {
                    batch.push(self.color, app.map.get_b(*b).polygon.clone());
                }
                self.state = Some((category, ctx.upload(batch)));
            }
        } else {
            self.state = None;
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let Some((_, ref draw)) = self.state {
            g.redraw(draw);
        }
    }
}
