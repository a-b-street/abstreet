use std::collections::BTreeSet;

use geom::{Distance, Polygon};
use map_gui::tools::EditPolygon;
use widgetry::tools::Lasso;
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, State, Text, TextExt,
    Widget,
};

use crate::components::{AppwidePanel, Mode};
use crate::partition::CustomBoundary;
use crate::{mut_partitioning, App, NeighbourhoodID, Transition};

pub struct FreehandBoundary {
    appwide_panel: AppwidePanel,
    left_panel: Panel,

    name: String,
    id: Option<NeighbourhoodID>,
    custom: Option<CustomBoundary>,
    draw_custom: Drawable,
    edit: EditPolygon,
    lasso: Option<Lasso>,
}

impl FreehandBoundary {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        name: String,
        id: Option<NeighbourhoodID>,
        custom: Option<CustomBoundary>,
    ) -> Box<dyn State<App>> {
        let appwide_panel = AppwidePanel::new(ctx, app, Mode::FreehandBoundary);
        let left_panel = make_panel(ctx, &appwide_panel.top_panel);
        let mut state = Self {
            appwide_panel,
            left_panel,
            id,
            custom,
            draw_custom: Drawable::empty(ctx),
            edit: EditPolygon::new(ctx, app, Vec::new(), false),
            lasso: None,
            name,
        };
        if let Some(ref custom) = state.custom {
            state.edit = EditPolygon::new(
                ctx,
                app,
                custom
                    .boundary_polygon
                    .clone()
                    .into_outer_ring()
                    .into_points(),
                false,
            );
            state.draw_custom = render(ctx, app, custom);
        }
        Box::new(state)
    }
}

impl State<App> for FreehandBoundary {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(ref mut lasso) = self.lasso {
            if let Some(polygon) = lasso.event(ctx) {
                let polygon = polygon.simplify(50.0);

                self.lasso = None;
                self.edit = EditPolygon::new(
                    ctx,
                    app,
                    polygon.clone().into_outer_ring().into_points(),
                    false,
                );

                self.custom = Some(polygon_to_custom_boundary(app, polygon, self.name.clone()));
                self.draw_custom = render(ctx, app, self.custom.as_ref().unwrap());
                self.left_panel = make_panel(ctx, &self.appwide_panel.top_panel);
            }
            return Transition::Keep;
        }

        if self.edit.event(ctx, app) {
            if let Ok(ring) = self.edit.get_ring() {
                self.custom = Some(polygon_to_custom_boundary(
                    app,
                    ring.into_polygon(),
                    self.name.clone(),
                ));
                self.draw_custom = render(ctx, app, self.custom.as_ref().unwrap());
            }
        }

        // PreserveState doesn't matter, can't switch proposals in FreehandBoundary anyway
        if let Some(t) =
            self.appwide_panel
                .event(ctx, app, &crate::save::PreserveState::Route, help)
        {
            return t;
        }
        if let Some(t) = app
            .session
            .layers
            .event(ctx, &app.cs, Mode::FreehandBoundary, None)
        {
            return t;
        }
        if let Outcome::Clicked(x) = self.left_panel.event(ctx) {
            match x.as_ref() {
                "Cancel" => {
                    return Transition::Replace(crate::PickArea::new_state(ctx, app));
                }
                "Confirm" => {
                    if let Some(custom) = self.custom.take() {
                        let id = if let Some(id) = self.id {
                            // Overwrite the existing one
                            mut_partitioning!(app).custom_boundaries.insert(id, custom);
                            id
                        } else {
                            mut_partitioning!(app).add_custom_boundary(custom)
                        };
                        // TODO Clicking is weird, acts like we click load proposal
                        return Transition::Replace(crate::design_ltn::DesignLTN::new_state(
                            ctx, app, id,
                        ));
                    }
                }
                "Select freehand" => {
                    self.lasso = Some(Lasso::new(Distance::meters(1.0)));
                    self.left_panel = make_panel_for_lasso(ctx, &self.appwide_panel.top_panel);
                }
                _ => unreachable!(),
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.appwide_panel.draw(g);
        self.left_panel.draw(g);
        app.session.layers.draw(g, app);
        if let Some(ref lasso) = self.lasso {
            lasso.draw(g);
        }
        self.edit.draw(g);
        g.redraw(&self.draw_custom);
    }
}

fn make_panel(ctx: &mut EventCtx, top_panel: &Panel) -> Panel {
    crate::components::LeftPanel::builder(
        ctx,
        top_panel,
        Widget::col(vec![
            Line("Draw custom neighbourhood boundary")
                .small_heading()
                .into_widget(ctx),
            ctx.style()
                .btn_outline
                .icon_text("system/assets/tools/select.svg", "Select freehand")
                .hotkey(Key::F)
                .build_def(ctx),
            Widget::row(vec![
                ctx.style()
                    .btn_solid_primary
                    .text("Confirm")
                    .hotkey(Key::Enter)
                    .build_def(ctx),
                ctx.style()
                    .btn_solid_destructive
                    .text("Cancel")
                    .hotkey(Key::Escape)
                    .build_def(ctx),
            ]),
        ]),
    )
    .build(ctx)
}

fn make_panel_for_lasso(ctx: &mut EventCtx, top_panel: &Panel) -> Panel {
    crate::components::LeftPanel::builder(
        ctx,
        top_panel,
        Widget::col(vec![
            "Draw a custom boundary for a neighbourhood"
                .text_widget(ctx)
                .centered_vert(),
            Text::from_all(vec![
                Line("Click and drag").fg(ctx.style().text_hotkey_color),
                Line(" to select the blocks to add to this neighbourhood"),
            ])
            .into_widget(ctx),
        ]),
    )
    .build(ctx)
}

fn help() -> Vec<&'static str> {
    vec!["TODO"]
}

fn polygon_to_custom_boundary(
    app: &App,
    boundary_polygon: Polygon,
    name: String,
) -> CustomBoundary {
    let map = &app.per_map.map;

    // Find all intersections inside the polygon
    let mut intersections_inside = BTreeSet::new();
    for i in map.all_intersections() {
        if boundary_polygon.contains_pt(i.polygon.center()) {
            intersections_inside.insert(i.id);
        }
    }

    // Which ones are borders? If the intersection has roads leading out of the polygon
    let mut borders = BTreeSet::new();
    let mut interior_roads = BTreeSet::new();
    for i in &intersections_inside {
        let i = map.get_i(*i);
        for r in &i.roads {
            let r = map.get_r(*r);
            if intersections_inside.contains(&r.src_i) && intersections_inside.contains(&r.dst_i) {
                interior_roads.insert(r.id);
            } else {
                borders.insert(i.id);
            }
        }
    }

    CustomBoundary {
        name,
        boundary_polygon,
        borders,
        interior_roads,
    }
}

fn render(ctx: &EventCtx, app: &App, custom: &CustomBoundary) -> Drawable {
    let mut batch = GeomBatch::new();
    //batch.push(Color::YELLOW.alpha(0.5), boundary_polygon.clone());

    let mut border_polygons = Vec::new();
    for i in &custom.borders {
        border_polygons.push(app.per_map.map.get_i(*i).polygon.clone());
    }
    /*if let Ok(p) = Polygon::convex_hull(border_polygons.clone()) {
        batch.push(Color::RED.alpha(0.5), p);
    }*/
    batch.extend(Color::BLACK, border_polygons);

    for r in &custom.interior_roads {
        batch.push(
            Color::GREEN.alpha(0.5),
            app.per_map.map.get_r(*r).get_thick_polygon(),
        );
    }

    ctx.upload(batch)
}
