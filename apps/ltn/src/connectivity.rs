use abstutil::Timer;
use geom::{ArrowCap, Distance, PolyLine, Polygon};
use map_gui::colors::{ColorScheme, ColorSchemeChoice};
use map_gui::render::DrawMap;
use street_network::Direction;
use widgetry::mapspace::{DummyID, World};
use widgetry::tools::PopupMsg;
use widgetry::{
    Color, ControlState, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome,
    Panel, State, TextExt, Toggle, Widget,
};

use crate::draw_cells::RenderCells;
use crate::edit::{EditMode, EditNeighbourhood, EditOutcome};
use crate::filters::auto::Heuristic;
use crate::{colors, App, Neighbourhood, NeighbourhoodID, Transition};

pub struct Viewer {
    top_panel: Panel,
    left_panel: Panel,
    neighbourhood: Neighbourhood,
    draw_top_layer: Drawable,
    draw_under_roads_layer: Drawable,
    highlight_cell: World<DummyID>,
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
            draw_top_layer: Drawable::empty(ctx),
            draw_under_roads_layer: Drawable::empty(ctx),
            highlight_cell: World::unbounded(),
            edit: EditNeighbourhood::temporary(),
            show_error: Drawable::empty(ctx),
        };
        viewer.update(ctx, app);
        Box::new(viewer)
    }

    fn update(&mut self, ctx: &mut EventCtx, app: &App) {
        let (edit, draw_top_layer, draw_under_roads_layer, render_cells, highlight_cell) =
            setup_editing(ctx, app, &self.neighbourhood);
        self.edit = edit;
        self.draw_top_layer = draw_top_layer;
        self.draw_under_roads_layer = draw_under_roads_layer;
        self.highlight_cell = highlight_cell;

        let mut show_error = GeomBatch::new();
        let mut disconnected_cells = 0;
        for (idx, cell) in self.neighbourhood.cells.iter().enumerate() {
            if cell.is_disconnected() {
                disconnected_cells += 1;
                show_error.extend(
                    Color::RED.alpha(0.8),
                    render_cells.polygons_per_cell[idx].clone(),
                );
            }
        }
        let warning = if disconnected_cells == 0 {
            Widget::nothing()
        } else {
            let msg = if disconnected_cells == 1 {
                "1 cell isn't reachable".to_string()
            } else {
                format!("{disconnected_cells} cells aren't reachable")
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
                &self.top_panel,
                Widget::col(vec![
                    format!(
                        "Neighbourhood area: {}",
                        app.session
                            .partitioning
                            .neighbourhood_area_km2(self.neighbourhood.id)
                    )
                    .text_widget(ctx),
                    Widget::row(vec![
                        "Draw traffic cells as".text_widget(ctx).centered_vert(),
                        Toggle::choice(
                            ctx,
                            "draw cells",
                            "areas",
                            "outlines",
                            Key::D,
                            app.session.draw_cells_as_areas,
                        ),
                    ]),
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
        if let Some(t) = app.session.layers.event(ctx) {
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
                }

                match self.edit.handle_panel_action(
                    ctx,
                    app,
                    x.as_ref(),
                    &self.neighbourhood,
                    &mut self.left_panel,
                ) {
                    // Fall through to AltProposals
                    EditOutcome::Nothing => {}
                    EditOutcome::UpdatePanelAndWorld => {
                        self.update(ctx, app);
                        return Transition::Keep;
                    }
                    EditOutcome::Transition(t) => {
                        return t;
                    }
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
            Outcome::Changed(x) => match x.as_ref() {
                "Advanced features" => {
                    app.opts.dev = self.left_panel.is_checked("Advanced features");
                    self.update(ctx, app);
                    return Transition::Keep;
                }
                "heuristic" => {
                    app.session.heuristic = self.left_panel.dropdown_value("heuristic");
                    return Transition::Keep;
                }
                "areas" | "outlines" => {
                    app.session.draw_cells_as_areas = self.left_panel.is_checked("draw cells");

                    app.cs = ColorScheme::new(
                        ctx,
                        if app.session.draw_cells_as_areas {
                            ColorSchemeChoice::ClassicLTN
                        } else {
                            ColorSchemeChoice::LTN
                        },
                    );
                    let (buildings, draw_all_buildings, draw_all_building_outlines) =
                        DrawMap::regenerate_buildings(
                            ctx,
                            &app.map,
                            &app.cs,
                            &app.opts,
                            &mut Timer::throwaway(),
                        );
                    app.draw_map.buildings = buildings;
                    app.draw_map.draw_all_buildings = draw_all_buildings;
                    app.draw_map.draw_all_building_outlines = draw_all_building_outlines;

                    let (parking_lots, draw_all_unzoomed_parking_lots) =
                        DrawMap::regenerate_parking_lots(ctx, &app.map, &app.cs, &app.opts);
                    app.draw_map.parking_lots = parking_lots;
                    app.draw_map.draw_all_unzoomed_parking_lots = draw_all_unzoomed_parking_lots;

                    let (edit, draw_top_layer, draw_under_roads_layer, _, highlight_cell) =
                        setup_editing(ctx, app, &self.neighbourhood);
                    self.edit = edit;
                    self.draw_top_layer = draw_top_layer;
                    self.draw_under_roads_layer = draw_under_roads_layer;
                    self.highlight_cell = highlight_cell;
                    return Transition::Keep;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        match self.edit.event(ctx, app, &self.neighbourhood) {
            EditOutcome::Nothing => {}
            EditOutcome::UpdatePanelAndWorld => {
                self.update(ctx, app);
            }
            EditOutcome::Transition(t) => {
                return t;
            }
        }

        self.highlight_cell.event(ctx);

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        crate::draw_with_layering(g, app, |g| g.redraw(&self.draw_under_roads_layer));
        g.redraw(&self.neighbourhood.fade_irrelevant);
        self.draw_top_layer.draw(g);
        self.highlight_cell.draw(g);
        self.edit.world.draw(g);

        self.top_panel.draw(g);
        self.left_panel.draw(g);
        app.session.layers.draw(g);
        self.neighbourhood.labels.draw(g);
        app.session.draw_all_filters.draw(g);
        app.session.draw_poi_icons.draw(g);

        if self.left_panel.currently_hovering() == Some(&"warning".to_string()) {
            g.redraw(&self.show_error);
        }

        if let EditMode::FreehandFilters(ref lasso) = app.session.edit_mode {
            lasso.draw(g);
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
) -> (
    EditNeighbourhood,
    Drawable,
    Drawable,
    RenderCells,
    World<DummyID>,
) {
    let edit = EditNeighbourhood::new(ctx, app, neighbourhood);
    let map = &app.map;

    // Draw some stuff under roads and other stuff on top
    let mut draw_top_layer = GeomBatch::new();
    let mut draw_under_roads_layer = GeomBatch::new();
    // Use a separate world to highlight cells when hovering on them. This is separate from
    // edit.world so that we draw it even while hovering on roads/intersections in a cell
    let mut highlight_cell = World::bounded(app.map.get_bounds());

    let render_cells = RenderCells::new(map, neighbourhood);
    if app.session.draw_cells_as_areas {
        draw_under_roads_layer = render_cells.draw_colored_areas();
        draw_top_layer.append(render_cells.draw_island_outlines(true));

        // Highlight border arrows when hovered
        for (idx, polygons) in render_cells.polygons_per_cell.iter().enumerate() {
            // Edge case happening near https://www.openstreetmap.org/way/106879596
            if polygons.is_empty() {
                continue;
            }

            let color = render_cells.colors[idx].alpha(1.0);
            let mut batch = GeomBatch::new();
            for arrow in neighbourhood.cells[idx].border_arrows(app) {
                batch.push(color, arrow);
            }

            highlight_cell
                .add_unnamed()
                .hitbox(Polygon::union_all(polygons.clone()))
                // Don't draw cells by default
                .drawn_in_master_batch()
                .draw_hovered(batch)
                .build(ctx);
        }
    } else {
        draw_top_layer.append(render_cells.draw_island_outlines(false));

        // Highlight cell areas and their border arrows when hovered
        for (idx, polygons) in render_cells.polygons_per_cell.iter().enumerate() {
            // Edge case happening near https://www.openstreetmap.org/way/106879596
            if polygons.is_empty() {
                continue;
            }

            let mut batch = GeomBatch::new();
            batch.extend(Color::YELLOW.alpha(0.1), polygons.clone());
            for arrow in neighbourhood.cells[idx].border_arrows(app) {
                batch.push(Color::YELLOW, arrow);
            }

            highlight_cell
                .add_unnamed()
                .hitbox(Polygon::union_all(polygons.clone()))
                // Don't draw cells by default
                .drawn_in_master_batch()
                .draw_hovered(batch)
                .build(ctx);
        }
    }

    if !matches!(app.session.edit_mode, EditMode::Shortcuts(_)) {
        draw_top_layer.append(neighbourhood.shortcuts.draw_heatmap(app));
    }

    // Draw the borders of each cell
    for (idx, cell) in neighbourhood.cells.iter().enumerate() {
        let color = if app.session.draw_cells_as_areas {
            render_cells.colors[idx].alpha(1.0)
        } else {
            Color::BLACK
        };
        for arrow in cell.border_arrows(app) {
            draw_top_layer.push(color, arrow.clone());
            if let Ok(outline) = arrow.to_outline(Distance::meters(1.0)) {
                draw_top_layer.push(Color::BLACK, outline);
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
                    draw_top_layer.push(colors::ROAD_LABEL, poly);
                }
            }
        }
    }

    (
        edit,
        draw_top_layer.build(ctx),
        ctx.upload(draw_under_roads_layer),
        render_cells,
        highlight_cell,
    )
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
