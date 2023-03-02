//! This is a tool to experiment with the concept of 15-minute neighborhoods. Can you access your
//! daily needs (like groceries, a cafe, a library) within a 15-minute walk, bike ride, or public
//! transit ride of your home?
//!
//! See https://github.com/a-b-street/abstreet/issues/393 for more context.

use abstutil::prettyprint_usize;
use geom::{Distance, FindClosest, Percent};
use map_gui::ID;
use map_model::{AmenityType, Building, BuildingID};
use std::str::FromStr;
use widgetry::tools::{ColorLegend, URLManager};
use widgetry::{
    Cached, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, State, Text,
    Transition, Widget,
};

use crate::common::{HoverKey, HoverOnBuilding};
use crate::isochrone::{Isochrone, Options};
use crate::{common, render, App};

/// This is the UI state for exploring the isochrone/walkshed from a single building.
pub struct SingleStart {
    panel: Panel,
    snap_to_buildings: FindClosest<BuildingID>,
    draw_unwalkable_roads: Drawable,

    highlight_start: Drawable,
    isochrone: Isochrone,
    hovering_on_bldg: Cached<HoverKey, HoverOnBuilding>,
    // TODO Can't use Cached due to a double borrow
    hovering_on_category: Option<(AmenityType, Drawable)>,
}

impl SingleStart {
    /// Start with a random building
    pub fn random_start(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let bldgs = app.map.all_buildings();
        let start = bldgs[bldgs.len() / 2].id;
        Self::new_state(ctx, app, start)
    }

    pub fn new_state(ctx: &mut EventCtx, app: &App, start: BuildingID) -> Box<dyn State<App>> {
        map_gui::tools::update_url_map_name(app);

        let draw_unwalkable_roads = render::draw_unwalkable_roads(ctx, app);

        let mut snap_to_buildings = FindClosest::new();
        for b in app.map.all_buildings() {
            snap_to_buildings.add_polygon(b.id, &b.polygon);
        }

        let start = app.map.get_b(start);
        let isochrone = Isochrone::new(ctx, app, vec![start.id], app.session.clone());
        let highlight_start = render::draw_star(ctx, start);
        let panel = build_panel(ctx, app, start, &isochrone);

        Box::new(Self {
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
        self.isochrone = Isochrone::new(ctx, app, vec![start.id], app.session.clone());
        let star = render::draw_star(ctx, start);
        self.highlight_start = ctx.upload(star);
        self.panel = build_panel(ctx, app, start, &self.isochrone);
        // Any previous hover is from the perspective of the old `highlight_start`.
        // Remove it so we don't have a dotted line to the previous isochrone's origin
        self.hovering_on_bldg.clear();
    }
}

impl State<App> for SingleStart {
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
            Outcome::Clicked(x) => {
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
                    return common::on_click(ctx, app, &x);
                }
            }
            Outcome::Changed(_) => {
                app.session = Options {
                    movement: common::options_from_controls(&self.panel),
                    thresholds: Options::default_thresholds(),
                };
                self.draw_unwalkable_roads = render::draw_unwalkable_roads(ctx, app);
                self.isochrone =
                    Isochrone::new(ctx, app, vec![self.isochrone.start[0]], app.session.clone());
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

fn build_panel(ctx: &mut EventCtx, app: &App, start: &Building, isochrone: &Isochrone) -> Panel {
    let mut contents = vec![
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
    contents.push(Widget::custom_row(amenities).flex_wrap(ctx, Percent::int(30)));

    common::build_panel(ctx, app, common::Mode::SingleStart, Widget::col(contents))
}
