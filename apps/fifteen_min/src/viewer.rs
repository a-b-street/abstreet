//! This is a tool to experiment with the concept of 15-minute neighborhoods. Can you access your
//! daily needs (like groceries, a cafe, a library) within a 15-minute walk, bike ride, or public
//! transit ride of your home?
//!
//! See https://github.com/a-b-street/abstreet/issues/393 for more context.

use abstutil::prettyprint_usize;
use geom::{Distance, FindClosest, Percent};
use map_gui::tools::{CityPicker, Navigator};
use map_gui::ID;
use map_model::connectivity::WalkingOptions;
use map_model::{AmenityType, Building, BuildingID};
use std::str::FromStr;
use widgetry::tools::{ColorLegend, PopupMsg, URLManager};
use widgetry::{
    lctrl, Cached, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Panel, State, Text, Toggle, Transition, VerticalAlignment, Widget,
};

use crate::find_amenities::FindAmenity;
use crate::find_home::FindHome;
use crate::isochrone::{Isochrone, MovementOptions, Options};
use crate::{render, App};

/// This is the UI state for exploring the isochrone/walkshed from a single building.
pub struct Viewer {
    panel: Panel,
    snap_to_buildings: FindClosest<BuildingID>,
    highlight_start: Drawable,
    isochrone: Isochrone,

    hovering_on_bldg: Cached<HoverKey, HoverOnBuilding>,
    // TODO Can't use Cached due to a double borrow
    hovering_on_category: Option<(AmenityType, Drawable)>,
    draw_unwalkable_roads: Drawable,
}

impl Viewer {
    /// Start with a random building
    pub fn random_start(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let bldgs = app.map.all_buildings();
        let start = bldgs[bldgs.len() / 2].id;
        Viewer::new_state(ctx, app, start)
    }

    pub fn new_state(ctx: &mut EventCtx, app: &App, start: BuildingID) -> Box<dyn State<App>> {
        map_gui::tools::update_url_map_name(app);

        let options = Options {
            movement: MovementOptions::Walking(WalkingOptions::default()),
            thresholds: Options::default_thresholds(),
        };
        let start = app.map.get_b(start);
        let isochrone = Isochrone::new(ctx, app, vec![start.id], options);
        let highlight_start = render::draw_star(ctx, start);
        let panel = build_panel(ctx, app, start, &isochrone);
        let draw_unwalkable_roads = render::draw_unwalkable_roads(ctx, app, &isochrone.options);

        let mut snap_to_buildings = FindClosest::new();
        for b in app.map.all_buildings() {
            snap_to_buildings.add_polygon(b.id, &b.polygon);
        }

        Box::new(Viewer {
            panel,
            snap_to_buildings,
            highlight_start: ctx.upload(highlight_start),
            isochrone,
            hovering_on_bldg: Cached::new(),
            hovering_on_category: None,
            draw_unwalkable_roads,
        })
    }

