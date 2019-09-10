use crate::common::ColorLegend;
use crate::helpers::{rotating_color, ColorScheme, ID};
use crate::render::area::DrawArea;
use crate::render::building::DrawBuilding;
use crate::render::bus_stop::DrawBusStop;
use crate::render::extra_shape::{DrawExtraShape, ExtraShapeID};
use crate::render::intersection::DrawIntersection;
use crate::render::lane::DrawLane;
use crate::render::road::DrawRoad;
use crate::render::turn::DrawTurn;
use crate::render::Renderable;
use crate::ui::{Flags, PerMapUI};
use aabb_quadtree::QuadTree;
use abstutil::{Cloneable, Timer};
use ezgui::{Color, Drawable, EventCtx, GeomBatch, GfxCtx};
use geom::{Bounds, Circle, Distance, Duration, FindClosest};
use map_model::{
    AreaID, BuildingID, BusStopID, DirectedRoadID, IntersectionID, Lane, LaneID, Map, RoadID,
    Traversable, Turn, TurnID, TurnType, LANE_THICKNESS,
};
use sim::{
    AgentMetadata, CarStatus, DrawCarInput, DrawPedestrianInput, UnzoomedAgent, VehicleType,
};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;

pub struct DrawMap {
    pub roads: Vec<DrawRoad>,
    pub lanes: Vec<DrawLane>,
    pub intersections: Vec<DrawIntersection>,
    pub turns: HashMap<TurnID, DrawTurn>,
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
        let mut all_roads = GeomBatch::new();
        timer.start_iter("make DrawRoads", map.all_roads().len());
        for r in map.all_roads() {
            timer.next();
            let draw_r = DrawRoad::new(r, cs, ctx.prerender);
            all_roads.push(
                osm_rank_to_color(cs, r.get_rank()),
                r.get_thick_polygon().get(timer),
            );
            all_roads.push(
                cs.get_def("unzoomed outline", Color::BLACK),
                draw_r.get_outline(map),
            );
            roads.push(draw_r);
        }
        timer.start("upload thick roads");
        let draw_all_thick_roads = ctx.prerender.upload(all_roads);
        timer.stop("upload thick roads");

