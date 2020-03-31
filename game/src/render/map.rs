use crate::app::{App, Flags};
use crate::helpers::{ColorScheme, ID};
use crate::render::area::DrawArea;
use crate::render::building::DrawBuilding;
use crate::render::bus_stop::DrawBusStop;
use crate::render::extra_shape::{DrawExtraShape, ExtraShapeID};
use crate::render::intersection::DrawIntersection;
use crate::render::lane::DrawLane;
use crate::render::road::DrawRoad;
use crate::render::{draw_vehicle, DrawPedCrowd, DrawPedestrian, Renderable};
use aabb_quadtree::QuadTree;
use abstutil::Timer;
use ezgui::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Prerender};
use geom::{Bounds, Circle, Distance, FindClosest, Pt2D, Time};
use map_model::{
    AreaID, BuildingID, BusStopID, DirectedRoadID, Intersection, IntersectionID, LaneID, Map, Road,
    RoadID, Traversable, NORMAL_LANE_THICKNESS,
};
use sim::{GetDrawAgents, UnzoomedAgent, VehicleType};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;

pub struct DrawMap {
    pub roads: Vec<DrawRoad>,
    pub lanes: Vec<DrawLane>,
    pub intersections: Vec<DrawIntersection>,
    pub buildings: Vec<DrawBuilding>,
    pub extra_shapes: Vec<DrawExtraShape>,
    pub bus_stops: HashMap<BusStopID, DrawBusStop>,
    pub areas: Vec<DrawArea>,

    // TODO Move?
    pub agents: RefCell<AgentCache>,

    pub boundary_polygon: Drawable,
    pub draw_all_thick_roads: Drawable,
    pub draw_all_unzoomed_intersections: Drawable,
    pub draw_all_buildings: Drawable,
    pub draw_all_building_paths: Drawable,
    pub draw_all_areas: Drawable,

    quadtree: QuadTree<ID>,
}

