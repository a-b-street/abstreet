use std::collections::BTreeSet;

use anyhow::Result;
use geo::MapCoordsInPlace;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use serde::Serialize;

use map_gui::tools::checkbox_per_mode;
use map_model::{PathV2, Road};
use synthpop::make::ScenarioGenerator;
use synthpop::{Scenario, TripMode};
use widgetry::tools::{FileLoader, PopupMsg};
use widgetry::{
    Color, DrawBaselayer, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, Slider, State, Text, TextExt, Toggle, VerticalAlignment, Widget,
};

use crate::components::{AppwidePanel, Mode};
use crate::impact::{end_of_day, Filters, Impact};
use crate::render::colors;
use crate::{App, Transition};

// TODO Share structure or pieces with Ungap's predict mode
// ... can't we just produce data of a certain shape, and have a UI pretty tuned for that?

pub struct ShowImpactResults {
    appwide_panel: AppwidePanel,
    left_panel: Panel,
}

impl ShowImpactResults {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let map_name = app.per_map.map.get_name().clone();
        if app.per_map.impact.map != map_name {
            let scenario_name = Scenario::default_scenario_for_map(&map_name);

            if scenario_name != "home_to_work" {
                return FileLoader::<App, Scenario>::new_state(
                    ctx,
                    abstio::path_scenario(&map_name, &scenario_name),
                    Box::new(move |ctx, app, timer, maybe_scenario| {
                        // TODO Handle corrupt files
                        let scenario = maybe_scenario.unwrap();
                        app.per_map.impact = Impact::from_scenario(ctx, app, scenario, timer);
                        Transition::Replace(ShowImpactResults::new_state(ctx, app))
                    }),
                );
            }
            ctx.loading_screen("synthesize travel demand model", |ctx, timer| {
                // TODO Argh, this internally uses the map's pathfinder to estimate mode split.
                // Just ignore any edits or pre-existing files.
                app.per_map.map.keep_pathfinder_despite_edits();

                let scenario = ScenarioGenerator::proletariat_robot(
                    &app.per_map.map,
                    &mut XorShiftRng::seed_from_u64(42),
                    timer,
                );
                app.per_map.impact = Impact::from_scenario(ctx, app, scenario, timer);
            });
        }

        if app.per_map.impact.change_key != app.edits().get_change_key() {
            ctx.loading_screen("recalculate impact", |ctx, timer| {
                // Avoid a double borrow
                let mut impact = std::mem::replace(&mut app.per_map.impact, Impact::empty(ctx));
                impact.map_edits_changed(ctx, app, timer);
                app.per_map.impact = impact;
            });
        }

        let contents = Widget::col(vec![
            Line("Impact prediction").small_heading().into_widget(ctx),
            Text::from(Line("This tool starts with a travel demand model, calculates the route every trip takes before and after changes, and displays volumes along roads")).wrap_to_pct(ctx, 20).into_widget(ctx),
            Text::from_all(vec![
                    Line("Red").fg(Color::RED),
                    Line(" roads have increased volume, and "),
                    Line("green").fg(Color::GREEN),
                    Line(" roads have less. Width of the road shows how much baseline traffic it has."),
                ]).wrap_to_pct(ctx, 20).into_widget(ctx),
                Text::from(Line("Click a road to see changed routes through it.")).wrap_to_pct(ctx, 20).into_widget(ctx),
                Text::from(Line("Results may be wrong for various reasons. Interpret carefully.").bold_body()).wrap_to_pct(ctx, 20).into_widget(ctx),
            // TODO Dropdown for the scenario, and explain its source/limitations
            app.per_map.impact.filters.to_panel(ctx, app),
            app.per_map
                .impact
                .compare_counts
                .get_panel_widget(ctx)
                .named("compare counts"),
            ctx.style()
                .btn_outline
                .text("Save before/after counts to files (JSON)")
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Save before/after counts to files (CSV)")
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Save before/after counts to files (GeoJSON)")
                .build_def(ctx),
        ]);
        let appwide_panel = AppwidePanel::new(ctx, app, Mode::Impact);
        let left_panel =
            crate::components::LeftPanel::builder(ctx, &appwide_panel.top_panel, contents)
                .build(ctx);

