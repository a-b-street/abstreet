use std::collections::HashSet;

use abstutil::Counter;
use map_gui::tools::{ColorNetwork, DrawSimpleRoadLabels};
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{
    Choice, Color, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel,
    State, TextExt, Toggle, Widget,
};

use crate::components::Mode;
use crate::edit::EditMode;
use crate::filters::auto::Heuristic;
use crate::{colors, App, Neighbourhood, NeighbourhoodID, Transition};

pub struct BrowseNeighbourhoods {
    top_panel: Panel,
    left_panel: Panel,
    world: World<NeighbourhoodID>,
    draw_over_roads: Drawable,
    labels: DrawSimpleRoadLabels,
    draw_boundary_roads: Drawable,
}

impl BrowseNeighbourhoods {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        map_gui::tools::update_url_map_name(app);

        // Make sure we clear this state if we ever switch neighbourhoods
        if let EditMode::Shortcuts(ref mut maybe_focus) = app.session.edit_mode {
            *maybe_focus = None;
        }
        if let EditMode::FreehandFilters(_) = app.session.edit_mode {
            app.session.edit_mode = EditMode::Filters;
        }

        let (world, draw_over_roads) =
            ctx.loading_screen("calculate neighbourhoods", |ctx, timer| {
                if &app.per_map.partitioning.map != app.per_map.map.get_name() {
                    app.per_map.alt_proposals = crate::save::AltProposals::new();
                    crate::clear_current_proposal(ctx, app, timer);
                }
                (make_world(ctx, app), draw_over_roads(ctx, app))
            });

        let top_panel = crate::components::TopPanel::panel(ctx, app, Mode::BrowseNeighbourhoods);
        let left_panel = crate::components::LeftPanel::builder(
            ctx,
            &top_panel,
            Widget::col(vec![
                Toggle::checkbox(ctx, "Advanced features", None, app.opts.dev),
                advanced_panel(ctx, app),
            ]),
        )
        .build(ctx);
        Box::new(BrowseNeighbourhoods {
            top_panel,
            left_panel,
            world,
            draw_over_roads,
            labels: DrawSimpleRoadLabels::only_major_roads(ctx, app, colors::ROAD_LABEL),
            draw_boundary_roads: draw_boundary_roads(ctx, app),
        })
    }
}

impl State<App> for BrowseNeighbourhoods {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) = crate::components::TopPanel::event(
            ctx,
            app,
            &mut self.top_panel,
            &crate::save::PreserveState::BrowseNeighbourhoods,
            help,
        ) {
            return t;
        }
        if let Some(t) = app
            .session
            .layers
            .event(ctx, &app.cs, Mode::BrowseNeighbourhoods)
        {
            return t;
        }
        match self.left_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Automatically place filters" => {
                    ctx.loading_screen("automatically filter all neighbourhoods", |ctx, timer| {
                        timer.start_iter(
                            "filter neighbourhood",
                            app.per_map.partitioning.all_neighbourhoods().len(),
                        );
                        for id in app
                            .per_map
                            .partitioning
                            .all_neighbourhoods()
                            .keys()
                            .cloned()
                            .collect::<Vec<_>>()
                        {
                            timer.next();
                            let neighbourhood = Neighbourhood::new(ctx, app, id);
                            // Ignore errors
                            let _ = app.session.heuristic.apply(ctx, app, &neighbourhood, timer);
                        }
                    });
                    return Transition::Replace(BrowseNeighbourhoods::new_state(ctx, app));
                }
                _ => unreachable!(),
            },
            Outcome::Changed(x) => {
                if x == "Advanced features" {
                    app.opts.dev = self.left_panel.is_checked("Advanced features");
                    return Transition::Replace(BrowseNeighbourhoods::new_state(ctx, app));
                }
                if x == "heuristic" {
                    app.session.heuristic = self.left_panel.dropdown_value("heuristic");
                } else if x == "style" {
                    app.session.draw_neighbourhood_style = self.left_panel.dropdown_value("style");

                    ctx.loading_screen("change style", |ctx, _| {
                        self.world = make_world(ctx, app);
                        self.draw_over_roads = draw_over_roads(ctx, app);
                    });
                }
            }
            _ => {}
        }

        if let WorldOutcome::ClickedObject(id) = self.world.event(ctx) {
            return Transition::Push(crate::connectivity::Viewer::new_state(ctx, app, id));
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        app.draw_with_layering(g, |g| self.world.draw(g));
        self.draw_over_roads.draw(g);

        self.top_panel.draw(g);
        self.left_panel.draw(g);
        app.session.layers.draw(g, app);
        self.draw_boundary_roads.draw(g);
        self.labels.draw(g);
        app.per_map.draw_all_filters.draw(g);
        app.per_map.draw_poi_icons.draw(g);
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app)
    }
}