impl DrawMap {
    pub fn new(
        map: &Map,
        flags: &Flags,
        cs: &ColorScheme,
        ctx: &EventCtx,
        timer: &mut Timer,
    ) -> DrawMap {
        let mut roads: Vec<DrawRoad> = Vec::new();
        timer.start_iter("make DrawRoads", map.all_roads().len());
        for r in map.all_roads() {
            timer.next();
            roads.push(DrawRoad::new(r, map, cs, ctx.prerender));
        }

        timer.start("generate thick roads");
        let mut road_refs: Vec<&Road> = map.all_roads().iter().collect();
        road_refs.sort_by_key(|r| r.get_zorder());
        let mut all_roads = GeomBatch::new();
        for r in road_refs {
            all_roads.push(
                osm_rank_to_color(cs, r.get_rank()),
                r.get_thick_polygon(map).get(timer),
            );
            if false {
                all_roads.push(
                    cs.get_def("unzoomed outline", Color::BLACK),
                    roads[r.id.0].get_outline(map),
                );
            }
        }
        let draw_all_thick_roads = all_roads.upload(ctx);
        timer.stop("generate thick roads");

        let almost_lanes =
            timer.parallelize("prepare DrawLanes", map.all_lanes().iter().collect(), |l| {
                DrawLane::new(
                    l,
                    map,
                    flags.draw_lane_markings,
                    cs,
                    // TODO Really parallelize should give us something thread-safe that can at
                    // least take notes.
                    &mut Timer::throwaway(),
                )
            });
        timer.start_iter("finalize DrawLanes", almost_lanes.len());
        let mut lanes: Vec<DrawLane> = Vec::new();
        for almost in almost_lanes {
            timer.next();
            let lane = map.get_l(almost.id);
            lanes.push(almost.finish(ctx.prerender, lane));
        }

        let mut intersections: Vec<DrawIntersection> = Vec::new();
        timer.start_iter("make DrawIntersections", map.all_intersections().len());
        for i in map.all_intersections() {
            timer.next();
            intersections.push(DrawIntersection::new(i, map, cs, ctx.prerender, timer));
        }

        timer.start("generate unzoomed intersections");
        let mut intersection_refs: Vec<&Intersection> = map.all_intersections().iter().collect();
        intersection_refs.sort_by_key(|i| i.get_zorder(map));
        let mut all_intersections = GeomBatch::new();
        for i in intersection_refs {
            // TODO Would be neat to show closed intersections here, but then edits need to
            // regenerate this
            if i.is_stop_sign() {
                all_intersections.push(osm_rank_to_color(cs, i.get_rank(map)), i.polygon.clone());
                if false {
                    all_intersections.push(
                        cs.get("unzoomed outline"),
                        intersections[i.id.0].get_outline(map),
                    );
                }
            } else {
                all_intersections.push(
                    cs.get_def("unzoomed interesting intersection", Color::BLACK),
                    i.polygon.clone(),
                );
            }
        }
        let draw_all_unzoomed_intersections = all_intersections.upload(ctx);
        timer.stop("generate unzoomed intersections");

        let mut buildings: Vec<DrawBuilding> = Vec::new();
        let mut all_buildings = GeomBatch::new();
        let mut all_building_paths = GeomBatch::new();
        timer.start_iter("make DrawBuildings", map.all_buildings().len());
        for b in map.all_buildings() {
            timer.next();
            buildings.push(DrawBuilding::new(
                b,
                cs,
                &mut all_buildings,
                &mut all_building_paths,
                ctx.prerender,
            ));
        }
        timer.start("upload all buildings");
        let draw_all_buildings = all_buildings.upload(ctx);
        let draw_all_building_paths = all_building_paths.upload(ctx);
        timer.stop("upload all buildings");

        let mut extra_shapes: Vec<DrawExtraShape> = Vec::new();
        if let Some(ref path) = flags.kml {
            let raw_shapes = if path.ends_with(".kml") {
                kml::load(&path, &map.get_gps_bounds(), timer)
                    .expect("Couldn't load extra KML shapes")
                    .shapes
            } else {
                let shapes: kml::ExtraShapes = abstutil::read_binary(path.clone(), timer);
                shapes.shapes
            };

            let mut closest: FindClosest<DirectedRoadID> = FindClosest::new(&map.get_bounds());
            for r in map.all_roads().iter() {
                closest.add(
                    r.id.forwards(),
                    map.right_shift(r.center_pts.clone(), NORMAL_LANE_THICKNESS)
                        .get(timer)
                        .points(),
                );
                closest.add(
                    r.id.backwards(),
                    map.left_shift(r.center_pts.clone(), NORMAL_LANE_THICKNESS)
                        .get(timer)
                        .points(),
                );
            }

            let gps_bounds = map.get_gps_bounds();
            for s in raw_shapes.into_iter() {
                if let Some(es) = DrawExtraShape::new(
                    ExtraShapeID(extra_shapes.len()),
                    s,
                    gps_bounds,
                    &closest,
                    ctx.prerender,
                    cs,
                ) {
                    extra_shapes.push(es);
                }
            }
        }

        timer.start_iter("make DrawBusStop", map.all_bus_stops().len());
        let mut bus_stops: HashMap<BusStopID, DrawBusStop> = HashMap::new();
        for s in map.all_bus_stops().values() {
            timer.next();
            bus_stops.insert(s.id, DrawBusStop::new(s, map, cs, ctx.prerender));
        }

        let mut areas: Vec<DrawArea> = Vec::new();
        let mut all_areas = GeomBatch::new();
        timer.start_iter("make DrawAreas", map.all_areas().len());
        for a in map.all_areas() {
            timer.next();
            areas.push(DrawArea::new(a, cs, &mut all_areas));
        }
        timer.start("upload all areas");
        let draw_all_areas = all_areas.upload(ctx);
        timer.stop("upload all areas");

        let boundary_polygon = ctx.prerender.upload(GeomBatch::from(vec![(
            cs.get_def("map background", Color::grey(0.87)),
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
        for obj in &extra_shapes {
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
            extra_shapes,
            bus_stops,
            areas,
            boundary_polygon,
            draw_all_thick_roads,
            draw_all_unzoomed_intersections,
            draw_all_buildings,
            draw_all_building_paths,
            draw_all_areas,

            agents: RefCell::new(AgentCache {
                time: None,
                agents_per_on: HashMap::new(),
                unzoomed: None,
            }),

            quadtree,
        }
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

    pub fn get_es(&self, id: ExtraShapeID) -> &DrawExtraShape {
        &self.extra_shapes[id.0]
    }

    pub fn get_bs(&self, id: BusStopID) -> &DrawBusStop {
        &self.bus_stops[&id]
    }

    pub fn get_a(&self, id: AreaID) -> &DrawArea {
        &self.areas[id.0]
    }

    pub fn get_obj<'a>(
        &'a self,
        id: ID,
        app: &App,
        agents: &'a mut AgentCache,
        prerender: &Prerender,
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
            ID::Turn(_) => unreachable!(),
            ID::Building(id) => {
                return Some(self.get_b(id));
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
            ID::ExtraShape(id) => {
                return Some(self.get_es(id));
            }
            ID::BusStop(id) => {
                return Some(self.get_bs(id));
            }
            ID::Area(id) => {
                return Some(self.get_a(id));
            }
        };

        agents.populate_if_needed(on, &app.primary.map, &app.primary.sim, &app.cs, prerender);

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

pub struct AgentCache {
    // This time applies to agents_per_on. unzoomed has its own possibly separate Time!
    time: Option<Time>,
    agents_per_on: HashMap<Traversable, Vec<Box<dyn Renderable>>>,
    // agent radius also matters
    unzoomed: Option<(Time, Distance, AgentColorScheme, Drawable)>,
}

impl AgentCache {
    pub fn get(&self, on: Traversable) -> Vec<&dyn Renderable> {
        self.agents_per_on[&on]
            .iter()
            .map(|obj| obj.borrow())
            .collect()
    }

    pub fn populate_if_needed(
        &mut self,
        on: Traversable,
        map: &Map,
        source: &dyn GetDrawAgents,
        cs: &ColorScheme,
        prerender: &Prerender,
    ) {
        let now = source.time();
        if Some(now) == self.time && self.agents_per_on.contains_key(&on) {
            return;
        }
        let step_count = source.step_count();

        let mut list: Vec<Box<dyn Renderable>> = Vec::new();
        for c in source.get_draw_cars(on, map).into_iter() {
            list.push(draw_vehicle(c, map, prerender, cs));
        }
        let (loners, crowds) = source.get_draw_peds(on, map);
        for p in loners {
            list.push(Box::new(DrawPedestrian::new(
                p, step_count, map, prerender, cs,
            )));
        }
        for c in crowds {
            list.push(Box::new(DrawPedCrowd::new(c, map, prerender, cs)));
        }

        if Some(now) != self.time {
            self.agents_per_on.clear();
            self.time = Some(now);
        }

        self.agents_per_on.insert(on, list);
    }

    // TODO GetDrawAgents indirection added for time traveling, but that's been removed. Maybe
    // simplify this.
    pub fn draw_unzoomed_agents(
        &mut self,
        source: &dyn GetDrawAgents,
        map: &Map,
        acs: &AgentColorScheme,
        g: &mut GfxCtx,
        radius: Distance,
    ) {
        let now = source.time();
        if let Some((time, r, ref orig_acs, ref draw)) = self.unzoomed {
            if now == time && radius == r && acs == orig_acs {
                g.redraw(draw);
                return;
            }
        }

        // It's quite silly to produce triangles for the same circle over and over again. ;)
        let circle = Circle::new(Pt2D::new(0.0, 0.0), radius).to_polygon();
        let mut batch = GeomBatch::new();
        for agent in source.get_unzoomed_agents(map) {
            if let Some(color) = acs.color(&agent) {
                batch.push(color, circle.translate(agent.pos.x(), agent.pos.y()));
            }
        }

        let draw = g.upload(batch);
        g.redraw(&draw);
        self.unzoomed = Some((now, radius, acs.clone(), draw));
    }
}

fn osm_rank_to_color(cs: &ColorScheme, rank: usize) -> Color {
    if rank >= 16 {
        cs.get_def("unzoomed highway road", Color::rgb(232, 146, 162))
    } else if rank >= 6 {
        cs.get_def("unzoomed arterial road", Color::rgb(255, 199, 62))
    } else {
        cs.get_def("unzoomed residential road", Color::WHITE)
    }
}

#[derive(PartialEq, Clone)]
pub struct AgentColorScheme {
    // TODO Could consider specializing this more?
    pub rows: Vec<(String, Color, bool)>,
}

impl AgentColorScheme {
    pub fn new(cs: &ColorScheme) -> AgentColorScheme {
        AgentColorScheme {
            rows: vec![
                (
                    "car".to_string(),
                    cs.get_def("unzoomed car", Color::hex("#A32015")).alpha(0.8),
                    true,
                ),
                (
                    "bike".to_string(),
                    cs.get_def("unzoomed bike", Color::hex("#5D9630"))
                        .alpha(0.8),
                    true,
                ),
                (
                    "bus".to_string(),
                    cs.get_def("unzoomed bus", Color::hex("#12409D")).alpha(0.8),
                    true,
                ),
                (
                    "pedestrian".to_string(),
                    cs.get_def("unzoomed pedestrian", Color::hex("#DF8C3D").alpha(0.8)),
                    true,
                ),
            ],
        }
    }

    pub fn toggle(&mut self, name: String) {
        for (n, _, enabled) in &mut self.rows {
            if &name == n {
                *enabled = !*enabled;
                return;
            }
        }
        panic!("Can't toggle category {}", name);
    }

    fn color(&self, agent: &UnzoomedAgent) -> Option<Color> {
        let category = match agent.vehicle_type {
            Some(VehicleType::Car) => "car".to_string(),
            Some(VehicleType::Bike) => "bike".to_string(),
            Some(VehicleType::Bus) => "bus".to_string(),
            None => "pedestrian".to_string(),
        };
        for (name, color, enabled) in &self.rows {
            if name == &category {
                if *enabled {
                    return Some(*color);
                }
                return None;
            }
        }
        panic!("Unknown AgentColorScheme category {}", category);
    }
}