        Box::new(Self {
            appwide_panel,
            left_panel,
        })
    }
}
impl State<App> for ShowImpactResults {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        // PreserveState doesn't matter
        if let Some(t) =
            self.appwide_panel
                .event(ctx, app, &crate::save::PreserveState::Route, help)
        {
            return t;
        }
        if let Some(t) = app.session.layers.event(ctx, &app.cs, Mode::Impact, None) {
            return t;
        }
        match self.left_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Save before/after counts to files (JSON)" => {
                    let path1 = "counts_a.json";
                    let path2 = "counts_b.json";
                    abstio::write_json(
                        path1.to_string(),
                        &app.per_map.impact.compare_counts.counts_a,
                    );
                    abstio::write_json(
                        path2.to_string(),
                        &app.per_map.impact.compare_counts.counts_b,
                    );
                    return Transition::Push(PopupMsg::new_state(
                        ctx,
                        "Saved",
                        vec![format!("Saved {} and {}", path1, path2)],
                    ));
                }
                "Save before/after counts to files (CSV)" => {
                    let path = "before_after_counts.csv";
                    let msg = match export_csv(app)
                        .and_then(|contents| abstio::write_file(path.to_string(), contents))
                    {
                        Ok(_) => format!("Saved {path}"),
                        Err(err) => format!("Failed to export: {err}"),
                    };
                    return Transition::Push(PopupMsg::new_state(ctx, "CSV export", vec![msg]));
                }
                "Save before/after counts to files (GeoJSON)" => {
                    let path = "before_after_counts.geojson";
                    let msg = match export_geojson(app)
                        .and_then(|contents| abstio::write_file(path.to_string(), contents))
                    {
                        Ok(_) => format!("Saved {path}"),
                        Err(err) => format!("Failed to export: {err}"),
                    };
                    return Transition::Push(PopupMsg::new_state(ctx, "GeoJSON export", vec![msg]));
                }
                x => {
                    // Avoid a double borrow
                    let mut impact = std::mem::replace(&mut app.per_map.impact, Impact::empty(ctx));
                    let widget = impact
                        .compare_counts
                        .on_click(ctx, app, x)
                        .expect("button click didn't belong to CompareCounts");
                    app.per_map.impact = impact;
                    self.left_panel.replace(ctx, "compare counts", widget);
                    return Transition::Keep;
                }
            },
            Outcome::Changed(_) => {
                // TODO The sliders should only trigger updates when the user lets go; way too slow
                // otherwise
                let filters = Filters::from_panel(&self.left_panel);
                if filters == app.per_map.impact.filters {
                    return Transition::Keep;
                }

                // Avoid a double borrow
                let mut impact = std::mem::replace(&mut app.per_map.impact, Impact::empty(ctx));
                impact.filters = Filters::from_panel(&self.left_panel);
                ctx.loading_screen("update filters", |ctx, timer| {
                    impact.trips_changed(ctx, app, timer);
                });
                app.per_map.impact = impact;
                return Transition::Keep;
            }
            _ => {}
        }

        if let Some(r) = app.per_map.impact.compare_counts.other_event(ctx) {
            let results = ctx.loading_screen("find changed routes", |_, timer| {
                app.per_map.impact.find_changed_routes(app, r, timer)
            });
            return Transition::Push(ChangedRoutes::new_state(ctx, app, results));
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // Just emphasize roads that've changed, so don't draw the baselayer of roads. Even
        // buildings are a distraction.
        g.clear(app.cs.void_background);
        g.redraw(&app.per_map.draw_map.boundary_polygon);
        g.redraw(&app.per_map.draw_map.draw_all_areas);
        app.per_map.impact.compare_counts.draw(g, app);
        app.per_map.draw_all_filters.draw(g);

        self.appwide_panel.draw(g);
        self.left_panel.draw(g);
        app.session.layers.draw(g, app);
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app)
    }
}

impl Filters {
    fn from_panel(panel: &Panel) -> Filters {
        let (p1, p2) = (
            panel.slider("depart from").get_percent(),
            panel.slider("depart until").get_percent(),
        );
        let departure_time = (end_of_day().percent_of(p1), end_of_day().percent_of(p2));
        let modes = TripMode::all()
            .into_iter()
            .filter(|m| panel.is_checked(m.ongoing_verb()))
            .collect::<BTreeSet<_>>();
        Filters {
            modes,
            include_borders: panel.is_checked("include borders"),
            departure_time,
        }
    }

    fn to_panel(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        Widget::col(vec![
            "Filter trips".text_widget(ctx),
            Toggle::switch(ctx, "include borders", None, self.include_borders),
            Widget::row(vec![
                "Departing from:".text_widget(ctx).margin_right(20),
                Slider::area(
                    ctx,
                    0.15 * ctx.canvas.window_width,
                    self.departure_time.0.to_percent(end_of_day()),
                    "depart from",
                ),
            ]),
            Widget::row(vec![
                "Departing until:".text_widget(ctx).margin_right(20),
                Slider::area(
                    ctx,
                    0.15 * ctx.canvas.window_width,
                    self.departure_time.1.to_percent(end_of_day()),
                    "depart until",
                ),
            ]),
            checkbox_per_mode(ctx, app, &self.modes),
            // TODO Filter by trip purpose
        ])
        .section(ctx)
    }
}

fn help() -> Vec<&'static str> {
    vec![
        "This tool is still experimental.",
        "Until better travel demand models are available, we can't predict where most detours will occur,",
        "because we don't know where trips begin and end.",
        "",
        "And note this tool doesn't predict traffic dissipation as people decide to not drive.",
    ]
}

struct ChangedRoutes {
    panel: Panel,
    // TODO Not sure what to precompute. Smallest memory would be the PathRequest.
    paths: Vec<(PathV2, PathV2)>,
    current: usize,
    draw_paths: Drawable,
}

