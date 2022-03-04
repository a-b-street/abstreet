use geom::{Angle, ArrowCap, Distance, PolyLine};
use map_model::Perimeter;
use widgetry::mapspace::{ToggleZoomed, World};
use widgetry::tools::PolyLineLasso;
use widgetry::{
    Color, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, ScreenPt, State,
    Text, TextExt, Toggle, Widget,
};

use crate::filters::auto::Heuristic;
use crate::per_neighborhood::{FilterableObj, Tab};
use crate::{after_edit, App, Neighborhood, NeighborhoodID, Transition};

pub struct Viewer {
    panel: Panel,
    neighborhood: Neighborhood,
    world: World<FilterableObj>,
    draw_top_layer: ToggleZoomed,
}

impl Viewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App, id: NeighborhoodID) -> Box<dyn State<App>> {
        let neighborhood = Neighborhood::new(ctx, app, id);

        let mut viewer = Viewer {
            panel: Panel::empty(ctx),
            neighborhood,
            world: World::unbounded(),
            draw_top_layer: ToggleZoomed::empty(ctx),
        };
        viewer.update(ctx, app);
        Box::new(viewer)
    }

    fn update(&mut self, ctx: &mut EventCtx, app: &App) {
        let disconnected_cells = self
            .neighborhood
            .cells
            .iter()
            .filter(|c| c.is_disconnected())
            .count();
        let warning = if disconnected_cells == 0 {
            String::new()
        } else {
            format!("{} cells are totally disconnected", disconnected_cells)
        };

        self.panel = Tab::Connectivity
            .panel_builder(
                ctx,
                app,
                Widget::col(vec![
                    Widget::row(vec![
                        "Draw traffic cells as".text_widget(ctx).centered_vert(),
                        Toggle::choice(
                            ctx,
                            "draw cells",
                            "areas",
                            "streets",
                            Key::D,
                            app.session.draw_cells_as_areas,
                        ),
                    ]),
                    warning.text_widget(ctx),
                    Widget::row(vec![
                        Widget::dropdown(
                            ctx,
                            "heuristic",
                            app.session.heuristic,
                            Heuristic::choices(),
                        ),
                        ctx.style()
                            .btn_outline
                            .text("Automatically stop rat-runs")
                            .hotkey(Key::A)
                            .build_def(ctx),
                    ]),
                    ctx.style()
                        .btn_outline
                        .icon_text(
                            "system/assets/tools/select.svg",
                            "Create filters along a shape",
                        )
                        .hotkey(Key::F)
                        .build_def(ctx),
                ]),
            )
            .build(ctx);

        let (world, draw_top_layer) = make_world(ctx, app, &self.neighborhood);
        self.world = world;
        self.draw_top_layer = draw_top_layer;
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "Automatically stop rat-runs" {
                    ctx.loading_screen("automatically filter a neighborhood", |ctx, timer| {
                        app.session
                            .heuristic
                            .apply(ctx, app, &self.neighborhood, timer);
                    });
                    self.neighborhood = Neighborhood::new(ctx, app, self.neighborhood.id);
                    self.update(ctx, app);
                    return Transition::Keep;
                } else if x == "Create filters along a shape" {
                    return Transition::Push(FreehandFilters::new_state(
                        ctx,
                        &self.neighborhood,
                        self.panel.center_of("Create filters along a shape"),
                    ));
                }

                return Tab::Connectivity
                    .handle_action(ctx, app, x.as_ref(), self.neighborhood.id)
                    .unwrap();
            }
            Outcome::Changed(x) => {
                app.session.draw_cells_as_areas = self.panel.is_checked("draw cells");
                app.session.heuristic = self.panel.dropdown_value("heuristic");

                if x != "heuristic" {
                    let (world, draw_top_layer) = make_world(ctx, app, &self.neighborhood);
                    self.world = world;
                    self.draw_top_layer = draw_top_layer;
                }
            }
            _ => {}
        }

        let world_outcome = self.world.event(ctx);
        if crate::per_neighborhood::handle_world_outcome(ctx, app, world_outcome) {
            self.neighborhood = Neighborhood::new(ctx, app, self.neighborhood.id);
            self.update(ctx, app);
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        crate::draw_with_layering(g, app, |g| self.world.draw(g));
        g.redraw(&self.neighborhood.fade_irrelevant);
        self.draw_top_layer.draw(g);

        self.panel.draw(g);
        app.session.draw_all_filters.draw(g);
        // TODO Since we cover such a small area, treating multiple segments of one road as the
        // same might be nice. And we should seed the quadtree with the locations of filters and
        // arrows, possibly.
        if g.canvas.is_unzoomed() {
            self.neighborhood.labels.draw(g, app);
        }
    }
}

fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighborhood: &Neighborhood,
) -> (World<FilterableObj>, ToggleZoomed) {
    let map = &app.map;
    let mut world = World::bounded(map.get_bounds());

    crate::per_neighborhood::populate_world(ctx, app, neighborhood, &mut world, |id| id, 0);

    // The world is drawn in between areas and roads, but some things need to be drawn on top of
    // roads
    let mut draw_top_layer = GeomBatch::new();

    let render_cells = crate::draw_cells::RenderCells::new(map, neighborhood);
    if app.session.draw_cells_as_areas {
        world.draw_master_batch(ctx, render_cells.draw());
    } else {
        for (idx, cell) in neighborhood.cells.iter().enumerate() {
            let color = render_cells.colors[idx].alpha(0.9);
            for (r, interval) in &cell.roads {
                let road = map.get_r(*r);
                draw_top_layer.push(
                    color,
                    road.center_pts
                        .exact_slice(interval.start, interval.end)
                        .make_polygons(road.get_width()),
                );
            }
            for i in
                map_gui::tools::intersections_from_roads(&cell.roads.keys().cloned().collect(), map)
            {
                draw_top_layer.push(color, map.get_i(i).polygon.clone());
            }
        }
    }

    // Draw the borders of each cell
    for (idx, cell) in neighborhood.cells.iter().enumerate() {
        let color = render_cells.colors[idx];
        for i in &cell.borders {
            let angles: Vec<Angle> = cell
                .roads
                .keys()
                .filter_map(|r| {
                    let road = map.get_r(*r);
                    // Design choice: when we have a filter right at the entrance of a
                    // neighborhood, it creates its own little cell allowing access to just the
                    // very beginning of the road. Let's not draw anything for that.
                    if app.session.modal_filters.roads.contains_key(r) {
                        None
                    } else if road.src_i == *i {
                        Some(road.center_pts.first_line().angle())
                    } else if road.dst_i == *i {
                        Some(road.center_pts.last_line().angle().opposite())
                    } else {
                        None
                    }
                })
                .collect();
            // Tiny cell with a filter right at the border
            if angles.is_empty() {
                continue;
            }

            let center = map.get_i(*i).polygon.center();
            let angle = Angle::average(angles);

            // TODO Consider showing borders with one-way roads. For now, always point the
            // arrow into the neighborhood
            draw_top_layer.push(
                color.alpha(0.8),
                PolyLine::must_new(vec![
                    center.project_away(Distance::meters(30.0), angle.opposite()),
                    center.project_away(Distance::meters(10.0), angle.opposite()),
                ])
                .make_arrow(Distance::meters(6.0), ArrowCap::Triangle),
            );
        }
    }

    let mut top_layer = ToggleZoomed::builder();
    top_layer.unzoomed = draw_top_layer.clone();
    top_layer.zoomed = draw_top_layer.clone();

    // Draw one-way arrows
    for r in neighborhood
        .orig_perimeter
        .interior
        .iter()
        .chain(neighborhood.orig_perimeter.roads.iter().map(|id| &id.road))
    {
        let road = map.get_r(*r);
        if road.osm_tags.is("oneway", "yes") {
            let arrow_len = Distance::meters(10.0);
            let thickness = Distance::meters(1.0);
            for (pt, angle) in road
                .center_pts
                .step_along(Distance::meters(30.0), Distance::meters(5.0))
            {
                if let Ok(poly) = PolyLine::must_new(vec![
                    pt.project_away(arrow_len / 2.0, angle.opposite()),
                    pt.project_away(arrow_len / 2.0, angle),
                ])
                .make_arrow(thickness * 2.0, ArrowCap::Triangle)
                .to_outline(thickness / 2.0)
                {
                    top_layer.unzoomed.push(Color::BLACK, poly);
                }
            }
        }
    }

    world.initialize_hover(ctx);

    (world, top_layer.build(ctx))
}

struct FreehandFilters {
    lasso: PolyLineLasso,
    id: NeighborhoodID,
    perimeter: Perimeter,
    instructions: Text,
    instructions_at: ScreenPt,
}

impl FreehandFilters {
    fn new_state(
        ctx: &EventCtx,
        neighborhood: &Neighborhood,
        instructions_at: ScreenPt,
    ) -> Box<dyn State<App>> {
        Box::new(Self {
            lasso: PolyLineLasso::new(),
            id: neighborhood.id,
            perimeter: neighborhood.orig_perimeter.clone(),
            instructions_at,
            instructions: Text::from_all(vec![
                Line("Click and drag").fg(ctx.style().text_hotkey_color),
                Line(" across the roads you want to flter"),
            ]),
        })
    }
}

impl State<App> for FreehandFilters {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(pl) = self.lasso.event(ctx) {
            make_filters_along_path(ctx, app, &self.perimeter, pl);
            return Transition::Multi(vec![
                Transition::Pop,
                Transition::Replace(Viewer::new_state(ctx, app, self.id)),
            ]);
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.lasso.draw(g);
        // Hacky, but just draw instructions over the other panel
        g.draw_tooltip_at(self.instructions.clone(), self.instructions_at);
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }
}

fn make_filters_along_path(
    ctx: &mut EventCtx,
    app: &mut App,
    perimeter: &Perimeter,
    path: PolyLine,
) {
    app.session.modal_filters.before_edit();
    for r in &perimeter.interior {
        if app.session.modal_filters.roads.contains_key(r) {
            continue;
        }
        let road = app.map.get_r(*r);
        if let Some((pt, _)) = road.center_pts.intersection(&path) {
            let dist = road
                .center_pts
                .dist_along_of_point(pt)
                .map(|pair| pair.0)
                .unwrap_or(road.center_pts.length() / 2.0);
            app.session.modal_filters.roads.insert(*r, dist);
        }
    }
    after_edit(ctx, app);
}
