use geom::{Angle, ArrowCap, Distance, PolyLine, Pt2D};
use map_gui::tools::DrawSimpleRoadLabels;
use osm2streets::Direction;
use widgetry::mapspace::{DummyID, World};
use widgetry::tools::{ChooseSomething, PopupMsg};
use widgetry::{
    lctrl, Choice, Color, ControlState, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx, Key,
    Line, Outcome, Panel, RewriteColor, State, Text, TextExt, Widget,
};

use super::{EditMode, EditNeighbourhood, EditOutcome};
use crate::components::{AppwidePanel, BottomPanel, Mode};
use crate::logic::AutoFilterHeuristic;
use crate::render::colors;
use crate::{
    is_private, pages, render, App, FilterType, Neighbourhood, NeighbourhoodID, Transition,
};

pub struct DesignLTN {
    appwide_panel: AppwidePanel,
    bottom_panel: Panel,
    neighbourhood: Neighbourhood,
    draw_top_layer: Drawable,
    draw_under_roads_layer: Drawable,
    fade_irrelevant: Drawable,
    labels: DrawSimpleRoadLabels,
    highlight_cell: World<DummyID>,
    edit: EditNeighbourhood,
    // Expensive to calculate
    preserve_state: crate::save::PreserveState,

    show_unreachable_cell: Drawable,
    show_suspicious_perimeters: Drawable,
}