impl ChangedRoutes {
    fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        paths: Vec<(PathV2, PathV2)>,
    ) -> Box<dyn State<App>> {
        if paths.is_empty() {
            return PopupMsg::new_state(
                ctx,
                "No changes",
                vec!["No routes changed near this road"],
            );
        }

        let mut state = ChangedRoutes {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Routes that changed near a road")
                        .small_heading()
                        .into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Widget::row(vec![
                    ctx.style()
                        .btn_prev()
                        .hotkey(Key::LeftArrow)
                        .build_widget(ctx, "previous"),
                    "route X/Y"
                        .text_widget(ctx)
                        .named("pointer")
                        .centered_vert(),
                    ctx.style()
                        .btn_next()
                        .hotkey(Key::RightArrow)
                        .build_widget(ctx, "next"),
                ])
                .evenly_spaced(),
                Line("Route before changes")
                    .fg(*colors::PLAN_ROUTE_BEFORE)
                    .into_widget(ctx),
                Line("Route after changes")
                    .fg(*colors::PLAN_ROUTE_AFTER)
                    .into_widget(ctx),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            paths,
            current: 0,
            draw_paths: Drawable::empty(ctx),
        };
        state.recalculate(ctx, app);
        Box::new(state)
    }

    fn recalculate(&mut self, ctx: &mut EventCtx, app: &App) {
        self.panel.replace(
            ctx,
            "pointer",
            format!("route {}/{}", self.current + 1, self.paths.len()).text_widget(ctx),
        );

        let mut batch = map_gui::tools::draw_overlapping_paths(
            app,
            vec![
                (
                    self.paths[self.current].0.clone(),
                    *colors::PLAN_ROUTE_BEFORE,
                ),
                (
                    self.paths[self.current].1.clone(),
                    *colors::PLAN_ROUTE_AFTER,
                ),
            ],
        )
        .unzoomed;
        let req = self.paths[self.current].0.get_req();
        batch.append(map_gui::tools::start_marker(
            ctx,
            req.start.pt(&app.per_map.map),
            2.0,
        ));
        batch.append(map_gui::tools::goal_marker(
            ctx,
            req.end.pt(&app.per_map.map),
            2.0,
        ));
        self.draw_paths = ctx.upload(batch);
    }
}

impl State<App> for ChangedRoutes {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "previous" => {
                    if self.current != 0 {
                        self.current -= 1;
                    }
                    self.recalculate(ctx, app);
                }
                "next" => {
                    if self.current != self.paths.len() - 1 {
                        self.current += 1;
                    }
                    self.recalculate(ctx, app);
                }
                _ => unreachable!(),
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        g.redraw(&self.draw_paths);
        app.per_map.draw_all_filters.draw(g);
        app.per_map.draw_poi_icons.draw(g);
    }
}

fn export_csv(app: &App) -> Result<String> {
    let mut out = Vec::new();
    {
        let mut writer = csv::Writer::from_writer(&mut out);
        for r in app.per_map.map.all_roads() {
            writer.serialize(ExportRow::new(r, app))?;
        }
        writer.flush()?;
    }
    let out = String::from_utf8(out)?;
    Ok(out)
}

#[derive(Serialize)]
struct ExportRow {
    road_name: String,
    osm_way_id: i64,
    osm_intersection1: i64,
    osm_intersection2: i64,
    total_count_before: usize,
    total_count_after: usize,
}

impl ExportRow {
    fn new(r: &Road, app: &App) -> Self {
        Self {
            road_name: r.get_name(None),
            osm_way_id: r.orig_id.osm_way_id.0,
            osm_intersection1: r.orig_id.i1.0,
            osm_intersection2: r.orig_id.i2.0,
            total_count_before: app
                .per_map
                .impact
                .compare_counts
                .counts_a
                .per_road
                .get(r.id),
            total_count_after: app
                .per_map
                .impact
                .compare_counts
                .counts_b
                .per_road
                .get(r.id),
        }
    }
}

fn export_geojson(app: &App) -> Result<String> {
    let mut string_buffer: Vec<u8> = vec![];
    {
        let mut writer = geojson::FeatureWriter::from_writer(&mut string_buffer);

        #[derive(Serialize)]
        struct RoadGeoJson {
            #[serde(serialize_with = "geojson::ser::serialize_geometry")]
            geometry: geo::LineString,
            #[serde(flatten)]
            export_row: ExportRow,
        }

        for r in app.per_map.map.all_roads() {
            let bounds = app.per_map.map.get_gps_bounds();
            let mut geometry = geo::LineString::from(&r.center_pts);
            geometry.map_coords_in_place(|c| {
                let lonlat = bounds.convert_back_xy(c.x, c.y);
                return geo::coord! { x: lonlat.x(), y: lonlat.y() };
            });

            let sr = RoadGeoJson {
                export_row: ExportRow::new(r, app),
                geometry,
            };

            writer.serialize(&sr)?;
        }
    }
    let out = String::from_utf8(string_buffer)?;
    Ok(out)
}
