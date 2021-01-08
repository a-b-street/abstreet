//! This is a tool to experiment with the concept of 15-minute neighborhoods. Can you access your
//! daily needs (like groceries, a cafe, a library) within a 15-minute walk, bike ride, or public
//! transit ride of your home?
//!
//! See https://github.com/dabreegster/abstreet/issues/393 for more context.

use abstutil::prettyprint_usize;
use geom::{Distance, Duration};
use map_gui::tools::{nice_map_name, open_browser, CityPicker, ColorLegend, Navigator, PopupMsg};
use map_gui::ID;
use map_model::connectivity::WalkingOptions;
use map_model::{AmenityType, Building, BuildingID, LaneType};
use widgetry::table::{Col, Filter, Table};
use widgetry::{
    lctrl, Btn, Cached, Checkbox, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Panel, RewriteColor, State, Text, TextExt, Transition,
    VerticalAlignment, Widget,
};

use crate::find_home::FindHome;
use crate::isochrone::{Isochrone, Options};
use crate::App;

/// This is the UI state for exploring the isochrone/walkshed from a single building.
pub struct Viewer {
    panel: Panel,
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
        Viewer::new(ctx, app, start)
    }

    pub fn new(ctx: &mut EventCtx, app: &App, start: BuildingID) -> Box<dyn State<App>> {
        let options = Options::Walking(WalkingOptions::default());
        let start = app.map.get_b(start);
        let isochrone = Isochrone::new(ctx, app, start.id, options);
        let highlight_start = draw_star(ctx, start);
        let panel = build_panel(ctx, app, start, &isochrone);
        let draw_unwalkable_roads = draw_unwalkable_roads(ctx, app, &isochrone.options);

        Box::new(Viewer {
            panel,
            highlight_start: ctx.upload(highlight_start),
            isochrone,
            hovering_on_bldg: Cached::new(),
            hovering_on_category: None,
            draw_unwalkable_roads,
        })
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        // Allow panning and zooming
        ctx.canvas_movement();

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
                let category = AmenityType::parse(category).unwrap();
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
        }

        // Don't call normal_left_click unless we're hovering on something in map-space; otherwise
        // panel.event never sees clicks.
        if let Some((hover_id, _)) = self.hovering_on_bldg.key() {
            if ctx.normal_left_click() {
                let start = app.map.get_b(hover_id);
                self.isochrone = Isochrone::new(ctx, app, start.id, self.isochrone.options.clone());
                let star = draw_star(ctx, start);
                self.highlight_start = ctx.upload(star);
                self.panel = build_panel(ctx, app, start, &self.isochrone);
                // Any previous hover is from the perspective of the old `highlight_start`.
                // Remove it so we don't have a dotted line to the previous isochrone's origin
                self.hovering_on_bldg.clear();
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "change map" => {
                    return Transition::Push(CityPicker::new(
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
                "close" => {
                    return Transition::Pop;
                }
                "About" => {
                    return Transition::Push(PopupMsg::new(
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
                    return Transition::Push(Navigator::new(ctx, app));
                }
                "Find your perfect home" => {
                    return Transition::Push(FindHome::new(ctx, self.isochrone.options.clone()));
                }
                x => {
                    if let Some(category) = x.strip_prefix("businesses: ") {
                        return Transition::Push(ExploreAmenities::new(
                            ctx,
                            app,
                            &self.isochrone,
                            AmenityType::parse(category).unwrap(),
                        ));
                    } else {
                        unreachable!()
                    }
                }
            },
            Outcome::Changed => {
                let options = options_from_controls(&self.panel);
                self.draw_unwalkable_roads = draw_unwalkable_roads(ctx, app, &options);
                self.isochrone = Isochrone::new(ctx, app, self.isochrone.start, options);
                self.panel = build_panel(
                    ctx,
                    app,
                    app.map.get_b(self.isochrone.start),
                    &self.isochrone,
                );
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.isochrone.draw);
        g.redraw(&self.highlight_start);
        g.redraw(&self.draw_unwalkable_roads);
        self.panel.draw(g);
        if let Some(ref hover) = self.hovering_on_bldg.value() {
            g.draw_mouse_tooltip(hover.tooltip.clone());
            g.redraw(&hover.drawn_route);
        }
        if let Some((_, ref draw)) = self.hovering_on_category {
            g.redraw(draw);
        }
    }
}

fn options_to_controls(ctx: &mut EventCtx, opts: &Options) -> Widget {
    let mut rows = vec![Checkbox::toggle(
        ctx,
        "walking / biking",
        "walking",
        "biking",
        None,
        match opts {
            Options::Walking(_) => true,
            Options::Biking => false,
        },
    )];
    match opts {
        Options::Walking(ref opts) => {
            rows.push(Checkbox::switch(
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
        Options::Biking => {}
    }
    Widget::col(rows)
}

fn options_from_controls(panel: &Panel) -> Options {
    if panel.is_checked("walking / biking") {
        Options::Walking(WalkingOptions {
            allow_shoulders: panel
                .maybe_is_checked("Allow walking on the shoulder of the road without a sidewalk")
                .unwrap_or(true),
            walking_speed: panel
                .maybe_dropdown_value("speed")
                .unwrap_or(WalkingOptions::default_speed()),
        })
    } else {
        Options::Biking
    }
}

fn draw_star(ctx: &mut EventCtx, b: &Building) -> GeomBatch {
    GeomBatch::load_svg(ctx, "system/assets/tools/star.svg")
        .centered_on(b.polygon.center())
        .color(RewriteColor::ChangeAll(Color::BLACK))
}

fn build_panel(ctx: &mut EventCtx, app: &App, start: &Building, isochrone: &Isochrone) -> Panel {
    let mut rows = Vec::new();

    rows.push(Widget::row(vec![
        Line("15-minute neighborhood explorer")
            .small_heading()
            .draw(ctx),
        Btn::close(ctx),
    ]));

    rows.push(Widget::row(vec![
        "Map:".draw_text(ctx),
        Btn::pop_up(ctx, Some(nice_map_name(app.map.get_name()))).build(
            ctx,
            "change map",
            lctrl(Key::L),
        ),
    ]));

    rows.push(
        Text::from_all(vec![
            Line("Starting from: ").secondary(),
            Line(&start.address),
        ])
        .draw(ctx),
    );

    rows.push(
        Text::from_all(vec![
            Line("Estimated population: ").secondary(),
            Line(prettyprint_usize(isochrone.population)),
        ])
        .draw(ctx),
    );

    rows.push(
        Text::from_all(vec![
            Line("Estimated street parking spots: ").secondary(),
            Line(prettyprint_usize(isochrone.onstreet_parking_spots)),
        ])
        .draw(ctx),
    );

    rows.push(ColorLegend::categories(
        ctx,
        vec![
            (Color::GREEN, "5 mins"),
            (Color::ORANGE, "10 mins"),
            (Color::RED, "15 mins"),
        ],
    ));

    for (amenity, buildings) in isochrone.amenities_reachable.borrow() {
        rows.push(
            Btn::text_fg(format!("{}: {}", amenity, buildings.len())).build(
                ctx,
                format!("businesses: {}", amenity),
                None,
            ),
        );
    }

    // Start of toolbar
    rows.push(Widget::horiz_separator(ctx, 0.3).margin_above(10));

    rows.push(options_to_controls(ctx, &isochrone.options));
    rows.push(Btn::text_bg1("Find your perfect home").build_def(ctx, None));
    rows.push(Widget::row(vec![
        Btn::plaintext("About").build_def(ctx, None),
        Btn::svg_def("system/assets/tools/search.svg").build(ctx, "search", lctrl(Key::F)),
    ]));

    Panel::new(Widget::col(rows))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
        .build(ctx)
}

struct HoverOnBuilding {
    tooltip: Text,
    drawn_route: Drawable,
}
/// (building, scale factor)
type HoverKey = (BuildingID, f64);

impl HoverOnBuilding {
    fn key(ctx: &EventCtx, app: &App) -> Option<HoverKey> {
        match app.mouseover_unzoomed_buildings(ctx) {
            Some(ID::Building(b)) => {
                let scale_factor = if ctx.canvas.cam_zoom >= app.opts.min_zoom_for_detail {
                    1.0
                } else {
                    10.0
                };
                Some((b, scale_factor))
            }
            _ => None,
        }
    }

    fn value(
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
                Text::from(Line(format!("{} away", time)))
            } else {
                Text::from(Line("This is more than 15 minutes away"))
            },
            drawn_route: ctx.upload(batch),
        }
    }
}

struct ExploreAmenities {
    category: AmenityType,
    table: Table<App, Entry, ()>,
    panel: Panel,
    draw: Drawable,
}

struct Entry {
    bldg: BuildingID,
    name: String,
    amenity_type: String,
    address: String,
    duration_away: Duration,
}

impl ExploreAmenities {
    fn new(
        ctx: &mut EventCtx,
        app: &App,
        isochrone: &Isochrone,
        category: AmenityType,
    ) -> Box<dyn State<App>> {
        let mut batch = isochrone.draw_isochrone(app);
        batch.append(draw_star(ctx, app.map.get_b(isochrone.start)));

        let mut entries = Vec::new();
        for b in isochrone.amenities_reachable.get(category) {
            let bldg = app.map.get_b(*b);
            for amenity in &bldg.amenities {
                if AmenityType::categorize(&amenity.amenity_type) == Some(category) {
                    entries.push(Entry {
                        bldg: bldg.id,
                        name: amenity.names.get(app.opts.language.as_ref()).to_string(),
                        amenity_type: amenity.amenity_type.clone(),
                        address: bldg.address.clone(),
                        duration_away: isochrone.time_to_reach_building[&bldg.id],
                    });
                    // Highlight the matching buildings
                    batch.push(Color::RED, bldg.polygon.clone());
                }
            }
        }

        let mut table: Table<App, Entry, ()> = Table::new(
            entries,
            // The label has extra junk to avoid crashing when one building has two stores
            Box::new(|x| format!("{}: {}", x.bldg.0, x.name)),
            "Time to reach",
            Filter::empty(),
        );
        table.column(
            "Type",
            Box::new(|ctx, _, x| Text::from(Line(&x.amenity_type)).render(ctx)),
            Col::Sortable(Box::new(|rows| {
                rows.sort_by_key(|x| x.amenity_type.clone())
            })),
        );
        table.static_col("Name", Box::new(|x| x.name.clone()));
        table.static_col("Address", Box::new(|x| x.address.clone()));
        table.column(
            "Time to reach",
            Box::new(|ctx, app, x| {
                Text::from(Line(x.duration_away.to_string(&app.opts.units))).render(ctx)
            }),
            Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.duration_away))),
        );

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line(format!("{} within 15 minutes", category))
                    .small_heading()
                    .draw(ctx),
                Btn::close(ctx),
            ]),
            table.render(ctx, app),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::TopInset)
        .build(ctx);

        Box::new(ExploreAmenities {
            category: category,
            table,
            panel,
            draw: ctx.upload(batch),
        })
    }

    fn recalc(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut new = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line(format!("{} within 15 minutes", self.category))
                    .small_heading()
                    .draw(ctx),
                Btn::close(ctx),
            ]),
            self.table.render(ctx, app),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::TopInset)
        .build(ctx);
        new.restore(ctx, &self.panel);
        self.panel = new;
    }
}

