use std::collections::BTreeSet;

use geom::{ArrowCap, Distance, PolyLine};
use map_gui::tools::ColorNetwork;
use map_model::PathConstraints;
use raw_map::Direction;
use widgetry::mapspace::ToggleZoomed;
use widgetry::tools::PopupMsg;
use widgetry::{
    Color, ControlState, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome,
    Panel, RewriteColor, State, TextExt, Toggle, Widget,
};

use crate::draw_cells::RenderCells;
use crate::edit::{EditNeighbourhood, EditOutcome, Tab};
use crate::filters::auto::Heuristic;
use crate::shortcuts::find_shortcuts;
use crate::{colors, App, Neighbourhood, NeighbourhoodID, Transition};

pub struct Viewer {
    top_panel: Panel,
    left_panel: Panel,
    neighbourhood: Neighbourhood,
    draw_top_layer: ToggleZoomed,
    edit: EditNeighbourhood,

    show_error: Drawable,
}

impl Viewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App, id: NeighbourhoodID) -> Box<dyn State<App>> {
        let neighbourhood = Neighbourhood::new(ctx, app, id);

        let mut viewer = Viewer {
            top_panel: crate::components::TopPanel::panel(ctx, app),
            left_panel: Panel::empty(ctx),
            neighbourhood,
            draw_top_layer: ToggleZoomed::empty(ctx),
            edit: EditNeighbourhood::temporary(),
            show_error: Drawable::empty(ctx),
        };
        viewer.update(ctx, app);
        Box::new(viewer)
    }

    fn update(&mut self, ctx: &mut EventCtx, app: &App) {
        let (edit, draw_top_layer, render_cells) = setup_editing(ctx, app, &self.neighbourhood);
        self.edit = edit;
        self.draw_top_layer = draw_top_layer;

        let mut show_error = GeomBatch::new();
        let mut filter_problems = false;
        for (idx, cell) in self.neighbourhood.cells.iter().enumerate() {
            if cell.is_disconnected() {
                filter_problems = true;
                show_error.extend(
                    Color::RED.alpha(0.8),
                    render_cells.polygons_per_cell[idx].clone(),
                );
            }
        }

        let oneway_problems = detect_oneway_blackholes(app, &self.neighbourhood, &mut show_error);

        let warning = if !filter_problems && !oneway_problems {
            Widget::nothing()
        } else {
            let msg = if !filter_problems {
                "Some areas unreachable due to one-way streets"
            } else if !oneway_problems {
                "Some areas unreachable due to filters"
            } else {
                "Some areas unreachable due to one-way streets & filters"
            };

            ctx.style()
                .btn_plain
                .icon_text("system/assets/tools/warning.svg", msg)
                .label_color(Color::RED, ControlState::Default)
                .no_tooltip()
                .build_widget(ctx, "warning")
        };
        self.show_error = ctx.upload(show_error);

        self.left_panel = self
            .edit
            .panel_builder(
                ctx,
                app,
                Tab::Connectivity,
                &self.top_panel,
                Widget::col(vec![
                    format!(
                        "Neighbourhood area: {}",
                        app.session
                            .partitioning
                            .neighbourhood_area_km2(self.neighbourhood.id)
                    )
                    .text_widget(ctx),
                    warning,
                    advanced_panel(ctx, app),
                ]),
            )
            .build(ctx);
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
                    match ctx.loading_screen(
                        "automatically filter a neighbourhood",
                        |ctx, timer| {
                            app.session
                                .heuristic
                                .apply(ctx, app, &self.neighbourhood, timer)
                        },
                    ) {
                        Ok(()) => {
                            self.neighbourhood =
                                Neighbourhood::new(ctx, app, self.neighbourhood.id);
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
                } else if x == "Customize boundary" {
                    return Transition::Push(
                        crate::customize_boundary::CustomizeBoundary::new_state(
                            ctx,
                            app,
                            self.neighbourhood.id,
                        ),
                    );
                } else if x == "warning" {
                    // Not really clickable
                    return Transition::Keep;
                } else if let Some(t) = self.edit.handle_panel_action(
                    ctx,
                    app,
                    x.as_ref(),
                    &self.neighbourhood,
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
                            .all_blocks_in_neighbourhood(self.neighbourhood.id),
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
                    let (edit, draw_top_layer, _) = setup_editing(ctx, app, &self.neighbourhood);
                    self.edit = edit;
                    self.draw_top_layer = draw_top_layer;
                }
            }
            _ => {}
        }

        match self.edit.event(ctx, app) {
            EditOutcome::Nothing => {}
            EditOutcome::Recalculate => {
                self.neighbourhood = Neighbourhood::new(ctx, app, self.neighbourhood.id);
                self.update(ctx, app);
            }
            EditOutcome::Transition(t) => {
                return t;
            }
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        crate::draw_with_layering(g, app, |g| self.edit.world.draw(g));
        g.redraw(&self.neighbourhood.fade_irrelevant);
        self.draw_top_layer.draw(g);

        self.top_panel.draw(g);
        self.left_panel.draw(g);
        app.session.draw_all_filters.draw(g);
        // TODO Since we cover such a small area, treating multiple segments of one road as the
        // same might be nice. And we should seed the quadtree with the locations of filters and
        // arrows, possibly.
        if g.canvas.is_unzoomed() {
            self.neighbourhood.labels.draw(g, app);
        }

        if self.left_panel.currently_hovering() == Some(&"warning".to_string()) {
            g.redraw(&self.show_error);
        }
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app, self.neighbourhood.id)
    }
}

