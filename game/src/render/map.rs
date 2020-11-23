use std::collections::HashMap;

use aabb_quadtree::QuadTree;

use abstutil::Timer;
use geom::{Bounds, Polygon};
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, Map, ParkingLotID, RoadID};
use widgetry::{Color, Drawable, EventCtx, GeomBatch};

use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::options::Options;
use crate::render::building::DrawBuilding;
use crate::render::bus_stop::DrawBusStop;
use crate::render::intersection::DrawIntersection;
use crate::render::lane::DrawLane;
use crate::render::parking_lot::DrawParkingLot;
use crate::render::road::DrawRoad;
use crate::render::{AgentCache, DrawArea, Renderable};

pub struct DrawMap {
    pub roads: Vec<DrawRoad>,
    pub lanes: Vec<DrawLane>,
    pub intersections: Vec<DrawIntersection>,
    pub buildings: Vec<DrawBuilding>,
    pub parking_lots: Vec<DrawParkingLot>,
    pub bus_stops: HashMap<BusStopID, DrawBusStop>,
    pub areas: Vec<DrawArea>,

    pub boundary_polygon: Drawable,
    pub draw_all_unzoomed_roads_and_intersections: Drawable,
    pub draw_all_buildings: Drawable,
    pub draw_all_building_paths: Drawable,
    pub draw_all_building_outlines: Drawable,
    pub draw_all_unzoomed_parking_lots: Drawable,
    pub draw_all_areas: Drawable,

    pub zorder_range: (isize, isize),

    quadtree: QuadTree<ID>,
}

impl DrawMap {
    pub fn new(
        map: &Map,
        opts: &Options,
        cs: &ColorScheme,
        ctx: &EventCtx,
        timer: &mut Timer,
    ) -> DrawMap {
        let mut roads: Vec<DrawRoad> = Vec::new();
        let mut low_z = 0;
        let mut high_z = 0;
        timer.start_iter("make DrawRoads", map.all_roads().len());
        for r in map.all_roads() {
            timer.next();
            roads.push(DrawRoad::new(r));
            low_z = low_z.min(r.zorder);
            high_z = high_z.max(r.zorder);
        }

        let mut lanes: Vec<DrawLane> = Vec::new();
        timer.start_iter("make DrawLanes", map.all_lanes().len());
        for l in map.all_lanes() {
            timer.next();
            lanes.push(DrawLane::new(l, map));
        }

        let mut intersections: Vec<DrawIntersection> = Vec::new();
        timer.start_iter("make DrawIntersections", map.all_intersections().len());
        for i in map.all_intersections() {
            timer.next();
            intersections.push(DrawIntersection::new(i, map));
        }

        let draw_all_unzoomed_roads_and_intersections =
            DrawMap::regenerate_unzoomed_layer(map, cs, ctx, timer);

        let mut buildings: Vec<DrawBuilding> = Vec::new();
        let mut all_buildings = GeomBatch::new();
        let mut all_building_paths = GeomBatch::new();
        let mut all_building_outlines = GeomBatch::new();
        timer.start_iter("make DrawBuildings", map.all_buildings().len());
        for b in map.all_buildings() {
            timer.next();
            buildings.push(DrawBuilding::new(
                ctx,
                b,
                map,
                cs,
                opts,
                &mut all_buildings,
                &mut all_building_paths,
                &mut all_building_outlines,
            ));
        }
        timer.start("upload all buildings");
        let draw_all_buildings = all_buildings.upload(ctx);
        let draw_all_building_paths = all_building_paths.upload(ctx);
        let draw_all_building_outlines = all_building_outlines.upload(ctx);
        timer.stop("upload all buildings");

        timer.start("make DrawParkingLot");
        let mut parking_lots: Vec<DrawParkingLot> = Vec::new();
        let mut all_unzoomed_parking_lots = GeomBatch::new();
        for pl in map.all_parking_lots() {
            parking_lots.push(DrawParkingLot::new(
                ctx,
                pl,
                cs,
                &mut all_unzoomed_parking_lots,
            ));
        }
        let draw_all_unzoomed_parking_lots = all_unzoomed_parking_lots.upload(ctx);
        timer.stop("make DrawParkingLot");

        timer.start_iter("make DrawBusStop", map.all_bus_stops().len());
        let mut bus_stops: HashMap<BusStopID, DrawBusStop> = HashMap::new();
        for s in map.all_bus_stops().values() {
            timer.next();
            bus_stops.insert(s.id, DrawBusStop::new(ctx, s, map, cs));
        }

        let mut areas: Vec<DrawArea> = Vec::new();
        let mut all_areas = GeomBatch::new();
        timer.start_iter("make DrawAreas", map.all_areas().len());
        for a in map.all_areas() {
            timer.next();
            areas.push(DrawArea::new(ctx, a, cs, &mut all_areas));
        }
        timer.start("upload all areas");
        let draw_all_areas = all_areas.upload(ctx);
        timer.stop("upload all areas");

        let boundary_polygon = ctx.upload(GeomBatch::from(vec![(
            cs.map_background.clone(),
            map.get_boundary_polygon().clone(),
        )]));

        timer.start("create quadtree");
        let mut quadtree = QuadTree::default(map.get_bounds().as_bbox());
        // TODO use iter chain if everything was boxed as a renderable...
        for obj in &roads {
            quadtree.insert_with_box(obj.get_id(), obj.get_outline(map).get_bounds().as_bbox());
        }
        for obj in &lanes {
            quadtree.insert_with_box(obj.get_id(), obj.get_outline(map).get_bounds().as_bbox());
        }
        for obj in &intersections {
            quadtree.insert_with_box(obj.get_id(), obj.get_outline(map).get_bounds().as_bbox());
        }
        for obj in &buildings {
            quadtree.insert_with_box(obj.get_id(), obj.get_outline(map).get_bounds().as_bbox());
        }
        for obj in &parking_lots {
            quadtree.insert_with_box(obj.get_id(), obj.get_outline(map).get_bounds().as_bbox());
        }
        // Don't put BusStops in the quadtree
        for obj in &areas {
            quadtree.insert_with_box(obj.get_id(), obj.get_outline(map).get_bounds().as_bbox());
        }
        timer.stop("create quadtree");

        timer.note(format!(
            "static DrawMap consumes {} MB on the GPU",
            abstutil::prettyprint_usize(ctx.prerender.get_total_bytes_uploaded() / 1024 / 1024)
        ));

        DrawMap {
            roads,
            lanes,
            intersections,
            buildings,
            parking_lots,
            bus_stops,
            areas,
            boundary_polygon,
            draw_all_unzoomed_roads_and_intersections,
            draw_all_buildings,
            draw_all_building_paths,
            draw_all_building_outlines,
            draw_all_unzoomed_parking_lots,
            draw_all_areas,

            quadtree,

            zorder_range: (low_z, high_z),
        }
    }

