use geom::{Angle, ArrowCap, Distance, PolyLine};
use widgetry::mapspace::World;
use widgetry::{
    DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Key, Outcome, Panel, State, TextExt, Toggle, Widget,
};

use super::auto::Heuristic;
use super::per_neighborhood::{FilterableObj, Tab};
use super::{Neighborhood, NeighborhoodID};
use crate::{App, Transition};

pub struct Viewer {
    panel: Panel,
    neighborhood: Neighborhood,
    world: World<FilterableObj>,
}

impl Viewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App, id: NeighborhoodID) -> Box<dyn State<App>> {
        let neighborhood = Neighborhood::new(ctx, app, id);

        let mut viewer = Viewer {
            panel: Panel::empty(ctx),
            neighborhood,
            world: World::unbounded(),
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
                    Widget::row(vec![
                        "Draw entrances/exits as".text_widget(ctx).centered_vert(),
                        Toggle::choice(
                            ctx,
                            "draw borders",
                            "arrows",
                            "outlines",
                            Key::E,
                            app.session.draw_borders_as_arrows,
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
                ]),
            )
            .build(ctx);

        self.world = make_world(ctx, app, &self.neighborhood);
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
                }

                return Tab::Connectivity
                    .handle_action(ctx, app, x.as_ref(), self.neighborhood.id)
                    .unwrap();
            }
            Outcome::Changed(x) => {
                app.session.draw_cells_as_areas = self.panel.is_checked("draw cells");
                app.session.draw_borders_as_arrows = self.panel.is_checked("draw borders");
                app.session.heuristic = self.panel.dropdown_value("heuristic");

                if x != "heuristic" {
                    self.world = make_world(ctx, app, &self.neighborhood);
                }
            }
            _ => {}
        }

        let world_outcome = self.world.event(ctx);
        if super::per_neighborhood::handle_world_outcome(ctx, app, world_outcome) {
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

        self.panel.draw(g);
        self.neighborhood.draw_filters.draw(g);
        // TODO Since we cover such a small area, treating multiple segments of one road as the
        // same might be nice. And we should seed the quadtree with the locations of filters and
        // arrows, possibly.
        if g.canvas.is_unzoomed() {
            self.neighborhood.labels.draw(g, app);
        }
    }
}

fn make_world(ctx: &mut EventCtx, app: &App, neighborhood: &Neighborhood) -> World<FilterableObj> {
    let map = &app.map;
    let mut world = World::bounded(map.get_bounds());

    super::per_neighborhood::populate_world(ctx, app, neighborhood, &mut world, |id| id, 0);

    let render_cells = super::draw_cells::RenderCells::new(map, neighborhood);
    if app.session.draw_cells_as_areas {
        world.draw_master_batch(ctx, render_cells.draw());
    } else {
        let mut draw = GeomBatch::new();
        for (idx, cell) in neighborhood.cells.iter().enumerate() {
            let color = render_cells.colors[idx].alpha(0.9);
            for (r, interval) in &cell.roads {
                let road = map.get_r(*r);
                draw.push(
                    color,
                    road.center_pts
                        .exact_slice(interval.start, interval.end)
                        .make_polygons(road.get_width()),
                );
            }
            for i in
                map_gui::tools::intersections_from_roads(&cell.roads.keys().cloned().collect(), map)
            {
                draw.push(color, map.get_i(i).polygon.clone());
            }
        }
        world.draw_master_batch(ctx, draw);
    }

    // Draw the borders of each cell
    let mut draw = GeomBatch::new();
    for (idx, cell) in neighborhood.cells.iter().enumerate() {
        let color = render_cells.colors[idx];
        for i in &cell.borders {
            if app.session.draw_borders_as_arrows {
                let angles: Vec<Angle> = cell
                    .roads
                    .keys()
                    .filter_map(|r| {
                        let road = map.get_r(*r);
                        // Design choice: when we have a filter right at the entrance of a
                        // neighborhood, it creates its own little cell allowing access to just the
                        // very beginning of the road. Let's not draw arrows for that.
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
                draw.push(
                    color.alpha(0.8),
                    PolyLine::must_new(vec![
                        center.project_away(Distance::meters(30.0), angle.opposite()),
                        center.project_away(Distance::meters(10.0), angle.opposite()),
                    ])
                    .make_arrow(Distance::meters(6.0), ArrowCap::Triangle),
                );
            } else if let Ok(p) = map.get_i(*i).polygon.to_outline(Distance::meters(2.0)) {
                draw.push(color, p);
            }
        }
    }
    world.draw_master_batch(ctx, draw);

    world.initialize_hover(ctx);

    world
}
