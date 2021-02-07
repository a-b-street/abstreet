use std::collections::HashSet;

use rand::Rng;
use rand_xorshift::XorShiftRng;

use geom::{Distance, Polygon};
use map_model::osm;
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Panel, SimpleState,
    State, StyledButtons, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

pub struct BuildingProceduralGenerator {
    houses: Drawable,
}

impl BuildingProceduralGenerator {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut batch = GeomBatch::new();
        let mut rng = app.primary.current_flags.sim_flags.make_rng();
        for b in generate_buildings_on_empty_residential_roads(app, &mut rng) {
            batch.push(Color::RED, b);
        }

        let panel = Panel::new(Widget::row(vec![
            Line("Procedurally generated buildings")
                .small_heading()
                .draw(ctx),
            ctx.style().btn_close_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
        SimpleState::new(
            panel,
            Box::new(BuildingProceduralGenerator {
                houses: ctx.upload(batch),
            }),
        )
    }
}

impl SimpleState<App> for BuildingProceduralGenerator {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "close" => Transition::Pop,
            _ => unreachable!(),
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.houses);
    }
}

fn generate_buildings_on_empty_residential_roads(app: &App, rng: &mut XorShiftRng) -> Vec<Polygon> {
    let map = &app.primary.map;

    let mut lanes_with_buildings = HashSet::new();
    for b in map.all_buildings() {
        lanes_with_buildings.insert(b.sidewalk());
    }

    // Find all sidewalks belonging to residential roads that have no buildings
    let mut empty_sidewalks = Vec::new();
    for l in map.all_lanes() {
        if l.is_sidewalk()
            && !lanes_with_buildings.contains(&l.id)
            && map.get_r(l.parent).osm_tags.is(osm::HIGHWAY, "residential")
        {
            empty_sidewalks.push(l.id);
            //houses.push(l.lane_center_pts.make_polygons(l.width));
        }
    }

    // Walk along each sidewalk, trying to place some simple houses with a bit of setback from the
    // road.
    let mut houses = Vec::new();
    for l in empty_sidewalks {
        let lane = map.get_l(l);
        let mut dist_along = rand_dist(rng, 1.0, 5.0);
        while dist_along < lane.lane_center_pts.length() {
            let (sidewalk_pt, angle) = lane.lane_center_pts.must_dist_along(dist_along);
            let setback = rand_dist(rng, 13.0, 17.0);
            let center = sidewalk_pt.project_away(setback, angle.rotate_degs(-90.0));

            let width = rng.gen_range(4.0..7.0);
            let height = rng.gen_range(4.0..7.0);
            houses.push(
                Polygon::rectangle(width, height)
                    .rotate(angle)
                    .translate(center.x() - width / 2.0, center.y() - height / 2.0),
            );

            dist_along += Distance::meters(width.max(height)) + rand_dist(rng, 2.0, 4.0);
        }
    }

    // TODO Remove buildings that hit other ones or parks/water or roads

    houses
}

fn rand_dist(rng: &mut XorShiftRng, low: f64, high: f64) -> Distance {
    assert!(high > low);
    Distance::meters(rng.gen_range(low..high))
}