    fn change_start(&mut self, ctx: &mut EventCtx, app: &App, b: BuildingID) {
        if self.isochrone.start[0] == b {
            return;
        }

        let start = app.map.get_b(b);
        self.isochrone = Isochrone::new(ctx, app, vec![start.id], self.isochrone.options.clone());
        let star = render::draw_star(ctx, start);
        self.highlight_start = ctx.upload(star);
        self.panel = build_panel(ctx, app, start, &self.isochrone);
        // Any previous hover is from the perspective of the old `highlight_start`.
        // Remove it so we don't have a dotted line to the previous isochrone's origin
        self.hovering_on_bldg.clear();
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        // Allow panning and zooming
        if ctx.canvas_movement() {
            URLManager::update_url_cam(ctx, app.map.get_gps_bounds());
        }

        if ctx.redo_mouseover() {
            let isochrone = &self.isochrone;
            self.hovering_on_bldg
                .update(HoverOnBuilding::key(ctx, app), |key| {
                    HoverOnBuilding::value(ctx, app, key, isochrone)
                });
            // Also update this to conveniently get an outline drawn. Note we don't want to do this
            // inside the callback above, because it doesn't run when the key becomes None.
            app.current_selection = self.hovering_on_bldg.key().map(|(b, _)| ID::Building(b));

            // Update the preview of all businesses belonging to one category
            let key = self
                .panel
                .currently_hovering()
                .and_then(|x| x.strip_prefix("businesses: "));
            if let Some(category) = key {
                let category = AmenityType::from_str(category).unwrap();
                if self
                    .hovering_on_category
                    .as_ref()
                    .map(|(cat, _)| *cat != category)
                    .unwrap_or(true)
                {
                    let mut batch = GeomBatch::new();
                    for b in self.isochrone.amenities_reachable.get(category) {
                        batch.push(Color::RED, app.map.get_b(*b).polygon.clone());
                    }
                    self.hovering_on_category = Some((category, ctx.upload(batch)));
                }
            } else {
                self.hovering_on_category = None;
            }

            if ctx.is_key_down(Key::LeftControl) {
                if let Some(cursor) = ctx.canvas.get_cursor_in_map_space() {
                    if let Some((b, _)) = self
                        .snap_to_buildings
                        .closest_pt(cursor, Distance::meters(30.0))
                    {
                        self.change_start(ctx, app, b);
                    }
                }
            }
        }

        // Don't call normal_left_click unless we're hovering on something in map-space; otherwise
        // panel.event never sees clicks.
        if let Some(cursor) = ctx.canvas.get_cursor_in_map_space() {
            if ctx.normal_left_click() {
                if let Some((b, _)) = self
                    .snap_to_buildings
                    .closest_pt(cursor, Distance::meters(30.0))
                {
                    self.change_start(ctx, app, b);
                }
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Sketch bus route (experimental)" => {
                    return Transition::Push(crate::bus::BusExperiment::new_state(ctx, app));
                }
                "Home" => {
                    return Transition::Clear(vec![map_gui::tools::TitleScreen::new_state(
                        ctx,
                        app,
                        map_gui::tools::Executable::FifteenMin,
                        Box::new(|ctx, app, _| Self::random_start(ctx, app)),
                    )]);
                }
                "change map" => {
                    return Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(Self::random_start(ctx, app)),
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
                "Find your perfect home" => {
                    return Transition::Push(FindHome::new_state(
                        ctx,
                        self.isochrone.options.clone(),
                    ));
                }
                "Search by amenity" => {
                    return Transition::Push(FindAmenity::new_state(
                        ctx,
                        self.isochrone.options.clone(),
                    ));
                }
                x => {
                    if let Some(category) = x.strip_prefix("businesses: ") {
                        return Transition::Push(
                            crate::amenities_details::ExploreAmenitiesDetails::new_state(
                                ctx,
                                app,
                                &self.isochrone,
                                AmenityType::from_str(category).unwrap(),
                            ),
                        );
                    } else {
                        unreachable!()
                    }
                }
            },
            Outcome::Changed(_) => {
                let options = Options {
                    movement: options_from_controls(&self.panel),
                    thresholds: Options::default_thresholds(),
                };
                self.draw_unwalkable_roads = render::draw_unwalkable_roads(ctx, app, &options);
                self.isochrone = Isochrone::new(ctx, app, vec![self.isochrone.start[0]], options);
                self.panel = build_panel(
                    ctx,
                    app,
                    app.map.get_b(self.isochrone.start[0]),
                    &self.isochrone,
                );
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.isochrone.draw.draw(g);
        g.redraw(&self.highlight_start);
        g.redraw(&self.draw_unwalkable_roads);
        self.panel.draw(g);
        if let Some(hover) = self.hovering_on_bldg.value() {
            g.draw_mouse_tooltip(hover.tooltip.clone());
            g.redraw(&hover.drawn_route);
        }
        if let Some((_, ref draw)) = self.hovering_on_category {
            g.redraw(draw);
        }
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
    Widget::col(rows)
}

fn options_from_controls(panel: &Panel) -> MovementOptions {
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

fn build_panel(ctx: &mut EventCtx, app: &App, start: &Building, isochrone: &Isochrone) -> Panel {
    let mut rows = vec![
        map_gui::tools::app_header(ctx, app, "15-minute neighborhood explorer"),
        Widget::row(vec![
            ctx.style()
                .btn_outline
                .text("Find your perfect home")
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Search by amenity")
                .build_def(ctx),
            ctx.style().btn_outline.text("About").build_def(ctx),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/search.svg")
                .hotkey(lctrl(Key::F))
                .build_widget(ctx, "search"),
        ]),
        ctx.style()
            .btn_outline
            .text("Sketch bus route (experimental)")
            .hotkey(Key::B)
            .build_def(ctx),
        Widget::horiz_separator(ctx, 1.0).margin_above(10),
        Text::from_all(vec![
            Line("Click").fg(ctx.style().text_hotkey_color),
            Line(" a building or hold ").secondary(),
            Line(Key::LeftControl.describe()).fg(ctx.style().text_hotkey_color),
            Line(" to change the start point"),
        ])
        .into_widget(ctx),
        Text::from_all(vec![
            Line("Starting from: ").secondary(),
            Line(&start.address),
        ])
        .into_widget(ctx),
        Text::from_all(vec![
            Line("Estimated population: ").secondary(),
            Line(prettyprint_usize(isochrone.population)),
        ])
        .into_widget(ctx),
        Text::from_all(vec![
            Line("Estimated street parking spots: ").secondary(),
            Line(prettyprint_usize(isochrone.onstreet_parking_spots)),
        ])
        .into_widget(ctx),
        ColorLegend::categories(
            ctx,
            vec![
                (Color::GREEN, "5 mins"),
                (Color::ORANGE, "10 mins"),
                (Color::RED, "15 mins"),
            ],
        ),
    ];

    let mut amenities = Vec::new();
    for (amenity, buildings) in isochrone.amenities_reachable.borrow() {
        amenities.push(
            ctx.style()
                .btn_outline
                .text(format!("{}: {}", amenity, buildings.len()))
                .build_widget(ctx, format!("businesses: {}", amenity))
                .margin_right(4)
                .margin_below(4),
        );
    }
    rows.push(Widget::custom_row(amenities).flex_wrap(ctx, Percent::int(30)));

    rows.push(Widget::horiz_separator(ctx, 1.0).margin_above(10));

    rows.push(options_to_controls(ctx, &isochrone.options));

    Panel::new_builder(Widget::col(rows))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
        .build(ctx)
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
