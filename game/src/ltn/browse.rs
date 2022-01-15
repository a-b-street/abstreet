use std::collections::HashSet;

use geom::Distance;
use map_gui::tools::{CityPicker, DrawRoadLabels, Navigator, PopupMsg, URLManager};
use widgetry::mapspace::{ToggleZoomed, World, WorldOutcome};
use widgetry::{
    lctrl, Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State,
    TextExt, Toggle, VerticalAlignment, Widget,
};

use super::{Neighborhood, NeighborhoodID, Partitioning};
use crate::app::{App, Transition};
use crate::debug::DebugMode;

pub struct BrowseNeighborhoods {
    panel: Panel,
    world: World<NeighborhoodID>,
    labels: DrawRoadLabels,
    draw_all_filters: ToggleZoomed,
    draw_boundary_roads: ToggleZoomed,
}

impl BrowseNeighborhoods {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        URLManager::update_url_map_name(app);

        let style = Style::SimpleColoring;
        let world = ctx.loading_screen("calculate neighborhoods", |ctx, timer| {
            // TODO Or if the map doesn't match? Do we take care of this in SessionState for
            // anything?!
            if app.session.partitioning.neighborhoods.is_empty() {
                app.session.partitioning = Partitioning::seed_using_heuristics(app, timer);
            }
            make_world(ctx, app, style)
        });
        let draw_all_filters = app.session.modal_filters.draw(ctx, &app.primary.map, None);

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
            Toggle::checkbox(ctx, "highlight boundary roads", Key::H, true),
            Widget::row(vec![
                "Draw neighborhoods:".text_widget(ctx).centered_vert(),
                Widget::dropdown(
                    ctx,
                    "style",
                    style,
                    vec![
                        Choice::new("simple", Style::SimpleColoring),
                        Choice::new("cells", Style::Cells),
                    ],
                ),
            ]),
            ctx.style()
                .btn_outline
                .text("Export to GeoJSON")
                .build_def(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        Box::new(BrowseNeighborhoods {
            panel,
            world,
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
                "Home" => {
                    return Transition::Clear(vec![crate::pregame::TitleScreen::new_state(
                        ctx, app,
                    )]);
                }
                "change map" => {
                    return Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            // TODO If we leave the LTN tool and change maps elsewhere, this won't
                            // work! Do we have per-map session state?
                            app.session.partitioning = Partitioning::empty();
                            Transition::Replace(BrowseNeighborhoods::new_state(ctx, app))
                        }),
                    ));
                }
                "search" => {
                    return Transition::Push(Navigator::new_state(ctx, app));
                }
                "Export to GeoJSON" => {
                    let result = ctx.loading_screen("export LTNs", |ctx, timer| {
                        super::export::write_geojson_file(ctx, app, timer)
                    });
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
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                self.world = ctx.loading_screen("change style", |ctx, _| {
                    make_world(ctx, app, self.panel.dropdown_value("style"))
                });
            }
            _ => {}
        }

        if let WorldOutcome::ClickedObject(id) = self.world.event(ctx) {
            return Transition::Push(super::connectivity::Viewer::new_state(ctx, app, id));
        }

        if ctx.input.pressed(lctrl(Key::D)) {
            return Transition::Push(DebugMode::new_state(ctx, app));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        self.world.draw(g);
        self.draw_all_filters.draw(g);
        if self.panel.is_checked("highlight boundary roads") {
            self.draw_boundary_roads.draw(g);
        }
        if g.canvas.is_unzoomed() {
            self.labels.draw(g, app);
        }
    }
}

fn make_world(ctx: &mut EventCtx, app: &App, style: Style) -> World<NeighborhoodID> {
    let mut world = World::bounded(app.primary.map.get_bounds());
    for (id, (block, color)) in &app.session.partitioning.neighborhoods {
        match style {
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
                let map = &app.primary.map;
                let neighborhood = Neighborhood::new(ctx, app, *id);
                let render_cells = super::draw_cells::RenderCells::new(map, &neighborhood);
                let hovered_batch = render_cells.draw_grid();
                world
                    .add(*id)
                    .hitbox(block.polygon.clone())
                    .draw_color(color.alpha(0.5))
                    .draw_hovered(hovered_batch)
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
    for (block, _) in app.session.partitioning.neighborhoods.values() {
        for id in &block.perimeter.roads {
            let r = id.road;
            if seen_roads.contains(&r) {
                continue;
            }
            seen_roads.insert(r);
            let road = app.primary.map.get_r(r);
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
                batch.unzoomed.push(
                    Color::RED.alpha(0.8),
                    app.primary.map.get_i(i).polygon.clone(),
                );
                batch.zoomed.push(
                    Color::RED.alpha(0.5),
                    app.primary.map.get_i(i).polygon.clone(),
                );
            }
        }
    }
    batch.build(ctx)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Style {
    SimpleColoring,
    Cells,
}
