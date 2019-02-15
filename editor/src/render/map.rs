use crate::colors::ColorScheme;
use crate::objects::ID;
use crate::render::area::DrawArea;
use crate::render::building::DrawBuilding;
use crate::render::bus_stop::DrawBusStop;
use crate::render::extra_shape::{DrawExtraShape, ExtraShapeID};
use crate::render::intersection::DrawIntersection;
use crate::render::lane::DrawLane;
use crate::render::parcel::DrawParcel;
use crate::render::road::DrawRoad;
use crate::render::turn::DrawTurn;
use crate::render::Renderable;
use crate::state::Flags;
use aabb_quadtree::QuadTree;
use abstutil::Timer;
use ezgui::{Color, Drawable, Prerender};
use geom::{Bounds, FindClosest, Polygon};
use map_model::{
    AreaID, BuildingID, BusStopID, IntersectionID, Lane, LaneID, Map, ParcelID, RoadID,
    Traversable, Turn, TurnID, LANE_THICKNESS,
};
use sim::Tick;
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;

pub struct DrawMap {
    pub roads: Vec<DrawRoad>,
    pub lanes: Vec<DrawLane>,
    pub intersections: Vec<DrawIntersection>,
    pub turns: HashMap<TurnID, DrawTurn>,
    pub buildings: Vec<DrawBuilding>,
    pub parcels: Vec<DrawParcel>,
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
    pub draw_all_parcels: Drawable,

    quadtree: QuadTree<ID>,
}