fn setup_editing(
    ctx: &mut EventCtx,
    app: &App,
    neighbourhood: &Neighbourhood,
) -> (EditNeighbourhood, ToggleZoomed, RenderCells) {
    let shortcuts = ctx.loading_screen("find shortcuts", |_, timer| {
        find_shortcuts(app, neighbourhood, timer)
    });

    let mut edit = EditNeighbourhood::new(ctx, app, neighbourhood, &shortcuts);
    let map = &app.map;

    // The world is drawn in between areas and roads, but some things need to be drawn on top of
    // roads
    let mut draw_top_layer = ToggleZoomed::builder();

    let render_cells = RenderCells::new(map, neighbourhood);
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
        for (idx, cell) in neighbourhood.cells.iter().enumerate() {
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

    // Draw a caution icon inside any disconnected cells
    for (idx, cell) in neighbourhood.cells.iter().enumerate() {
        if cell.is_disconnected() {
            for poly in &render_cells.polygons_per_cell[idx] {
                draw_top_layer.append_batch(
                    GeomBatch::load_svg(ctx, "system/assets/tools/warning.svg")
                        .color(RewriteColor::ChangeAll(Color::RED))
                        .scale(1.0)
                        .centered_on(poly.polylabel()),
                );
            }
        }
    }

    // Draw the borders of each cell
    for (idx, cell) in neighbourhood.cells.iter().enumerate() {
        let color = render_cells.colors[idx];
        for i in &cell.borders {
            // Most borders only have one road in the interior of the neighbourhood. Draw an arrow
            // for each of those. If there happen to be multiple interior roads for one border, the
            // arrows will overlap each other -- but that happens anyway with borders close
            // together at certain angles.
            for r in cell.roads.keys() {
                let road = map.get_r(*r);
                // Design choice: when we have a filter right at the entrance of a neighbourhood, it
                // creates its own little cell allowing access to just the very beginning of the
                // road. Let's not draw anything for that.
                if app.session.modal_filters.roads.contains_key(r) {
                    continue;
                }

                // Find the angle pointing into the neighbourhood
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
                let arrow = if let Some(dir) = road.oneway_for_driving() {
                    let pl = if road.src_i == *i {
                        PolyLine::must_new(vec![pt_farther, pt_closer])
                    } else {
                        PolyLine::must_new(vec![pt_closer, pt_farther])
                    };
                    pl.maybe_reverse(dir == Direction::Back)
                        .make_arrow(thickness, ArrowCap::Triangle)
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
    for r in neighbourhood
        .orig_perimeter
        .interior
        .iter()
        .chain(neighbourhood.orig_perimeter.roads.iter().map(|id| &id.road))
    {
        let road = map.get_r(*r);
        if let Some(dir) = road.oneway_for_driving() {
            let arrow_len = Distance::meters(10.0);
            let thickness = Distance::meters(1.0);
            for (pt, angle) in road
                .center_pts
                .step_along(Distance::meters(30.0), Distance::meters(5.0))
            {
                // If the user has made the one-way point opposite to how the road is originally
                // oriented, reverse the arrows
                let pl = PolyLine::must_new(vec![
                    pt.project_away(arrow_len / 2.0, angle.opposite()),
                    pt.project_away(arrow_len / 2.0, angle),
                ])
                .maybe_reverse(dir == Direction::Back);

                if let Ok(poly) = pl
                    .make_arrow(thickness * 2.0, ArrowCap::Triangle)
                    .to_outline(thickness / 2.0)
                {
                    draw_top_layer.unzoomed.push(colors::OUTLINE, poly);
                }
            }
        }
    }

    (edit, draw_top_layer.build(ctx), render_cells)
}

fn help() -> Vec<&'static str> {
    vec![
        "The colored cells show where it's possible to drive without leaving the neighbourhood.",
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
        ctx.style()
            .btn_outline
            .text("Customize boundary")
            .build_def(ctx),
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

// True if there are problems
fn detect_oneway_blackholes(
    app: &App,
    neighbourhood: &Neighbourhood,
    show_error: &mut GeomBatch,
) -> bool {
    // Only focus on problems in the current neighbourhood
    let relevant_roads: BTreeSet<_> = neighbourhood
        .orig_perimeter
        .interior
        .iter()
        .cloned()
        .collect();

    let (_, lanes) = map_model::connectivity::find_scc(&app.map, PathConstraints::Car);
    let mut problem_roads = BTreeSet::new();
    for l in lanes {
        let r = l.road;
        if relevant_roads.contains(&r) {
            problem_roads.insert(r);
        }
    }
    if problem_roads.is_empty() {
        return false;
    }

    for r in problem_roads {
        // TODO Red rat-runs
        show_error.push(Color::CYAN.alpha(0.5), app.map.get_r(r).get_thick_polygon());
    }
    true
}