        let almost_lanes =
            timer.parallelize("prepare DrawLanes", map.all_lanes().iter().collect(), |l| {
                DrawLane::new(
                    l,
                    map,
                    !flags.dont_draw_lane_markings,
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
            lanes.push(almost.finish(ctx.prerender));
        }

        timer.start_iter("compute_turn_to_lane_offset", map.all_lanes().len());
        let mut turn_to_lane_offset: HashMap<TurnID, usize> = HashMap::new();
        for l in map.all_lanes() {
            timer.next();
            DrawMap::compute_turn_to_lane_offset(&mut turn_to_lane_offset, l, map);
        }

        timer.start_iter("make DrawTurns", map.all_turns().len());
        let mut turns: HashMap<TurnID, DrawTurn> = HashMap::new();
        for t in map.all_turns().values() {
            timer.next();
            // There's never a reason to draw these icons; the turn priority is only ever Priority,
            // since they can't conflict with anything.
            if t.turn_type != TurnType::SharedSidewalkCorner {
                turns.insert(t.id, DrawTurn::new(map, t, turn_to_lane_offset[&t.id]));
            }
        }

        let mut intersections: Vec<DrawIntersection> = Vec::new();
        let mut all_intersections = GeomBatch::new();
        timer.start_iter("make DrawIntersections", map.all_intersections().len());
        for i in map.all_intersections() {
            timer.next();
            let draw_i = DrawIntersection::new(i, map, cs, ctx.prerender, timer);
            if i.is_stop_sign() {
                all_intersections.push(osm_rank_to_color(cs, i.get_rank(map)), i.polygon.clone());
                all_intersections.push(cs.get("unzoomed outline"), draw_i.get_outline(map));
            } else {
                all_intersections.push(
                    cs.get_def("unzoomed interesting intersection", Color::BLACK),
                    i.polygon.clone(),
                );
            }
            intersections.push(draw_i);
        }
        timer.start("upload all intersections");
        let draw_all_unzoomed_intersections = ctx.prerender.upload(all_intersections);
        timer.stop("upload all intersections");

        let mut buildings: Vec<DrawBuilding> = Vec::new();
        let mut all_buildings = GeomBatch::new();
        timer.start_iter("make DrawBuildings", map.all_buildings().len());
        for b in map.all_buildings() {
            timer.next();
            buildings.push(DrawBuilding::new(b, cs, &mut all_buildings));
        }
        timer.start("upload all buildings");
        let draw_all_buildings = ctx.prerender.upload(all_buildings);
        timer.stop("upload all buildings");

        let mut extra_shapes: Vec<DrawExtraShape> = Vec::new();
        if let Some(ref path) = flags.kml {
            let raw_shapes = if path.ends_with(".kml") {
                kml::load(&path, &map.get_gps_bounds(), timer)
                    .expect("Couldn't load extra KML shapes")
                    .shapes
            } else {
                let shapes: kml::ExtraShapes =
                    abstutil::read_binary(&path, timer).expect("Couldn't load ExtraShapes");
                shapes.shapes
            };

            let mut closest: FindClosest<DirectedRoadID> = FindClosest::new(&map.get_bounds());
            for r in map.all_roads().iter() {
                closest.add(
                    r.id.forwards(),
                    r.center_pts.shift_right(LANE_THICKNESS).get(timer).points(),
                );
                closest.add(
                    r.id.backwards(),
                    r.center_pts.shift_left(LANE_THICKNESS).get(timer).points(),
                );
            }

            let gps_bounds = map.get_gps_bounds();
            for s in raw_shapes.into_iter() {
                if let Some(es) =
                    DrawExtraShape::new(ExtraShapeID(extra_shapes.len()), s, gps_bounds, &closest)
                {
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
            areas.push(DrawArea::new(a, ctx, &mut all_areas));
        }
        timer.start("upload all areas");
        let draw_all_areas = ctx.prerender.upload(all_areas);
        timer.stop("upload all areas");

        let boundary_polygon = ctx.prerender.upload_borrowed(vec![(
            cs.get_def("map background", Color::rgb(242, 239, 233)),
            map.get_boundary_polygon(),
        )]);

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
            turns,
            buildings,
            extra_shapes,
            bus_stops,
            areas,
            boundary_polygon,
            draw_all_thick_roads,
            draw_all_unzoomed_intersections,
            draw_all_buildings,
            draw_all_areas,

            agents: RefCell::new(AgentCache {
                time: None,
                agents_per_on: HashMap::new(),
                unzoomed: None,
            }),

            quadtree,
        }
    }

    pub fn compute_turn_to_lane_offset(result: &mut HashMap<TurnID, usize>, l: &Lane, map: &Map) {
        // Split into two groups, based on the endpoint
        let mut pair: (Vec<&Turn>, Vec<&Turn>) = map
            .get_turns_from_lane(l.id)
            .iter()
            .filter(|t| t.turn_type != TurnType::SharedSidewalkCorner)
            .partition(|t| t.id.parent == l.dst_i);

        // Sort the turn icons by angle.
        pair.0
            .sort_by_key(|t| t.angle().normalized_degrees() as i64);
        pair.1
            .sort_by_key(|t| t.angle().normalized_degrees() as i64);

        for (idx, t) in pair.0.iter().enumerate() {
            result.insert(t.id, idx);
        }
        for (idx, t) in pair.1.iter().enumerate() {
            result.insert(t.id, idx);
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

    pub fn get_t(&self, id: TurnID) -> &DrawTurn {
        &self.turns[&id]
    }

    pub fn get_turns(&self, i: IntersectionID, map: &Map) -> Vec<&DrawTurn> {
        let mut results = Vec::new();
        for t in &map.get_i(i).turns {
            if map.get_t(*t).turn_type != TurnType::SharedSidewalkCorner {
                results.push(self.get_t(*t));
            }
        }
        results
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
    time: Option<Duration>,
    agents_per_on: HashMap<Traversable, Vec<Box<dyn Renderable>>>,
    // cam_zoom also matters
    unzoomed: Option<(f64, Drawable)>,
}

impl AgentCache {
    pub fn has(&self, now: Duration, on: Traversable) -> bool {
        if Some(now) != self.time {
            return false;
        }
        self.agents_per_on.contains_key(&on)
    }

    // Must call has() first.
    pub fn get(&self, on: Traversable) -> Vec<&dyn Renderable> {
        self.agents_per_on[&on]
            .iter()
            .map(|obj| obj.borrow())
            .collect()
    }

    pub fn put(&mut self, now: Duration, on: Traversable, agents: Vec<Box<dyn Renderable>>) {
        if Some(now) != self.time {
            self.agents_per_on.clear();
            self.time = Some(now);
        }

        assert!(!self.agents_per_on.contains_key(&on));
        self.agents_per_on.insert(on, agents);
    }

    pub fn invalidate_cache(&mut self) {
        self.time = None;
        self.agents_per_on.clear();
        self.unzoomed = None;
    }

    pub fn draw_unzoomed_agents(
        &mut self,
        primary: &PerMapUI,
        acs: AgentColorScheme,
        cs: &ColorScheme,
        g: &mut GfxCtx,
    ) {
        let now = primary.sim.time();
        if let Some((z, ref draw)) = self.unzoomed {
            if g.canvas.cam_zoom == z && Some(now) == self.time {
                g.redraw(draw);
                return;
            }
        }

        // TODO The perf is a little slow compared to when we just returned a bunch of Pt2Ds
        // without the extra data. Try plumbing a callback that directly populates batch.
        let mut batch = GeomBatch::new();
        let radius = Distance::meters(10.0) / g.canvas.cam_zoom;
        for agent in primary.sim.get_unzoomed_agents(&primary.map) {
            batch.push(
                acs.unzoomed_color(&agent, cs),
                Circle::new(agent.pos, radius).to_polygon(),
            );
        }

        let draw = g.upload(batch);
        g.redraw(&draw);
        self.unzoomed = Some((g.canvas.cam_zoom, draw));
        if Some(now) != self.time {
            self.agents_per_on.clear();
            self.time = Some(now);
        }
    }
}

fn osm_rank_to_color(cs: &ColorScheme, rank: usize) -> Color {
    if rank >= 16 {
        cs.get_def("unzoomed highway road", Color::rgb(232, 146, 162))
    } else if rank >= 6 {
        cs.get_def("unzoomed arterial road", Color::rgb(247, 250, 191))
    } else {
        cs.get_def("unzoomed residential road", Color::WHITE)
    }
}

// TODO Show a little legend when it's first activated.
// TODO ETA till goal...
#[derive(Clone, Copy, PartialEq)]
pub enum AgentColorScheme {
    VehicleTypes,
    Delay,
    DistanceCrossedSoFar,
    TripTimeSoFar,
}

impl Cloneable for AgentColorScheme {}

impl AgentColorScheme {
    pub fn unzoomed_color(self, agent: &UnzoomedAgent, cs: &ColorScheme) -> Color {
        match self {
            AgentColorScheme::VehicleTypes => match agent.vehicle_type {
                Some(VehicleType::Car) => cs.get_def("unzoomed car", Color::RED.alpha(0.5)),
                Some(VehicleType::Bike) => cs.get_def("unzoomed bike", Color::GREEN.alpha(0.5)),
                Some(VehicleType::Bus) => cs.get_def("unzoomed bus", Color::BLUE.alpha(0.5)),
                None => cs.get_def("unzoomed pedestrian", Color::ORANGE.alpha(0.5)),
            },
            _ => self.by_metadata(&agent.metadata),
        }
    }

    pub fn zoomed_color_car(self, input: &DrawCarInput, cs: &ColorScheme) -> Color {
        match self {
            AgentColorScheme::VehicleTypes => {
                if input.id.1 == VehicleType::Bus {
                    cs.get_def("bus", Color::rgb(50, 133, 117))
                } else {
                    match input.status {
                        CarStatus::Debug => cs.get_def("debug car", Color::BLUE.alpha(0.8)),
                        CarStatus::Moving => cs.get_def("moving car", Color::CYAN),
                        CarStatus::Stuck => cs.get_def("stuck car", Color::rgb(222, 184, 135)),
                        CarStatus::Parked => cs.get_def("parked car", Color::rgb(180, 233, 76)),
                    }
                }
            }
            _ => self.by_metadata(&input.metadata),
        }
    }

    pub fn zoomed_color_bike(self, input: &DrawCarInput, cs: &ColorScheme) -> Color {
        match self {
            AgentColorScheme::VehicleTypes => match input.status {
                CarStatus::Debug => cs.get_def("debug bike", Color::BLUE.alpha(0.8)),
                // TODO Hard to see on the greenish bike lanes? :P
                CarStatus::Moving => cs.get_def("moving bike", Color::GREEN),
                CarStatus::Stuck => cs.get_def("stuck bike", Color::RED),
                CarStatus::Parked => panic!("Can't have a parked bike {}", input.id),
            },
            _ => self.by_metadata(&input.metadata),
        }
    }

    pub fn zoomed_color_ped(self, input: &DrawPedestrianInput, cs: &ColorScheme) -> Color {
        match self {
            AgentColorScheme::VehicleTypes => {
                if input.preparing_bike {
                    cs.get_def("pedestrian preparing bike", Color::rgb(255, 0, 144))
                } else {
                    cs.get_def("pedestrian", Color::rgb_f(0.2, 0.7, 0.7))
                }
            }
            _ => self.by_metadata(&input.metadata),
        }
    }

    fn by_metadata(self, md: &AgentMetadata) -> Color {
        match self {
            AgentColorScheme::VehicleTypes => unreachable!(),
            AgentColorScheme::Delay => delay_color(md.time_spent_blocked),
            AgentColorScheme::DistanceCrossedSoFar => percent_color(md.percent_dist_crossed),
            AgentColorScheme::TripTimeSoFar => delay_color(md.trip_time_so_far),
        }
    }

    // TODO Lots of duplicated values here. :\
    pub fn make_color_legend(self, cs: &ColorScheme) -> ColorLegend {
        match self {
            AgentColorScheme::VehicleTypes => ColorLegend::new(
                "vehicle types",
                vec![
                    ("car", cs.get("unzoomed car")),
                    ("bike", cs.get("unzoomed bike")),
                    ("bus", cs.get("unzoomed bus")),
                    ("pedestrian", cs.get("unzoomed pedestrian")),
                ],
            ),
            AgentColorScheme::Delay => ColorLegend::new(
                "time spent delayed/blocked",
                vec![
                    ("<= 1 minute", Color::BLUE.alpha(0.3)),
                    ("<= 5 minutes", Color::ORANGE.alpha(0.5)),
                    ("> 5 minutes", Color::RED.alpha(0.8)),
                ],
            ),
            AgentColorScheme::DistanceCrossedSoFar => ColorLegend::new(
                "distance crossed to goal so far",
                vec![
                    ("<= 10%", rotating_color(0)),
                    ("<= 20%", rotating_color(1)),
                    ("<= 30%", rotating_color(2)),
                    ("<= 40%", rotating_color(3)),
                    ("<= 50%", rotating_color(4)),
                    ("<= 60%", rotating_color(5)),
                    ("<= 70%", rotating_color(6)),
                    ("<= 80%", rotating_color(7)),
                    ("<= 90%", rotating_color(8)),
                    ("> 90%", rotating_color(9)),
                ],
            ),
            AgentColorScheme::TripTimeSoFar => ColorLegend::new(
                "trip time so far",
                vec![
                    ("<= 1 minute", Color::BLUE.alpha(0.3)),
                    ("<= 5 minutes", Color::ORANGE.alpha(0.5)),
                    ("> 5 minutes", Color::RED.alpha(0.8)),
                ],
            ),
        }
    }

    pub fn all() -> Vec<(AgentColorScheme, String)> {
        vec![
            (
                AgentColorScheme::VehicleTypes,
                "by vehicle type".to_string(),
            ),
            (
                AgentColorScheme::Delay,
                "by time spent delayed/blocked".to_string(),
            ),
            (
                AgentColorScheme::DistanceCrossedSoFar,
                "by distance crossed to goal so far".to_string(),
            ),
            (
                AgentColorScheme::TripTimeSoFar,
                "by trip time so far".to_string(),
            ),
        ]
    }
}

fn delay_color(delay: Duration) -> Color {
    // TODO Better gradient
    if delay <= Duration::minutes(1) {
        return Color::BLUE.alpha(0.3);
    }
    if delay <= Duration::minutes(5) {
        return Color::ORANGE.alpha(0.5);
    }
    Color::RED.alpha(0.8)
}

fn percent_color(percent: f64) -> Color {
    rotating_color((percent * 10.0).round() as usize)
}
