use std::collections::BTreeSet;

use map_gui::tools::{CityPicker, DrawRoadLabels};
use map_gui::ID;
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, State,
    TextExt, VerticalAlignment, Widget,
};

use super::{Neighborhood, Viewer};
use crate::app::{App, Transition};
use crate::common::intersections_from_roads;

pub struct BrowseNeighborhoods {
    panel: Panel,
    draw_neighborhoods: Drawable,
    labels: DrawRoadLabels,
}

impl BrowseNeighborhoods {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("LTN tool").small_heading().into_widget(ctx),
                map_gui::tools::change_map_btn(ctx, app)
                    .centered_vert()
                    .align_right(),
            ]),
            "Click a neighborhood".text_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        let draw_neighborhoods = calculate_neighborhoods(app).upload(ctx);
        Box::new(BrowseNeighborhoods {
            panel,
            draw_neighborhoods,
            labels: DrawRoadLabels::only_major_roads(),
        })
    }
}

impl State<App> for BrowseNeighborhoods {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection =
                match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                    x @ Some(ID::Road(_)) => x,
                    Some(ID::Lane(l)) => Some(ID::Road(l.road)),
                    _ => None,
                };
        }
        if let Some(ID::Road(r)) = app.primary.current_selection {
            if Neighborhood::is_interior_road(r, &app.primary.map) && ctx.normal_left_click() {
                return Transition::Replace(Viewer::start_from_road(ctx, app, r));
            }
        }

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "change map" => {
                    return Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Replace(BrowseNeighborhoods::new_state(ctx, app))
                        }),
                    ));
                }
                _ => unreachable!(),
            }
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        g.redraw(&self.draw_neighborhoods);
        self.labels.draw(g, app);
    }
}

fn calculate_neighborhoods(app: &App) -> GeomBatch {
    let map = &app.primary.map;
    let mut unvisited = BTreeSet::new();

    let mut batch = GeomBatch::new();
    let colors = [
        Color::BLUE,
        Color::ORANGE,
        Color::PURPLE,
        Color::RED,
        Color::GREEN,
        Color::CYAN,
    ];
    let mut num_neighborhoods = 0;

    for r in map.all_roads() {
        if Neighborhood::is_interior_road(r.id, map) {
            unvisited.insert(r.id);
        }
    }

    while !unvisited.is_empty() {
        let start = *unvisited.iter().next().unwrap();
        let neighborhood = Neighborhood::from_road(map, start);

        // TODO Either use that 4-color theorem and actually guarantee no adjacent same-color ones,
        // or change the style to have a clear outline around each
        let color = colors[num_neighborhoods % colors.len()];
        num_neighborhoods += 1;
        for i in intersections_from_roads(&neighborhood.interior, map) {
            batch.push(color, map.get_i(i).polygon.clone());
        }
        for r in neighborhood.interior {
            batch.push(color, map.get_r(r).get_thick_polygon());
            unvisited.remove(&r);
        }
    }

    batch
}
