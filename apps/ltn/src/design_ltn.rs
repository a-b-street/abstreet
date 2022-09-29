use geom::{ArrowCap, Distance, PolyLine};
use street_network::Direction;
use widgetry::mapspace::{DummyID, World};
use widgetry::tools::{ChooseSomething, PopupMsg};
use widgetry::{
    lctrl, Color, ControlState, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line,
    Outcome, Panel, RewriteColor, State, TextExt, Toggle, Widget,
};

use crate::components::{AppwidePanel, BottomPanel, Mode};
use crate::draw_cells::RenderCells;
use crate::edit::{EditMode, EditNeighbourhood, EditOutcome};
use crate::filters::auto::Heuristic;
use crate::{colors, is_private, App, FilterType, Neighbourhood, NeighbourhoodID, Transition};

pub struct DesignLTN {
    appwide_panel: AppwidePanel,
    bottom_panel: Panel,
    neighbourhood: Neighbourhood,
    draw_top_layer: Drawable,
    draw_under_roads_layer: Drawable,
    highlight_cell: World<DummyID>,
    edit: EditNeighbourhood,
    // Expensive to calculate
    preserve_state: crate::save::PreserveState,

    show_error: Drawable,
}

impl DesignLTN {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        id: NeighbourhoodID,
    ) -> Box<dyn State<App>> {
        app.per_map.current_neighbourhood = Some(id);

        let neighbourhood = Neighbourhood::new(ctx, app, id);

        let mut state = Self {
            appwide_panel: AppwidePanel::new(ctx, app, Mode::ModifyNeighbourhood),
            bottom_panel: Panel::empty(ctx),
            neighbourhood,
            draw_top_layer: Drawable::empty(ctx),
            draw_under_roads_layer: Drawable::empty(ctx),
            highlight_cell: World::unbounded(),
            edit: EditNeighbourhood::temporary(),
            preserve_state: crate::save::PreserveState::DesignLTN(
                app.per_map.partitioning.all_blocks_in_neighbourhood(id),
            ),

            show_error: Drawable::empty(ctx),
        };
        state.update(ctx, app);
        Box::new(state)
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

        self.bottom_panel = make_bottom_panel(
            ctx,
            app,
            &self.appwide_panel,
            Widget::col(vec![
                format!(
                    "Area: {}",
                    app.per_map
                        .partitioning
                        .neighbourhood_area_km2(self.neighbourhood.id)
                )
                .text_widget(ctx),
                warning,
                advanced_panel(ctx, app),
            ]),
        );
    }
}