impl DrawMap {
    pub fn new(
        map: &Map,
        flags: &Flags,
        cs: &ColorScheme,
        prerender: &Prerender,
        timer: &mut Timer,
    ) -> DrawMap {
        timer.start_iter("make DrawRoads", map.all_roads().len());
        let mut roads: Vec<DrawRoad> = Vec::new();
        let mut all_roads: Vec<Polygon> = Vec::new();
        for r in map.all_roads() {
            timer.next();
            let (draw, poly) = DrawRoad::new(r, cs, prerender);
            roads.push(draw);
            all_roads.push(poly);
        }
        let draw_all_thick_roads = prerender.upload(
            all_roads
                .into_iter()
                .map(|p| (cs.get_def("unzoomed road band", Color::BLACK), p))
                .collect(),
        );

        timer.start_iter("make DrawLanes", map.all_lanes().len());
        let mut lanes: Vec<DrawLane> = Vec::new();
        for l in map.all_lanes() {
            timer.next();
            lanes.push(DrawLane::new(
                l,
                map,
                !flags.dont_draw_lane_markings,
                cs,
                prerender,
            ));
        }

        let mut turn_to_lane_offset: HashMap<TurnID, usize> = HashMap::new();
        for l in map.all_lanes() {
            DrawMap::compute_turn_to_lane_offset(&mut turn_to_lane_offset, l, map);
        }
        assert_eq!(turn_to_lane_offset.len(), map.all_turns().len());

        timer.start_iter("make DrawTurns", map.all_turns().len());
        let mut turns: HashMap<TurnID, DrawTurn> = HashMap::new();
        for t in map.all_turns().values() {
            timer.next();
            turns.insert(t.id, DrawTurn::new(map, t, turn_to_lane_offset[&t.id]));
        }

        timer.start_iter("make DrawIntersections", map.all_intersections().len());
        let intersections: Vec<DrawIntersection> = map
            .all_intersections()
            .iter()
            .map(|i| {
                timer.next();
                DrawIntersection::new(i, map, cs, prerender)
            })
            .collect();
        let draw_all_unzoomed_intersections = prerender.upload_borrowed(
            intersections
                .iter()
                .map(|i| {
                    (
                        cs.get_def("unzoomed intersection", Color::BLACK),
                        &i.polygon,
                    )
                })
                .collect(),
        );

        let mut buildings: Vec<DrawBuilding> = Vec::new();
        let mut all_buildings: Vec<(Color, Polygon)> = Vec::new();
        if !flags.dont_draw_buildings {
            timer.start_iter("make DrawBuildings", map.all_buildings().len());
            for b in map.all_buildings() {
                timer.next();
                let (b, draw) = DrawBuilding::new(b, cs);
                buildings.push(b);
                all_buildings.extend(draw);
            }
        }
        let draw_all_buildings = prerender.upload(all_buildings);

        let mut parcels: Vec<DrawParcel> = Vec::new();
        let mut all_parcels: Vec<(Color, Polygon)> = Vec::new();
        if flags.draw_parcels {
            timer.start_iter("make DrawParcels", map.all_parcels().len());
            for p in map.all_parcels() {
                timer.next();
                let (draw, geom) = DrawParcel::new(p, cs);
                parcels.push(draw);
                all_parcels.extend(geom);
            }
        }
        let draw_all_parcels = prerender.upload(all_parcels);

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

            // Match shapes with the nearest road + direction (true for forwards)
            let mut closest: FindClosest<(RoadID, bool)> = FindClosest::new(&map.get_bounds());
            for r in map.all_roads().iter() {
                closest.add((r.id, true), &r.center_pts.shift_right(LANE_THICKNESS));
                closest.add((r.id, false), &r.center_pts.shift_left(LANE_THICKNESS));
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

        let mut bus_stops: HashMap<BusStopID, DrawBusStop> = HashMap::new();
        for s in map.all_bus_stops().values() {
            bus_stops.insert(s.id, DrawBusStop::new(s, map, cs, prerender));
        }

        let mut areas: Vec<DrawArea> = Vec::new();
        let mut all_areas: Vec<(Color, Polygon)> = Vec::new();
        if !flags.dont_draw_areas {
            timer.start_iter("make DrawAreas", map.all_areas().len());
            for a in map.all_areas() {
                timer.next();
                let (draw, color, poly) = DrawArea::new(a, cs);
                areas.push(draw);
                all_areas.push((color, poly));
            }
        }
        let draw_all_areas = prerender.upload(all_areas);

        let boundary_polygon = prerender.upload_borrowed(vec![(
            cs.get_def("map background", Color::rgb(242, 239, 233)),
            map.get_boundary_polygon(),
        )]);

        timer.start("create quadtree");
        let mut quadtree = QuadTree::default(map.get_bounds().as_bbox());
        // TODO use iter chain if everything was boxed as a renderable...
        for obj in &roads {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds(map).as_bbox());
        }
        for obj in &lanes {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds(map).as_bbox());
        }
        for obj in &intersections {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds(map).as_bbox());
        }
        for obj in &buildings {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds(map).as_bbox());
        }
        for obj in &parcels {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds(map).as_bbox());
        }
        for obj in &extra_shapes {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds(map).as_bbox());
        }
        // Don't put BusStops in the quadtree
        for obj in &areas {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds(map).as_bbox());
        }
        timer.stop("create quadtree");

        info!(
            "static DrawMap consumes {} MB on the GPU",
            abstutil::prettyprint_usize(prerender.get_total_bytes_uploaded() / 1024 / 1024)
        );

        DrawMap {
            roads,
            lanes,
            intersections,
            turns,
            buildings,
            parcels,
            extra_shapes,
            bus_stops,
            areas,
            boundary_polygon,
            draw_all_thick_roads,
            draw_all_unzoomed_intersections,
            draw_all_buildings,
            draw_all_areas,
            draw_all_parcels,

            agents: RefCell::new(AgentCache {
                tick: None,
                agents_per_on: HashMap::new(),
            }),

            quadtree,
        }
    }

    fn compute_turn_to_lane_offset(result: &mut HashMap<TurnID, usize>, l: &Lane, map: &Map) {
        // Split into two groups, based on the endpoint
        let mut pair: (Vec<&Turn>, Vec<&Turn>) = map
            .get_turns_from_lane(l.id)
            .iter()
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

    pub fn edit_lane_type(
        &mut self,
        id: LaneID,
        map: &Map,
        cs: &ColorScheme,
        prerender: &Prerender,
    ) {
        // No need to edit the quadtree; the bbox shouldn't depend on lane type.
        // TODO Preserve flags.dont_draw_lane_markings
        self.lanes[id.0] = DrawLane::new(map.get_l(id), map, true, cs, prerender);
    }

    pub fn edit_remove_turn(&mut self, id: TurnID) {
        self.turns.remove(&id);
    }

    pub fn edit_add_turn(&mut self, id: TurnID, map: &Map) {
        let t = map.get_t(id);
        let mut turn_to_lane_offset: HashMap<TurnID, usize> = HashMap::new();
        DrawMap::compute_turn_to_lane_offset(&mut turn_to_lane_offset, map.get_l(id.src), map);
        let draw_turn = DrawTurn::new(map, t, turn_to_lane_offset[&id]);
        self.turns.insert(id, draw_turn);
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

    pub fn get_b(&self, id: BuildingID) -> &DrawBuilding {
        &self.buildings[id.0]
    }

    pub fn get_p(&self, id: ParcelID) -> &DrawParcel {
        &self.parcels[id.0]
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

    #[allow(dead_code)]
    pub fn get_matching_lanes(&self, bounds: Bounds) -> Vec<LaneID> {
        let mut results: Vec<LaneID> = Vec::new();
        for &(id, _, _) in &self.quadtree.query(bounds.as_bbox()) {
            if let ID::Lane(id) = id {
                results.push(*id);
            }
        }
        results
    }

    // Unsorted, unexpanded, raw result.
    pub fn get_matching_objects(&self, bounds: Bounds) -> Vec<ID> {
        let mut results: Vec<ID> = Vec::new();
        for &(id, _, _) in &self.quadtree.query(bounds.as_bbox()) {
            results.push(*id);
        }
        results
    }
}

// TODO Invalidate when we interactively spawn stuff elsewhere?
pub struct AgentCache {
    tick: Option<Tick>,
    agents_per_on: HashMap<Traversable, Vec<Box<Renderable>>>,
}

impl AgentCache {
    pub fn has(&self, tick: Tick, on: Traversable) -> bool {
        if Some(tick) != self.tick {
            return false;
        }
        self.agents_per_on.contains_key(&on)
    }

    // Must call has() first.
    pub fn get(&self, on: Traversable) -> Vec<Box<&Renderable>> {
        self.agents_per_on[&on]
            .iter()
            .map(|obj| Box::new(obj.borrow()))
            .collect()
    }

    pub fn put(&mut self, tick: Tick, on: Traversable, agents: Vec<Box<Renderable>>) {
        if Some(tick) != self.tick {
            self.agents_per_on.clear();
            self.tick = Some(tick);
        }

        assert!(!self.agents_per_on.contains_key(&on));
        self.agents_per_on.insert(on, agents);
    }
}
