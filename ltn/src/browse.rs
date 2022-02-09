use std::collections::HashSet;

use abstutil::{Counter, Timer};
use geom::Distance;
use map_gui::tools::{ColorNetwork, DrawRoadLabels, PopupMsg, URLManager};
use synthpop::Scenario;
use widgetry::mapspace::{ToggleZoomed, World, WorldOutcome};
use widgetry::{
    Choice, Color, DrawBaselayer, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    State, Text, TextExt, Toggle, VerticalAlignment, Widget,
};

use super::auto::Heuristic;
use super::{Neighborhood, NeighborhoodID, Partitioning};
use crate::{App, ModalFilters, Toggle3Zoomed, Transition};

pub struct BrowseNeighborhoods {
    panel: Panel,
    world: World<NeighborhoodID>,
    draw_over_roads: ToggleZoomed,
    labels: DrawRoadLabels,
    draw_all_filters: Toggle3Zoomed,
    draw_boundary_roads: ToggleZoomed,
}

impl BrowseNeighborhoods {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        URLManager::update_url_map_name(app);

        let (world, draw_over_roads) =
            ctx.loading_screen("calculate neighborhoods", |ctx, timer| {
                if &app.session.partitioning.map != app.map.get_name() {
                    // Reset this first. transform_existing_filters will fill some out.
                    app.session.modal_filters = ModalFilters::default();
                    crate::filters::transform_existing_filters(ctx, app, timer);
                    app.session.partitioning = Partitioning::seed_using_heuristics(app, timer);
                }
                (
                    make_world(ctx, app, timer),
                    draw_over_roads(ctx, app, timer),
                )
            });
        let draw_all_filters = app.session.modal_filters.draw(ctx, &app.map, None);

