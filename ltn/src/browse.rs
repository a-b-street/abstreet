use std::collections::HashSet;

use abstutil::Timer;
use geom::Distance;
use map_gui::tools::{CityPicker, DrawRoadLabels, Navigator, PopupMsg, URLManager};
use synthpop::Scenario;
use widgetry::mapspace::{ToggleZoomed, World, WorldOutcome};
use widgetry::{
    Choice, Color, DrawBaselayer, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    Drawable, GeomBatch, State, Text, TextExt, Toggle, VerticalAlignment, Widget,
};

use super::{Neighborhood, NeighborhoodID, Partitioning};
use crate::{App, ModalFilters, Transition};

pub struct BrowseNeighborhoods {
    panel: Panel,
    world: World<NeighborhoodID>,
    labels: DrawRoadLabels,
    draw_all_filters: ToggleZoomed,
    draw_boundary_roads: ToggleZoomed,

    dark_buildings: Drawable,
}

impl BrowseNeighborhoods {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        URLManager::update_url_map_name(app);

        let world = ctx.loading_screen("calculate neighborhoods", |ctx, timer| {
            if &app.session.partitioning.map != app.map.get_name() {
                app.session.partitioning = Partitioning::seed_using_heuristics(app, timer);
                app.session.modal_filters = ModalFilters::default();
            }
            make_world(ctx, app, timer)
        });
        let draw_all_filters = app.session.modal_filters.draw(ctx, &app.map, None);

        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            Widget::row(vec![
                "Click a neighborhood".text_widget(ctx).centered_vert(),
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/search.svg")
                    .hotkey(Key::K)
                    .build_widget(ctx, "search")
                    .align_right(),
            ]),
            Toggle::checkbox(
                ctx,
                "highlight boundary roads",
                Key::H,
                app.session.highlight_boundary_roads,
            ),
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
                    ],
                ),
            ]),
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
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        Box::new(BrowseNeighborhoods {
            panel,
            world,
            labels: DrawRoadLabels::only_major_roads(),
            draw_all_filters,
            draw_boundary_roads: draw_boundary_roads(ctx, app),
            dark_buildings: draw_dark_buildings(ctx, app),
        })
    }
}

impl State<App> for BrowseNeighborhoods {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Home" => {
                    return Transition::Clear(vec![map_gui::tools::TitleScreen::new_state(
                        ctx,
                        app,
                        map_gui::tools::Executable::LTN,
                        Box::new(|ctx, app, _| BrowseNeighborhoods::new_state(ctx, app)),
                    )]);
                }
                "change map" => {
                    return Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Replace(BrowseNeighborhoods::new_state(ctx, app))
                        }),
                    ));
                }
                "search" => {
                    return Transition::Push(Navigator::new_state(ctx, app));
                }
                "New" => {
                    app.session.partitioning = Partitioning::empty();
                    app.session.modal_filters = ModalFilters::default();
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
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                app.session.highlight_boundary_roads =
                    self.panel.is_checked("highlight boundary roads");
                app.session.draw_neighborhood_style = self.panel.dropdown_value("style");

                self.world =
                    ctx.loading_screen("change style", |ctx, timer| make_world(ctx, app, timer));
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
        g.redraw(&self.dark_buildings);

        self.panel.draw(g);
        self.draw_all_filters.draw(g);
        if self.panel.is_checked("highlight boundary roads") {
            self.draw_boundary_roads.draw(g);
        }
        if g.canvas.is_unzoomed() {
            self.labels.draw(g, app);
        }
    }
}

fn draw_dark_buildings(ctx: &EventCtx, app: &App) -> Drawable {
    let mut road_to_color = std::collections::HashMap::new();
    for (_, (block, color)) in app.session.partitioning.all_neighborhoods() {
        for r in &block.perimeter.interior {
            road_to_color.insert(*r, *color);
        }
    }

    let mut bldgs = GeomBatch::new();
    for b in app.map.all_buildings() {
        if let Some(color) = road_to_color.get(&b.sidewalk().road) {
            bldgs.push(*color, b.polygon.clone());
        }
    }
    ctx.upload(bldgs)
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
                    .draw_color(color.alpha(0.5))
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
        }
    }
    world
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
                .push(Color::RED.alpha(0.8), road.get_thick_polygon());
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
                    .push(Color::RED.alpha(0.8), app.map.get_i(i).polygon.clone());
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