impl State<App> for DesignLTN {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) = self
            .appwide_panel
            .event(ctx, app, &self.preserve_state, help)
        {
            return t;
        }
        if let Some(t) = app.session.layers.event(
            ctx,
            &app.cs,
            Mode::ModifyNeighbourhood,
            Some(&self.bottom_panel),
        ) {
            return t;
        }
        match self.bottom_panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "Automatically place filters" {
                    return launch_autoplace_filters(ctx, self.neighbourhood.id);
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
                    &mut self.bottom_panel,
                ) {
                    EditOutcome::Nothing => unreachable!(),
                    EditOutcome::UpdatePanelAndWorld => {
                        self.update(ctx, app);
                        return Transition::Keep;
                    }
                    EditOutcome::Transition(t) => {
                        return t;
                    }
                }
            }
            Outcome::Changed(x) => match x.as_ref() {
                "Advanced features" => {
                    app.opts.dev = self.bottom_panel.is_checked("Advanced features");
                    self.update(ctx, app);
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
        app.draw_with_layering(g, |g| g.redraw(&self.draw_under_roads_layer));
        g.redraw(&self.neighbourhood.fade_irrelevant);
        self.draw_top_layer.draw(g);
        self.highlight_cell.draw(g);
        self.edit.world.draw(g);

        self.appwide_panel.draw(g);
        self.bottom_panel.draw(g);
        app.session.layers.draw(g, app);
        self.neighbourhood.labels.draw(g);
        app.per_map.draw_all_filters.draw(g);
        app.per_map.draw_poi_icons.draw(g);

        if self.bottom_panel.currently_hovering() == Some(&"warning".to_string()) {
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
    let map = &app.per_map.map;

    // Draw some stuff under roads and other stuff on top
    let mut draw_top_layer = GeomBatch::new();
    // Use a separate world to highlight cells when hovering on them. This is separate from
    // edit.world so that we draw it even while hovering on roads/intersections in a cell
    let mut highlight_cell = World::bounded(app.per_map.map.get_bounds());

    let render_cells = RenderCells::new(map, neighbourhood);

    let draw_under_roads_layer = render_cells.draw_colored_areas();
    draw_top_layer.append(render_cells.draw_island_outlines());

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
            .hitboxes(polygons.clone())
            // Don't draw cells by default
            .drawn_in_master_batch()
            .draw_hovered(batch)
            .build(ctx);
    }

    if !matches!(app.session.edit_mode, EditMode::Shortcuts(_)) {
        draw_top_layer.append(neighbourhood.shortcuts.draw_heatmap(app));
    }

    // Draw the borders of each cell
    for (idx, cell) in neighbourhood.cells.iter().enumerate() {
        let color = render_cells.colors[idx].alpha(1.0);
        for arrow in cell.border_arrows(app) {
            draw_top_layer.push(color, arrow.clone());
            draw_top_layer.push(Color::BLACK, arrow.to_outline(Distance::meters(1.0)));
        }
    }

    // Draw one-way arrows and mark private roads
    let private_road = GeomBatch::load_svg(ctx, "system/assets/map/private_road.svg");

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

                draw_top_layer.push(
                    colors::ROAD_LABEL,
                    pl.make_arrow(thickness * 2.0, ArrowCap::Triangle)
                        .to_outline(thickness / 2.0),
                );
            }
        }

        // Mimic the UK-style "no entry" / dead-end symbol at both ends of every private road
        // segment
        if is_private(road) {
            // The outline is 1m on each side
            let width = road.get_width() - Distance::meters(2.0);
            for (dist, rotate) in [(width, 90.0), (road.center_pts.length() - width, -90.0)] {
                if let Ok((pt, angle)) = road.center_pts.dist_along(dist) {
                    draw_top_layer.append(
                        private_road
                            .clone()
                            .scale_to_fit_width(width.inner_meters())
                            .centered_on(pt)
                            .rotate_around_batch_center(angle.rotate_degs(rotate)),
                    );
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

fn launch_autoplace_filters(ctx: &mut EventCtx, id: NeighbourhoodID) -> Transition {
    Transition::Push(ChooseSomething::new_state(
        ctx,
        "Add one filter automatically, using different heuristics",
        Heuristic::choices(),
        Box::new(move |heuristic, ctx, app| {
            match ctx.loading_screen("automatically filter a neighbourhood", |ctx, timer| {
                let neighbourhood = Neighbourhood::new(ctx, app, id);
                heuristic.apply(ctx, app, &neighbourhood, timer)
            }) {
                Ok(()) => Transition::Multi(vec![Transition::Pop, Transition::Recreate]),
                Err(err) => {
                    Transition::Replace(PopupMsg::new_state(ctx, "Error", vec![err.to_string()]))
                }
            }
        }),
    ))
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
    if app.per_map.consultation.is_some() {
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
    ])
    .section(ctx)
}

fn make_bottom_panel(
    ctx: &mut EventCtx,
    app: &App,
    appwide_panel: &AppwidePanel,
    per_tab_contents: Widget,
) -> Panel {
    let row = Widget::row(vec![
        if app.per_map.consultation.is_none() {
            ctx.style()
                .btn_outline
                .text("Adjust boundary")
                .hotkey(Key::B)
                .build_def(ctx)
                .centered_vert()
            // TODO Vertical sep
        } else {
            Widget::nothing()
        },
        edit_mode(ctx, app),
        match app.session.edit_mode {
            EditMode::Filters => crate::edit::filters::widget(ctx),
            EditMode::FreehandFilters(_) => crate::edit::freehand_filters::widget(ctx),
            EditMode::Oneways => crate::edit::one_ways::widget(ctx),
            EditMode::Shortcuts(ref focus) => {
                crate::edit::shortcuts::widget(ctx, app, focus.as_ref())
            }
        }
        .named("edit mode contents"),
        ctx.style()
            .btn_plain
            .icon("system/assets/tools/undo.svg")
            .disabled(app.per_map.edits.previous_version.is_none())
            .hotkey(lctrl(Key::Z))
            .build_widget(ctx, "undo"),
        Widget::col(vec![
            // TODO Only count new filters, not existing
            format!(
                "{} filters added",
                app.per_map.edits.roads.len() + app.per_map.edits.intersections.len()
            )
            .text_widget(ctx),
            format!(
                "{} road directions changed",
                app.per_map.edits.one_ways.len()
            )
            .text_widget(ctx),
        ]),
        per_tab_contents,
    ]);

    BottomPanel::new(ctx, appwide_panel, row)
}

fn edit_mode(ctx: &mut EventCtx, app: &App) -> Widget {
    let edit_mode = &app.session.edit_mode;
    let filter = |ft: FilterType, hide_color: Color, name: &str| {
        let mut btn = ctx
            .style()
            .btn_solid_primary
            .icon(ft.svg_path())
            .image_color(
                RewriteColor::Change(hide_color, Color::CLEAR),
                ControlState::Default,
            )
            .image_color(
                RewriteColor::Change(hide_color, Color::CLEAR),
                ControlState::Disabled,
            )
            .disabled(matches!(edit_mode, EditMode::Filters) && app.session.filter_type == ft);
        if app.session.filter_type == ft {
            btn = btn.hotkey(Key::F1);
        }
        btn.build_widget(ctx, name)
    };

    Widget::row(vec![
        Widget::row(vec![
            filter(
                FilterType::WalkCycleOnly,
                Color::hex("#0b793a"),
                "Modal filter -- walking/cycling only",
            ),
            filter(FilterType::NoEntry, Color::RED, "Modal filter - no entry"),
            filter(FilterType::BusGate, *colors::BUS_ROUTE, "Bus gate"),
        ])
        .section(ctx),
        ctx.style()
            .btn_solid_primary
            .icon("system/assets/tools/select.svg")
            .disabled(matches!(edit_mode, EditMode::FreehandFilters(_)))
            .hotkey(Key::F2)
            .build_widget(ctx, "Freehand filters")
            .centered_vert(),
        ctx.style()
            .btn_solid_primary
            .icon("system/assets/tools/one_ways.svg")
            .disabled(matches!(edit_mode, EditMode::Oneways))
            .hotkey(Key::F3)
            .build_widget(ctx, "One-ways")
            .centered_vert(),
        ctx.style()
            .btn_solid_primary
            .icon("system/assets/tools/shortcut.svg")
            .disabled(matches!(edit_mode, EditMode::Shortcuts(_)))
            .hotkey(Key::F4)
            .build_widget(ctx, "Shortcuts")
            .centered_vert(),
    ])
}