        let panel = Panel::new_builder(Widget::col(vec![
            crate::app_header(ctx, app),
            "Click a neighborhood to edit filters".text_widget(ctx),
            Widget::col(vec![
                Widget::row(vec![
                    "Draw neighborhoods:".text_widget(ctx).centered_vert(),
                    Widget::dropdown(
                        ctx,
                        "style",
                        app.session.draw_neighborhood_style,
                        vec![
                            Choice::new("simple", Style::SimpleColoring),
                            Choice::new("cells", Style::Cells),
                            Choice::new("quietness", Style::Quietness),
                            Choice::new("all rat-runs", Style::RatRuns),
                        ],
                    ),
                ]),
                Toggle::checkbox(
                    ctx,
                    "highlight boundary roads",
                    Key::H,
                    app.session.highlight_boundary_roads,
                ),
            ])
            .section(ctx),
            Widget::col(vec![
                Widget::row(vec![
                    ctx.style().btn_outline.text("New").build_def(ctx),
                    ctx.style().btn_outline.text("Load proposal").build_def(ctx),
                    ctx.style().btn_outline.text("Save proposal").build_def(ctx),
                ]),
                ctx.style()
                    .btn_outline
                    .text("Export to GeoJSON")
                    .build_def(ctx),
            ])
            .section(ctx),
            Widget::col(vec![
                "Predict proposal impact (experimental)".text_widget(ctx),
                impact_widget(ctx, app),
            ])
            .section(ctx),
            // TODO Consider putting some of the edit controls here, like undo?
            Widget::col(vec![Widget::row(vec![
                Widget::dropdown(
                    ctx,
                    "heuristic",
                    app.session.heuristic,
                    Heuristic::choices(),
                ),
                ctx.style()
                    .btn_outline
                    .text("Automatically stop rat-runs")
                    .build_def(ctx),
            ])])
            .section(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        Box::new(BrowseNeighborhoods {
            panel,
            world,
            draw_over_roads,
            labels: DrawRoadLabels::only_major_roads(),
            draw_all_filters,
            draw_boundary_roads: draw_boundary_roads(ctx, app),
        })
    }
}

impl State<App> for BrowseNeighborhoods {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "New" => {
                    app.session.partitioning = Partitioning::empty();
                    return Transition::Replace(BrowseNeighborhoods::new_state(ctx, app));
                }
                "Load proposal" => {
                    return Transition::Push(crate::save::Proposal::load_picker_ui(ctx, app));
                }
                "Save proposal" => {
                    return Transition::Push(crate::save::Proposal::save_ui(ctx));
                }
                "Export to GeoJSON" => {
                    let result = super::export::write_geojson_file(ctx, app);
                    return Transition::Push(match result {
                        Ok(path) => PopupMsg::new_state(
                            ctx,
                            "LTNs exported",
                            vec![format!("Data exported to {}", path)],
                        ),
                        Err(err) => {
                            PopupMsg::new_state(ctx, "Export failed", vec![err.to_string()])
                        }
                    });
                }
                "Calculate" | "Show impact" => {
                    return Transition::Push(super::impact::ShowResults::new_state(ctx, app));
                }
                "Automatically stop rat-runs" => {
                    ctx.loading_screen("automatically filter all neighborhoods", |ctx, timer| {
                        for id in app
                            .session
                            .partitioning
                            .all_neighborhoods()
                            .keys()
                            .cloned()
                            .collect::<Vec<_>>()
                        {
                            let neighborhood = Neighborhood::new(ctx, app, id);
                            app.session.heuristic.apply(ctx, app, &neighborhood, timer);
                        }
                    });
                    return Transition::Replace(BrowseNeighborhoods::new_state(ctx, app));
                }
                x => {
                    return crate::handle_app_header_click(ctx, app, x).unwrap();
                }
            },
            Outcome::Changed(_) => {
                app.session.highlight_boundary_roads =
                    self.panel.is_checked("highlight boundary roads");
                app.session.draw_neighborhood_style = self.panel.dropdown_value("style");

                ctx.loading_screen("change style", |ctx, timer| {
                    self.world = make_world(ctx, app, timer);
                    self.draw_over_roads = draw_over_roads(ctx, app, timer);
                });
            }
            _ => {}
        }

        if let WorldOutcome::ClickedObject(id) = self.world.event(ctx) {
            return Transition::Push(super::connectivity::Viewer::new_state(ctx, app, id));
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        crate::draw_with_layering(g, app, |g| self.world.draw(g));
        self.draw_over_roads.draw(g);

        self.panel.draw(g);
        if self.panel.is_checked("highlight boundary roads") {
            self.draw_boundary_roads.draw(g);
        }
        self.draw_all_filters.draw(g);
        if g.canvas.is_unzoomed() {
            self.labels.draw(g, app);
        }
    }
}

fn make_world(ctx: &mut EventCtx, app: &App, timer: &mut Timer) -> World<NeighborhoodID> {
    let mut world = World::bounded(app.map.get_bounds());
    let map = &app.map;
    for (id, (block, color)) in app.session.partitioning.all_neighborhoods() {
        match app.session.draw_neighborhood_style {
            Style::SimpleColoring => {
                world
                    .add(*id)
                    .hitbox(block.polygon.clone())
                    .draw_color(color.alpha(0.3))
                    .hover_outline(Color::BLACK, Distance::meters(5.0))
                    .clickable()
                    .build(ctx);
            }
            Style::Cells => {
                // TODO The cell colors are confusing alongside the other neighborhood colors. I
                // tried greying out everything else, but then the view is too jumpy.
                let neighborhood = Neighborhood::new(ctx, app, *id);
                let render_cells = super::draw_cells::RenderCells::new(map, &neighborhood);
                let hovered_batch = render_cells.draw();
                world
                    .add(*id)
                    .hitbox(block.polygon.clone())
                    .draw_color(color.alpha(0.5))
                    .draw_hovered(hovered_batch)
                    .clickable()
                    .build(ctx);
            }
            Style::Quietness => {
                let neighborhood = Neighborhood::new(ctx, app, *id);
                let rat_runs = super::rat_runs::find_rat_runs(app, &neighborhood, timer);
                let (quiet_streets, total_streets) =
                    rat_runs.quiet_and_total_streets(&neighborhood);
                let pct = if total_streets == 0 {
                    0.0
                } else {
                    1.0 - (quiet_streets as f64 / total_streets as f64)
                };
                let color = app.cs.good_to_bad_red.eval(pct);
                world
                    .add(*id)
                    .hitbox(block.polygon.clone())
                    .draw_color(color.alpha(0.5))
                    .hover_outline(Color::BLACK, Distance::meters(5.0))
                    .clickable()
                    .build(ctx);
            }
            Style::RatRuns => {
                world
                    .add(*id)
                    .hitbox(block.polygon.clone())
                    // Slight lie, because draw_over_roads has to be drawn after the World
                    .drawn_in_master_batch()
                    .hover_outline(Color::BLACK, Distance::meters(5.0))
                    .clickable()
                    .build(ctx);
            }
        }
    }
    world
}

fn draw_over_roads(ctx: &mut EventCtx, app: &App, timer: &mut Timer) -> ToggleZoomed {
    if app.session.draw_neighborhood_style != Style::RatRuns {
        return ToggleZoomed::empty(ctx);
    }

    let mut count_per_road = Counter::new();
    let mut count_per_intersection = Counter::new();

    for id in app.session.partitioning.all_neighborhoods().keys() {
        let neighborhood = Neighborhood::new(ctx, app, *id);
        let rat_runs = super::rat_runs::find_rat_runs(app, &neighborhood, timer);
        count_per_road.extend(rat_runs.count_per_road);
        count_per_intersection.extend(rat_runs.count_per_intersection);
    }

    // TODO It's a bit weird to draw one heatmap covering streets in every neighborhood. The
    // rat-runs are calculated per neighborhood, but now we're showing them all together, as if
    // it's the impact prediction mode using a demand model.
    let mut colorer = ColorNetwork::no_fading(app);
    colorer.ranked_roads(count_per_road, &app.cs.good_to_bad_red);
    colorer.ranked_intersections(count_per_intersection, &app.cs.good_to_bad_red);
    colorer.build(ctx)
}

fn draw_boundary_roads(ctx: &EventCtx, app: &App) -> ToggleZoomed {
    let mut seen_roads = HashSet::new();
    let mut seen_borders = HashSet::new();
    let mut batch = ToggleZoomed::builder();
    for (block, _) in app.session.partitioning.all_neighborhoods().values() {
        for id in &block.perimeter.roads {
            let r = id.road;
            if seen_roads.contains(&r) {
                continue;
            }
            seen_roads.insert(r);
            let road = app.map.get_r(r);
            batch
                .unzoomed
                .push(Color::RED.alpha(0.6), road.get_thick_polygon());
            batch
                .zoomed
                .push(Color::RED.alpha(0.5), road.get_thick_polygon());

            for i in [road.src_i, road.dst_i] {
                if seen_borders.contains(&i) {
                    continue;
                }
                seen_borders.insert(i);
                batch
                    .unzoomed
                    .push(Color::RED.alpha(0.6), app.map.get_i(i).polygon.clone());
                batch
                    .zoomed
                    .push(Color::RED.alpha(0.5), app.map.get_i(i).polygon.clone());
            }
        }
    }
    batch.build(ctx)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Style {
    SimpleColoring,
    Cells,
    Quietness,
    RatRuns,
}

fn impact_widget(ctx: &EventCtx, app: &App) -> Widget {
    let map_name = app.map.get_name();

    if &app.session.impact.map != map_name {
        // Starting from scratch
        let scenario_name = Scenario::default_scenario_for_map(map_name);
        if scenario_name == "home_to_work" {
            return "This city doesn't have travel demand model data available".text_widget(ctx);
        }
        let size = abstio::Manifest::load()
            .get_entry(&abstio::path_scenario(map_name, &scenario_name))
            .map(|entry| abstutil::prettyprint_bytes(entry.compressed_size_bytes))
            .unwrap_or_else(|| "???".to_string());
        return Widget::col(vec![
            Text::from_multiline(vec![
                Line("Predicting impact of your proposal may take a moment."),
                Line("The application may freeze up during that time."),
                Line(format!("We need to load a {} file", size)),
            ])
            .into_widget(ctx),
            ctx.style()
                .btn_solid_primary
                .text("Calculate")
                .build_def(ctx),
        ]);
    }

    if app.session.impact.change_key == app.session.modal_filters.change_key {
        // Nothing to calculate!
        return ctx
            .style()
            .btn_solid_primary
            .text("Show impact")
            .build_def(ctx);
    }

    // We'll need to do some pathfinding
    Widget::col(vec![
        Text::from_multiline(vec![
            Line("Predicting impact of your proposal may take a moment."),
            Line("The application may freeze up during that time."),
        ])
        .into_widget(ctx),
        ctx.style()
            .btn_solid_primary
            .text("Calculate")
            .build_def(ctx),
    ])
}