impl DesignLTN {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        id: NeighbourhoodID,
    ) -> Box<dyn State<App>> {
        app.per_map.current_neighbourhood = Some(id);

        let neighbourhood = Neighbourhood::new(app, id);
        let fade_irrelevant = neighbourhood.fade_irrelevant(ctx, app);

        let mut label_roads = neighbourhood.perimeter_roads.clone();
        label_roads.extend(neighbourhood.interior_roads.clone());
        let labels = DrawSimpleRoadLabels::new(
            ctx,
            app,
            colors::LOCAL_ROAD_LABEL,
            Box::new(move |r| label_roads.contains(&r.id)),
        );

        let mut show_suspicious_perimeters = GeomBatch::new();
        for r in &neighbourhood.suspicious_perimeter_roads {
            show_suspicious_perimeters
                .push(Color::RED, app.per_map.map.get_r(*r).get_thick_polygon());
        }

        let mut state = Self {
            appwide_panel: AppwidePanel::new(ctx, app, Mode::ModifyNeighbourhood),
            bottom_panel: Panel::empty(ctx),
            neighbourhood,
            draw_top_layer: Drawable::empty(ctx),
            draw_under_roads_layer: Drawable::empty(ctx),
            fade_irrelevant,
            labels,
            highlight_cell: World::new(),
            edit: EditNeighbourhood::temporary(),
            preserve_state: crate::save::PreserveState::DesignLTN(
                app.partitioning().neighbourhood_to_blocks(id),
            ),

            show_unreachable_cell: Drawable::empty(ctx),
            show_suspicious_perimeters: ctx.upload(show_suspicious_perimeters),
        };
        state.update(ctx, app);
        Box::new(state)
    }

    fn update(&mut self, ctx: &mut EventCtx, app: &App) {
        let (edit, draw_top_layer, draw_under_roads_layer, render_cells, highlight_cell) =
            setup_editing(ctx, app, &self.neighbourhood, &self.labels);
        self.edit = edit;
        self.draw_top_layer = draw_top_layer;
        self.draw_under_roads_layer = draw_under_roads_layer;
        self.highlight_cell = highlight_cell;

        let mut show_unreachable_cell = GeomBatch::new();
        let mut disconnected_cells = 0;
        for (idx, cell) in self.neighbourhood.cells.iter().enumerate() {
            if cell.is_disconnected() {
                disconnected_cells += 1;
                show_unreachable_cell.extend(
                    Color::RED.alpha(0.8),
                    render_cells.polygons_per_cell[idx].clone(),
                );
            }
        }
        let warning1 = if disconnected_cells == 0 {
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
                .build_widget(ctx, "warning1")
        };
        self.show_unreachable_cell = ctx.upload(show_unreachable_cell);

        let warning2 = if self.neighbourhood.suspicious_perimeter_roads.is_empty() {
            Widget::nothing()
        } else {
            ctx.style()
                .btn_plain
                .icon_text(
                    "system/assets/tools/warning.svg",
                    "Part of the perimeter is a local street",
                )
                .label_color(Color::RED, ControlState::Default)
                .no_tooltip()
                .build_widget(ctx, "warning2")
        };

        self.bottom_panel = make_bottom_panel(
            ctx,
            app,
            &self.appwide_panel,
            Widget::col(vec![
                format!(
                    "Area: {}",
                    app.partitioning()
                        .neighbourhood_area_km2(self.neighbourhood.id)
                )
                .text_widget(ctx)
                .centered_horiz(),
                warning1.centered_horiz(),
                warning2.centered_horiz(),
            ])
            .centered_vert(),
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
        if let Outcome::Clicked(x) = self.bottom_panel.event(ctx) {
            if x == "Advanced" {
                return launch_advanced(ctx, app, self.neighbourhood.id);
            } else if x == "warning1" {
                return Transition::Push(PopupMsg::new_state(
                    ctx,
                    "Unreachable cell",
                    vec![
                        "Some streets inside this area can't be reached by car at all.",
                        "You probably drew too many filters.",
                        "",
                        "(This may be incorrectly detected near some private/gated roads)",
                    ],
                ));
            } else if x == "warning2" {
                return Transition::Push(PopupMsg::new_state(
                        ctx,
                        "Unusual perimeter",
                        vec![
                        "Part of this area's perimeter consists of streets classified as local.",
                        "This is usually fine, when this area doesn't connect to other main roads farther away.",
                        "If you're near the edge of the map, it might be an error. Try importing a larger area, including the next major road in that direction",
                        ],
                        ));
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
        g.redraw(&self.fade_irrelevant);
        self.draw_top_layer.draw(g);
        self.highlight_cell.draw(g);
        self.edit.world.draw(g);

        self.appwide_panel.draw(g);
        self.bottom_panel.draw(g);
        app.session.layers.draw(g, app);
        self.labels.draw(g);
        app.per_map.draw_major_road_labels.draw(g);
        app.per_map.draw_all_filters.draw(g);
        app.per_map.draw_poi_icons.draw(g);

        if self.bottom_panel.currently_hovering() == Some(&"warning1".to_string()) {
            g.redraw(&self.show_unreachable_cell);
        }
        if self.bottom_panel.currently_hovering() == Some(&"warning2".to_string()) {
            g.redraw(&self.show_suspicious_perimeters);
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
    labels: &DrawSimpleRoadLabels,
) -> (
    EditNeighbourhood,
    Drawable,
    Drawable,
    render::RenderCells,
    World<DummyID>,
) {
    let edit = EditNeighbourhood::new(ctx, app, neighbourhood);
    let map = &app.per_map.map;

    // Draw some stuff under roads and other stuff on top
    let mut draw_top_layer = GeomBatch::new();
    // Use a separate world to highlight cells when hovering on them. This is separate from
    // edit.world so that we draw it even while hovering on roads/intersections in a cell
    let mut highlight_cell = World::new();

    let render_cells = render::RenderCells::new(map, neighbourhood);

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

    if !matches!(
        app.session.edit_mode,
        EditMode::Shortcuts(_) | EditMode::SpeedLimits
    ) {
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
        .interior_roads
        .iter()
        .chain(neighbourhood.perimeter_roads.iter())
    {
        let road = map.get_r(*r);
        if let Some(dir) = road.oneway_for_driving() {
            // Manually tuned to make arrows fit within roads of any width. We could be more
            // specific and calculate based on the road's outline, a buffer, etc, but we'd still
            // have to do some math to account for the triangular arrow cap. This value looks
            // reasonable.
            let thickness = 0.2 * road.get_width();
            let arrow_len = 5.0 * thickness;

            let slices = if let Some((start, end)) = labels.label_covers_road.get(r) {
                vec![
                    road.center_pts.exact_slice(Distance::ZERO, *start),
                    road.center_pts.exact_slice(*end, road.length()),
                ]
            } else {
                vec![road.center_pts.clone()]
            };

            let mut draw_arrow = |pt: Pt2D, angle: Angle| {
                // If the user has made the one-way point opposite to how the road is originally
                // oriented, reverse the arrows
                let pl = PolyLine::must_new(vec![
                    pt.project_away(arrow_len / 2.0, angle.opposite()),
                    pt.project_away(arrow_len / 2.0, angle),
                ])
                .maybe_reverse(dir == Direction::Back);

                draw_top_layer.push(
                    colors::LOCAL_ROAD_LABEL,
                    pl.make_arrow(thickness, ArrowCap::Triangle)
                        .to_outline(thickness / 4.0),
                );
            };

            let mut any = false;
            for slice in slices {
                for (pt, angle) in slice.step_along(3.0 * arrow_len, arrow_len) {
                    any = true;
                    draw_arrow(pt, angle);
                }
            }

            if !any {
                // If the label won and we haven't drawn anything, draw arrows right at the start
                // and end. Otherwise the user has no idea what's happening
                for dist in [Distance::ZERO, road.length()] {
                    let (pt, angle) = road.center_pts.must_dist_along(dist);
                    draw_arrow(pt, angle);
                }
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

fn launch_advanced(ctx: &mut EventCtx, app: &App, id: NeighbourhoodID) -> Transition {
    let mut choices = vec![Choice::string("Automatically place modal filters")];
    if !app.partitioning().custom_boundaries.contains_key(&id) {
        choices.push(Choice::string("Customize boundary (for drawing only)"));
        choices.push(Choice::string("Convert to freehand area"));
    }

    Transition::Push(ChooseSomething::new_state(
        ctx,
        "Advanced features",
        choices,
        Box::new(move |choice, ctx, app| {
            if choice == "Customize boundary (for drawing only)" {
                Transition::Replace(pages::CustomizeBoundary::new_state(ctx, app, id))
            } else if choice == "Convert to freehand area" {
                Transition::Replace(pages::FreehandBoundary::new_from_polygon(
                    ctx,
                    app,
                    format!("Converted from {:?}", id),
                    app.partitioning().get_info(id).block.polygon.clone(),
                ))
            } else {
                Transition::Replace(ChooseSomething::new_state(
                    ctx,
                    "Add one filter automatically, using different heuristics",
                    AutoFilterHeuristic::choices(),
                    Box::new(move |heuristic, ctx, app| {
                        match ctx.loading_screen(
                            "automatically filter a neighbourhood",
                            |ctx, timer| {
                                let neighbourhood = Neighbourhood::new(app, id);
                                heuristic.apply(ctx, app, &neighbourhood, timer)
                            },
                        ) {
                            Ok(()) => {
                                Transition::Multi(vec![Transition::Pop, Transition::Recreate])
                            }
                            Err(err) => Transition::Replace(PopupMsg::new_state(
                                ctx,
                                "Error",
                                vec![err.to_string()],
                            )),
                        }
                    }),
                ))
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

fn make_bottom_panel(
    ctx: &mut EventCtx,
    app: &App,
    appwide_panel: &AppwidePanel,
    per_tab_contents: Widget,
) -> Panel {
    let row = Widget::row(vec![
        edit_mode(ctx, app),
        if let EditMode::Shortcuts(ref focus) = app.session.edit_mode {
            super::shortcuts::widget(ctx, app, focus.as_ref())
        } else if let EditMode::SpeedLimits = app.session.edit_mode {
            super::speed_limits::widget(ctx)
        } else {
            Widget::nothing()
        }
        .named("edit mode contents"),
        Widget::vertical_separator(ctx),
        Widget::row(vec![
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/undo.svg")
                .disabled(app.edits().previous_version.is_none())
                .hotkey(lctrl(Key::Z))
                .build_widget(ctx, "undo"),
            Widget::col(vec![
                // TODO Only count new filters, not existing
                format!(
                    "{} filters",
                    app.edits().roads.len() + app.edits().intersections.len()
                )
                .text_widget(ctx),
                format!("{} road directions changed", app.edits().one_ways.len()).text_widget(ctx),
            ]),
        ]),
        Widget::vertical_separator(ctx),
        per_tab_contents,
        if app.per_map.consultation.is_none() {
            Widget::row(vec![
                Widget::vertical_separator(ctx),
                ctx.style()
                    .btn_outline
                    .text("Adjust boundary")
                    .hotkey(Key::B)
                    .build_def(ctx),
                ctx.style()
                    .btn_outline
                    .text("Per-resident route impact")
                    .build_def(ctx),
                ctx.style().btn_outline.text("Advanced").build_def(ctx),
            ])
            .centered_vert()
        } else {
            Widget::row(vec![
                Widget::vertical_separator(ctx),
                ctx.style()
                    .btn_outline
                    .text("Per-resident route impact")
                    .build_def(ctx),
            ])
            .centered_vert()
        },
    ])
    .evenly_spaced();

    BottomPanel::new(ctx, appwide_panel, row)
}

fn edit_mode(ctx: &mut EventCtx, app: &App) -> Widget {
    let edit_mode = &app.session.edit_mode;
    let hide_color = app.session.filter_type.hide_color();
    let name = match app.session.filter_type {
        FilterType::WalkCycleOnly => "Modal filter -- walking/cycling only",
        FilterType::NoEntry => "Modal filter - no entry",
        FilterType::BusGate => "Bus gate",
        FilterType::SchoolStreet => "School street",
    };

    Widget::row(vec![
        Widget::custom_row(vec![
            ctx.style()
                .btn_solid_primary
                .icon(app.session.filter_type.svg_path())
                .image_color(
                    RewriteColor::Change(hide_color, Color::CLEAR),
                    ControlState::Default,
                )
                .image_color(
                    RewriteColor::Change(hide_color, Color::CLEAR),
                    ControlState::Disabled,
                )
                .disabled(matches!(edit_mode, EditMode::Filters))
                .tooltip_and_disabled({
                    let mut txt = Text::new();
                    txt.add_line(Line(Key::F1.describe()).fg(ctx.style().text_hotkey_color));
                    txt.append(Line(" - "));
                    txt.append(Line(name));
                    txt.add_line(Line("Click").fg(ctx.style().text_hotkey_color));
                    txt.append(Line(
                        " a road or intersection to add or remove a modal filter",
                    ));
                    txt
                })
                .hotkey(Key::F1)
                .build_widget(ctx, name)
                .centered_vert(),
            ctx.style()
                .btn_plain
                .dropdown()
                .build_widget(ctx, "Change modal filter")
                .centered_vert(),
        ]),
        ctx.style()
            .btn_solid_primary
            .icon("system/assets/tools/select.svg")
            .disabled(matches!(edit_mode, EditMode::FreehandFilters(_)))
            .hotkey(Key::F2)
            .tooltip_and_disabled({
                let mut txt = Text::new();
                txt.add_line(Line(Key::F2.describe()).fg(ctx.style().text_hotkey_color));
                txt.append(Line(" - Freehand filters"));
                txt.add_line(Line("Click and drag").fg(ctx.style().text_hotkey_color));
                txt.append(Line(" across the roads you want to filter"));
                txt
            })
            .build_widget(ctx, "Freehand filters")
            .centered_vert(),
        ctx.style()
            .btn_solid_primary
            .icon("system/assets/tools/one_ways.svg")
            .disabled(matches!(edit_mode, EditMode::Oneways))
            .hotkey(Key::F3)
            .tooltip_and_disabled({
                let mut txt = Text::new();
                txt.add_line(Line(Key::F3.describe()).fg(ctx.style().text_hotkey_color));
                txt.append(Line(" - One-ways"));
                txt.add_line(Line("Click").fg(ctx.style().text_hotkey_color));
                txt.append(Line(" a road to change its direction"));
                txt
            })
            .build_widget(ctx, "One-ways")
            .centered_vert(),
        ctx.style()
            .btn_solid_primary
            .icon("system/assets/tools/shortcut.svg")
            .disabled(matches!(edit_mode, EditMode::Shortcuts(_)))
            .hotkey(Key::F4)
            .tooltip_and_disabled({
                let mut txt = Text::new();
                txt.add_line(Line(Key::F4.describe()).fg(ctx.style().text_hotkey_color));
                txt.append(Line(" - Shortcuts"));
                txt.add_line(Line("Click").fg(ctx.style().text_hotkey_color));
                txt.append(Line(" a road to view shortcuts through it"));
                txt
            })
            .build_widget(ctx, "Shortcuts")
            .centered_vert(),
        ctx.style()
            .btn_solid_primary
            .icon("system/assets/tools/20_mph.svg")
            .image_color(
                RewriteColor::Change(Color::RED, Color::CLEAR),
                ControlState::Default,
            )
            .image_color(
                RewriteColor::Change(Color::RED, Color::CLEAR),
                ControlState::Disabled,
            )
            .disabled(matches!(edit_mode, EditMode::SpeedLimits))
            .hotkey(Key::F5)
            .tooltip_and_disabled({
                let mut txt = Text::new();
                txt.add_line(Line(Key::F4.describe()).fg(ctx.style().text_hotkey_color));
                txt.append(Line(" - Speed limits"));
                txt.add_line(Line("Click").fg(ctx.style().text_hotkey_color));
                txt.append(Line(" a road to convert it to 20mph (32kph)"));
                txt
            })
            .build_widget(ctx, "Speed limits")
            .centered_vert(),
    ])
}
