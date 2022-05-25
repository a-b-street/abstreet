use geom::{ArrowCap, Distance, PolyLine};
use map_gui::tools::ColorNetwork;
use widgetry::mapspace::ToggleZoomed;
use widgetry::tools::PopupMsg;
use widgetry::{
    DrawBaselayer, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, TextExt, Toggle, Widget,
};

use crate::edit::{EditNeighborhood, Tab};
use crate::filters::auto::Heuristic;
use crate::shortcuts::find_shortcuts;
use crate::{colors, App, Neighborhood, NeighborhoodID, Transition};

pub struct Viewer {
    top_panel: Panel,
    left_panel: Panel,
    neighborhood: Neighborhood,
    draw_top_layer: ToggleZoomed,
    edit: EditNeighborhood,
}

impl Viewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App, id: NeighborhoodID) -> Box<dyn State<App>> {
        let neighborhood = Neighborhood::new(ctx, app, id);

        let mut viewer = Viewer {
            top_panel: crate::components::TopPanel::panel(ctx, app),
            left_panel: Panel::empty(ctx),
            neighborhood,
            draw_top_layer: ToggleZoomed::empty(ctx),
            edit: EditNeighborhood::temporary(),
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

        self.left_panel = self
            .edit
            .panel_builder(
                ctx,
                app,
                Tab::Connectivity,
                &self.top_panel,
                Widget::col(vec![
                    format!(
                        "Neighborhood area: {}",
                        app.session
                            .partitioning
                            .neighborhood_area_km2(self.neighborhood.id)
                    )
                    .text_widget(ctx),
                    warning.text_widget(ctx),
                    advanced_panel(ctx, app),
                ]),
            )
            .build(ctx);

        let (edit, draw_top_layer) = setup_editing(ctx, app, &self.neighborhood);
        self.edit = edit;
        self.draw_top_layer = draw_top_layer;
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) = crate::components::TopPanel::event(ctx, app, &mut self.top_panel, help) {
            return t;
        }
        match self.left_panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "Automatically place filters" {
                    match ctx.loading_screen("automatically filter a neighborhood", |ctx, timer| {
                        app.session
                            .heuristic
                            .apply(ctx, app, &self.neighborhood, timer)
                    }) {
                        Ok(()) => {
                            self.neighborhood = Neighborhood::new(ctx, app, self.neighborhood.id);
                            self.update(ctx, app);
                            return Transition::Keep;
                        }
                        Err(err) => {
                            return Transition::Push(PopupMsg::new_state(
                                ctx,
                                "Error",
                                vec![err.to_string()],
                            ));
                        }
                    }
                } else if let Some(t) = self.edit.handle_panel_action(
                    ctx,
                    app,
                    x.as_ref(),
                    &self.neighborhood,
                    &self.left_panel,
                ) {
                    return t;
                }

                return crate::save::AltProposals::handle_action(
                    ctx,
                    app,
                    crate::save::PreserveState::Connectivity(
                        app.session
                            .partitioning
                            .all_blocks_in_neighborhood(self.neighborhood.id),
                    ),
                    &x,
                )
                .unwrap();
            }
            Outcome::Changed(x) => {
                if x == "Advanced features" {
                    app.opts.dev = self.left_panel.is_checked("Advanced features");
                    self.update(ctx, app);
                    return Transition::Keep;
                }

                app.session.draw_cells_as_areas = self.left_panel.is_checked("draw cells");
                app.session.heuristic = self.left_panel.dropdown_value("heuristic");

                if x != "heuristic" {
                    let (edit, draw_top_layer) = setup_editing(ctx, app, &self.neighborhood);
                    self.edit = edit;
                    self.draw_top_layer = draw_top_layer;
                }
            }
            _ => {}
        }

        if self.edit.event(ctx, app) {
            self.neighborhood = Neighborhood::new(ctx, app, self.neighborhood.id);
            self.update(ctx, app);
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        crate::draw_with_layering(g, app, |g| self.edit.world.draw(g));
        g.redraw(&self.neighborhood.fade_irrelevant);
        self.draw_top_layer.draw(g);

        self.top_panel.draw(g);
        self.left_panel.draw(g);
        app.session.draw_all_filters.draw(g);
        // TODO Since we cover such a small area, treating multiple segments of one road as the
        // same might be nice. And we should seed the quadtree with the locations of filters and
        // arrows, possibly.
        if g.canvas.is_unzoomed() {
            self.neighborhood.labels.draw(g, app);
        }
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app, self.neighborhood.id)
    }
}

