use std::collections::HashSet;

use aabb_quadtree::QuadTree;
use geojson::{Feature, FeatureCollection, GeoJson};
use rand::Rng;
use rand_xorshift::XorShiftRng;

use abstutil::Timer;
use geom::{Distance, Polygon};
use map_model::{osm, Map};
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
        let houses = ctx.loading_screen("generate buildings", |_, mut timer| {
            generate_buildings_on_empty_residential_roads(&app.primary.map, &mut rng, &mut timer)
        });
        let mut features = Vec::new();
        for poly in houses {
            features.push(Feature {
                bbox: None,
                geometry: Some(poly.to_geojson(Some(app.primary.map.get_gps_bounds()))),
                id: None,
                properties: None,
                foreign_members: None,
            });

            batch.push(Color::RED, poly);
        }
        let num_houses = features.len();
        let geojson = GeoJson::from(FeatureCollection {
            bbox: None,
            features,
            foreign_members: None,
        });
        abstio::write_json("procgen_houses.json".to_string(), &geojson);

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line("Procedurally generated buildings")
                    .small_heading()
                    .draw(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            format!(
                "Generated {} houses",
                abstutil::prettyprint_usize(num_houses)
            )
            .draw_text(ctx),
            "Wrote results to procgen_houses.json".draw_text(ctx),
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

fn generate_buildings_on_empty_residential_roads(
    map: &Map,
    rng: &mut XorShiftRng,
    timer: &mut Timer,
) -> Vec<Polygon> {
    timer.start("initially place buildings");
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
            let width = rng.gen_range(6.0..14.0);
            let height = rng.gen_range(6.0..14.0);

            // Make it so that the front of the house is always set back a fixed amount. So account
            // for the chosen "height".
            let setback = Distance::meters(10.0) + Distance::meters(height / 2.0);
            let center = sidewalk_pt.project_away(setback, angle.rotate_degs(-90.0));

            houses.push(
                Polygon::rectangle(width, height)
                    .rotate(angle)
                    .translate(center.x() - width / 2.0, center.y() - height / 2.0),
            );

            dist_along += Distance::meters(width.max(height)) + rand_dist(rng, 2.0, 4.0);
        }
    }
    timer.stop("initially place buildings");

    // Remove buildings that hit each other. Build up the quadtree of finalized houses as we go,
    // using index as the ID.
    let mut non_overlapping = Vec::new();
    let mut quadtree = QuadTree::default(map.get_bounds().as_bbox());
    timer.start_iter("prune buildings overlapping each other", houses.len());
    'HOUSE: for poly in houses {
        timer.next();
        let mut search = poly.get_bounds();
        search.add_buffer(Distance::meters(1.0));
        for (idx, _, _) in quadtree.query(search.as_bbox()) {
            if poly.intersects(&non_overlapping[*idx]) {
                continue 'HOUSE;
            }
        }
        quadtree.insert_with_box(non_overlapping.len(), poly.get_bounds().as_bbox());
        non_overlapping.push(poly);
    }

    // Create a different quadtree, just containing static things in the map that we don't want
    // new buildings to hit. The index is just into a list of polygons.
    quadtree = QuadTree::default(map.get_bounds().as_bbox());
    let mut static_polygons = Vec::new();
    for r in map.all_roads() {
        let poly = r.get_thick_polygon(map);
        quadtree.insert_with_box(static_polygons.len(), poly.get_bounds().as_bbox());
        static_polygons.push(poly);
    }
    for i in map.all_intersections() {
        quadtree.insert_with_box(static_polygons.len(), i.polygon.get_bounds().as_bbox());
        static_polygons.push(i.polygon.clone());
    }
    for b in map.all_buildings() {
        quadtree.insert_with_box(static_polygons.len(), b.polygon.get_bounds().as_bbox());
        static_polygons.push(b.polygon.clone());
    }
    for pl in map.all_parking_lots() {
        quadtree.insert_with_box(static_polygons.len(), pl.polygon.get_bounds().as_bbox());
        static_polygons.push(pl.polygon.clone());
    }
    for a in map.all_areas() {
        quadtree.insert_with_box(static_polygons.len(), a.polygon.get_bounds().as_bbox());
        static_polygons.push(a.polygon.clone());
    }

    let mut survivors = Vec::new();
    timer.start_iter(
        "prune buildings overlapping the basemap",
        non_overlapping.len(),
    );
    'NON_OVERLAP: for poly in non_overlapping {
        timer.next();
        for (idx, _, _) in quadtree.query(poly.get_bounds().as_bbox()) {
            if poly.intersects(&static_polygons[*idx]) {
                continue 'NON_OVERLAP;
            }
        }
        survivors.push(poly);
    }
    survivors
}

fn rand_dist(rng: &mut XorShiftRng, low: f64, high: f64) -> Distance {
    assert!(high > low);
    Distance::meters(rng.gen_range(low..high))
}