impl State<App> for ExploreAmenities {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if self.table.clicked(&x) {
                    self.recalc(ctx, app);
                } else if x == "close" {
                    return Transition::Pop;
                } else if let Some(idx) = x.split(":").next().and_then(|x| x.parse::<usize>().ok())
                {
                    let b = app.map.get_b(BuildingID(idx));
                    open_browser(b.orig_id.to_string());
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed => {
                self.table.panel_changed(&self.panel);
                self.recalc(ctx, app);
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw);
        self.panel.draw(g);
        if let Some(x) = self
            .panel
            .currently_hovering()
            .and_then(|x| x.split(":").next())
            .and_then(|x| x.parse::<usize>().ok())
        {
            g.draw_polygon(Color::CYAN, app.map.get_b(BuildingID(x)).polygon.clone());
        }
    }
}

fn draw_unwalkable_roads(ctx: &mut EventCtx, app: &App, opts: &Options) -> Drawable {
    let allow_shoulders = match opts {
        Options::Walking(ref opts) => opts.allow_shoulders,
        Options::Biking => {
            return Drawable::empty(ctx);
        }
    };

    let mut batch = GeomBatch::new();
    'ROADS: for road in app.map.all_roads() {
        if road.is_light_rail() {
            continue;
        }
        for (_, _, lt) in road.lanes_ltr() {
            if lt == LaneType::Sidewalk || (lt == LaneType::Shoulder && allow_shoulders) {
                continue 'ROADS;
            }
        }
        // TODO Skip highways
        batch.push(Color::BLUE.alpha(0.5), road.get_thick_polygon(&app.map));
    }
    ctx.upload(batch)
}