    pub fn regenerate_unzoomed_layer(
        map: &Map,
        cs: &ColorScheme,
        ctx: &EventCtx,
        timer: &mut Timer,
    ) -> Drawable {
        timer.start("generate unzoomed roads and intersections");
        let mut unzoomed_pieces: Vec<(isize, Polygon, Color)> = Vec::new();
        for r in map.all_roads() {
            unzoomed_pieces.push((
                r.zorder,
                r.get_thick_polygon(map),
                if r.is_light_rail() {
                    cs.light_rail_track
                } else if r.is_private() {
                    cs.private_road
                } else {
                    cs.unzoomed_road_surface(r.get_rank())
                },
            ));
        }
        for i in map.all_intersections() {
            unzoomed_pieces.push((
                i.get_zorder(map),
                i.polygon.clone(),
                if i.is_stop_sign() {
                    if i.is_light_rail(map) {
                        cs.light_rail_track
                    } else if i.is_private(map) {
                        cs.private_road
                    } else {
                        cs.unzoomed_road_surface(i.get_rank(map))
                    }
                } else {
                    cs.unzoomed_interesting_intersection
                },
            ));
        }
        unzoomed_pieces.sort_by_key(|(z, _, _)| *z);
        let mut unzoomed_batch = GeomBatch::new();
        for (_, poly, color) in unzoomed_pieces {
            unzoomed_batch.push(color, poly);
        }
        let draw_all_unzoomed_roads_and_intersections = unzoomed_batch.upload(ctx);
        timer.stop("generate unzoomed roads and intersections");
        draw_all_unzoomed_roads_and_intersections
    }

    // The alt to these is implementing std::ops::Index, but that's way more verbose!
    pub fn get_r(&self, id: RoadID) -> &DrawRoad {
        &self.roads[id.0]
    }

    pub fn get_l(&self, id: LaneID) -> &DrawLane {
        &self.lanes[id.0]
    }

    pub fn get_i(&self, id: IntersectionID) -> &DrawIntersection {
        &self.intersections[id.0]
    }

    pub fn get_b(&self, id: BuildingID) -> &DrawBuilding {
        &self.buildings[id.0]
    }

    pub fn get_pl(&self, id: ParkingLotID) -> &DrawParkingLot {
        &self.parking_lots[id.0]
    }

    pub fn get_bs(&self, id: BusStopID) -> &DrawBusStop {
        &self.bus_stops[&id]
    }

    pub fn get_a(&self, id: AreaID) -> &DrawArea {
        &self.areas[id.0]
    }

    pub fn get_obj<'a>(
        &'a self,
        ctx: &EventCtx,
        id: ID,
        app: &App,
        agents: &'a mut AgentCache,
    ) -> Option<&'a dyn Renderable> {
        let on = match id {
            ID::Road(id) => {
                return Some(self.get_r(id));
            }
            ID::Lane(id) => {
                return Some(self.get_l(id));
            }
            ID::Intersection(id) => {
                return Some(self.get_i(id));
            }
            ID::Building(id) => {
                return Some(self.get_b(id));
            }
            ID::ParkingLot(id) => {
                return Some(self.get_pl(id));
            }
            ID::Car(id) => {
                // Cars might be parked in a garage!
                app.primary.sim.get_draw_car(id, &app.primary.map)?.on
            }
            ID::Pedestrian(id) => {
                app.primary
                    .sim
                    .get_draw_ped(id, &app.primary.map)
                    .unwrap()
                    .on
            }
            ID::PedCrowd(ref members) => {
                // If the first member has vanished, just give up
                app.primary
                    .sim
                    .get_draw_ped(members[0], &app.primary.map)?
                    .on
            }
            ID::BusStop(id) => {
                return Some(self.get_bs(id));
            }
            ID::Area(id) => {
                return Some(self.get_a(id));
            }
        };

        agents.populate_if_needed(
            on,
            &app.primary.map,
            &app.primary.sim,
            &app.cs,
            ctx.prerender,
        );

        // Why might this fail? Pedestrians merge into crowds, and crowds dissipate into
        // individuals
        agents.get(on).into_iter().find(|r| r.get_id() == id)
    }

    // Unsorted, unexpanded, raw result.
    pub fn get_matching_objects(&self, bounds: Bounds) -> Vec<ID> {
        let mut results: Vec<ID> = Vec::new();
        for &(id, _, _) in &self.quadtree.query(bounds.as_bbox()) {
            results.push(id.clone());
        }
        results
    }
}