fn make_world(ctx: &mut EventCtx, app: &App) -> World<NeighbourhoodID> {
    let mut world = World::bounded(app.per_map.map.get_bounds());
    let map = &app.per_map.map;
    ctx.loading_screen("render neighbourhoods", |ctx, timer| {
        timer.start_iter(
            "render neighbourhoods",
            app.per_map.partitioning.all_neighbourhoods().len(),
        );
        for (id, info) in app.per_map.partitioning.all_neighbourhoods() {
            timer.next();
            match app.session.draw_neighbourhood_style {
                Style::Simple => {
                    world
                        .add(*id)
                        .hitbox(info.block.polygon.clone())
                        .draw_color(Color::YELLOW.alpha(0.1))
                        .hover_alpha(0.5)
                        .clickable()
                        .build(ctx);
                }
                Style::Cells => {
                    let neighbourhood = Neighbourhood::new(ctx, app, *id);
                    let render_cells = crate::draw_cells::RenderCells::new(map, &neighbourhood);
                    let hovered_batch = render_cells.draw_colored_areas();
                    world
                        .add(*id)
                        .hitbox(info.block.polygon.clone())
                        .drawn_in_master_batch()
                        .draw_hovered(hovered_batch)
                        .clickable()
                        .build(ctx);
                }
                Style::Quietness => {
                    let neighbourhood = Neighbourhood::new(ctx, app, *id);
                    let (quiet_streets, total_streets) = neighbourhood
                        .shortcuts
                        .quiet_and_total_streets(&neighbourhood);
                    let pct = if total_streets == 0 {
                        0.0
                    } else {
                        1.0 - (quiet_streets as f64 / total_streets as f64)
                    };
                    let color = app.cs.good_to_bad_red.eval(pct);
                    world
                        .add(*id)
                        .hitbox(info.block.polygon.clone())
                        .draw_color(color.alpha(0.5))
                        .hover_color(colors::HOVER)
                        .clickable()
                        .build(ctx);
                }
                Style::Shortcuts => {
                    world
                        .add(*id)
                        .hitbox(info.block.polygon.clone())
                        // Slight lie, because draw_over_roads has to be drawn after the World
                        .drawn_in_master_batch()
                        .hover_color(colors::HOVER)
                        .clickable()
                        .build(ctx);
                }
            }
        }
    });
    world
}

fn draw_over_roads(ctx: &mut EventCtx, app: &App) -> Drawable {
    if app.session.draw_neighbourhood_style != Style::Shortcuts {
        return Drawable::empty(ctx);
    }

    let mut count_per_road = Counter::new();
    let mut count_per_intersection = Counter::new();

    for id in app.per_map.partitioning.all_neighbourhoods().keys() {
        let neighbourhood = Neighbourhood::new(ctx, app, *id);
        count_per_road.extend(neighbourhood.shortcuts.count_per_road);
        count_per_intersection.extend(neighbourhood.shortcuts.count_per_intersection);
    }

    // TODO It's a bit weird to draw one heatmap covering streets in every neighbourhood. The
    // shortcuts are calculated per neighbourhood, but now we're showing them all together, as if
    // it's the impact prediction mode using a demand model.
    let mut colorer = ColorNetwork::no_fading(app);
    colorer.ranked_roads(count_per_road, &app.cs.good_to_bad_red);
    colorer.ranked_intersections(count_per_intersection, &app.cs.good_to_bad_red);
    colorer.build(ctx).unzoomed
}

pub fn draw_boundary_roads(ctx: &EventCtx, app: &App) -> Drawable {
    let mut seen_roads = HashSet::new();
    let mut seen_borders = HashSet::new();
    let mut batch = GeomBatch::new();
    for info in app.per_map.partitioning.all_neighbourhoods().values() {
        for id in &info.block.perimeter.roads {
            let r = id.road;
            if seen_roads.contains(&r) {
                continue;
            }
            seen_roads.insert(r);
            let road = app.per_map.map.get_r(r);
            batch.push(colors::HIGHLIGHT_BOUNDARY, road.get_thick_polygon());

            for i in [road.src_i, road.dst_i] {
                if seen_borders.contains(&i) {
                    continue;
                }
                seen_borders.insert(i);
                batch.push(
                    colors::HIGHLIGHT_BOUNDARY,
                    app.per_map.map.get_i(i).polygon.clone(),
                );
            }
        }
    }
    batch.build(ctx)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Style {
    Simple,
    Cells,
    Quietness,
    Shortcuts,
}

fn help() -> Vec<&'static str> {
    vec![
        "Basic map navigation: click and drag to pan, swipe or scroll to zoom",
        "",
        "Click a neighbourhood to analyze it. You can adjust boundaries there.",
    ]
}

fn advanced_panel(ctx: &EventCtx, app: &App) -> Widget {
    if !app.opts.dev {
        return Widget::nothing();
    }
    Widget::col(vec![
        Line("Advanced features").small_heading().into_widget(ctx),
        Widget::col(vec![Widget::row(vec![
            "Draw neighbourhoods:".text_widget(ctx).centered_vert(),
            Widget::dropdown(
                ctx,
                "style",
                app.session.draw_neighbourhood_style,
                vec![
                    Choice::new("simple", Style::Simple),
                    Choice::new("cells", Style::Cells),
                    Choice::new("quietness", Style::Quietness),
                    Choice::new("all shortcuts", Style::Shortcuts),
                ],
            ),
        ])])
        .section(ctx),
        Widget::col(vec![
            ctx.style()
                .btn_outline
                .text("Automatically place filters")
                .build_def(ctx),
            Widget::dropdown(
                ctx,
                "heuristic",
                app.session.heuristic,
                Heuristic::choices(),
            ),
        ])
        .section(ctx),
    ])
}