fn setup_editing(
    ctx: &mut EventCtx,
    app: &App,
    neighborhood: &Neighborhood,
) -> (EditNeighborhood, ToggleZoomed) {
    let shortcuts = ctx.loading_screen("find shortcuts", |_, timer| {
        find_shortcuts(app, neighborhood, timer)
    });

    let mut edit = EditNeighborhood::new(ctx, app, neighborhood, &shortcuts);
    let map = &app.map;

    // The world is drawn in between areas and roads, but some things need to be drawn on top of
    // roads
    let mut draw_top_layer = ToggleZoomed::builder();

    let render_cells = crate::draw_cells::RenderCells::new(map, neighborhood);
    if app.session.draw_cells_as_areas {
        edit.world.draw_master_batch(ctx, render_cells.draw());

        let mut colorer = ColorNetwork::no_fading(app);
        colorer.ranked_roads(shortcuts.count_per_road.clone(), &app.cs.good_to_bad_red);
        // TODO These two will be on different scales, which'll look really weird!
        colorer.ranked_intersections(
            shortcuts.count_per_intersection.clone(),
            &app.cs.good_to_bad_red,
        );

        draw_top_layer.append(colorer.draw);
    } else {
        for (idx, cell) in neighborhood.cells.iter().enumerate() {
            let color = render_cells.colors[idx].alpha(0.9);
            for (r, interval) in &cell.roads {
                let road = map.get_r(*r);
                draw_top_layer = draw_top_layer.push(
                    color,
                    road.center_pts
                        .exact_slice(interval.start, interval.end)
                        .make_polygons(road.get_width()),
                );
            }
            for i in
                map_gui::tools::intersections_from_roads(&cell.roads.keys().cloned().collect(), map)
            {
                draw_top_layer = draw_top_layer.push(color, map.get_i(i).polygon.clone());
            }
        }
    }

    // Draw the borders of each cell
    for (idx, cell) in neighborhood.cells.iter().enumerate() {
        let color = render_cells.colors[idx];
        for i in &cell.borders {
            // Most borders only have one road in the interior of the neighborhood. Draw an arrow
            // for each of those. If there happen to be multiple interior roads for one border, the
            // arrows will overlap each other -- but that happens anyway with borders close
            // together at certain angles.
            for r in cell.roads.keys() {
                let road = map.get_r(*r);
                // Design choice: when we have a filter right at the entrance of a neighborhood, it
                // creates its own little cell allowing access to just the very beginning of the
                // road. Let's not draw anything for that.
                if app.session.modal_filters.roads.contains_key(r) {
                    continue;
                }

                // Find the angle pointing into the neighborhood
                let angle_in = if road.src_i == *i {
                    road.center_pts.first_line().angle()
                } else if road.dst_i == *i {
                    road.center_pts.last_line().angle().opposite()
                } else {
                    // This interior road isn't connected to this border
                    continue;
                };

                let center = map.get_i(*i).polygon.center();
                let pt_farther = center.project_away(Distance::meters(40.0), angle_in.opposite());
                let pt_closer = center.project_away(Distance::meters(10.0), angle_in.opposite());

                // The arrow direction depends on if the road is one-way
                let thickness = Distance::meters(6.0);
                let arrow = if road.is_oneway() {
                    if road.src_i == *i {
                        PolyLine::must_new(vec![pt_farther, pt_closer])
                            .make_arrow(thickness, ArrowCap::Triangle)
                    } else {
                        PolyLine::must_new(vec![pt_closer, pt_farther])
                            .make_arrow(thickness, ArrowCap::Triangle)
                    }
                } else {
                    // Order doesn't matter
                    PolyLine::must_new(vec![pt_closer, pt_farther])
                        .make_double_arrow(thickness, ArrowCap::Triangle)
                };
                draw_top_layer = draw_top_layer.push(color.alpha(1.0), arrow);
            }
        }
    }

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
                    draw_top_layer.unzoomed.push(colors::OUTLINE, poly);
                }
            }
        }
    }

    (edit, draw_top_layer.build(ctx))
}

fn help() -> Vec<&'static str> {
    vec![
        "The colored cells show where it's possible to drive without leaving the neighborhood.",
        "",
        "The darker red roads have more predicted shortcutting traffic.",
        "",
        "Hint: You can place filters at roads or intersections.",
        "Use the lasso tool to quickly sketch your idea.",
    ]
}

fn advanced_panel(ctx: &EventCtx, app: &App) -> Widget {
    if app.session.consultation.is_some() {
        return Widget::nothing();
    }
    if !app.opts.dev {
        return Toggle::checkbox(ctx, "Advanced features", None, app.opts.dev);
    }
    Widget::col(vec![
        Toggle::checkbox(ctx, "Advanced features", None, app.opts.dev),
        Line("Advanced features").small_heading().into_widget(ctx),
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
        ctx.style()
            .btn_outline
            .text("Automatically place filters")
            .hotkey(Key::A)
            .build_def(ctx),
        Widget::dropdown(
            ctx,
            "heuristic",
            app.session.heuristic,
            Heuristic::choices(),
        ),
    ])
    .section(ctx)
}
