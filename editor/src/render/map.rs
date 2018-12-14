// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use crate::objects::ID;
use crate::plugins::debug::DebugMode;
use crate::render::area::DrawArea;
use crate::render::building::DrawBuilding;
use crate::render::bus_stop::DrawBusStop;
use crate::render::extra_shape::{DrawExtraShape, ExtraShapeID};
use crate::render::intersection::DrawIntersection;
use crate::render::lane::DrawLane;
use crate::render::parcel::DrawParcel;
use crate::render::pedestrian::DrawPedestrian;
use crate::render::turn::DrawTurn;
use crate::render::{draw_vehicle, Renderable};
use crate::state::ShowTurnIcons;
use aabb_quadtree::QuadTree;
use abstutil::Timer;
use geom::Bounds;
use kml::ExtraShape;
use map_model::{
    AreaID, BuildingID, BusStopID, FindClosest, IntersectionID, Lane, LaneID, Map, ParcelID,
    RoadID, Traversable, Turn, TurnID, LANE_THICKNESS,
};
use sim::GetDrawAgents;
use std::collections::HashMap;

pub struct DrawMap {
    pub lanes: Vec<DrawLane>,
    pub intersections: Vec<DrawIntersection>,
    pub turns: HashMap<TurnID, DrawTurn>,
    pub buildings: Vec<DrawBuilding>,
    pub parcels: Vec<DrawParcel>,
    pub extra_shapes: Vec<DrawExtraShape>,
    pub bus_stops: HashMap<BusStopID, DrawBusStop>,
    pub areas: Vec<DrawArea>,

    quadtree: QuadTree<ID>,
}

impl DrawMap {
    pub fn new(map: &Map, raw_extra_shapes: Vec<ExtraShape>, timer: &mut Timer) -> DrawMap {
        let mut lanes: Vec<DrawLane> = Vec::new();
        timer.start_iter("make DrawLanes", map.all_lanes().len());
        for l in map.all_lanes() {
            timer.next();
            lanes.push(DrawLane::new(l, map));
        }

        let mut turn_to_lane_offset: HashMap<TurnID, usize> = HashMap::new();
        for l in map.all_lanes() {
            DrawMap::compute_turn_to_lane_offset(&mut turn_to_lane_offset, l, map);
        }
        assert_eq!(turn_to_lane_offset.len(), map.all_turns().len());

        let mut turns: HashMap<TurnID, DrawTurn> = HashMap::new();
        for t in map.all_turns().values() {
            turns.insert(t.id, DrawTurn::new(map, t, turn_to_lane_offset[&t.id]));
        }
        let intersections: Vec<DrawIntersection> = map
            .all_intersections()
            .iter()
            .map(|i| DrawIntersection::new(i, map))
            .collect();
        timer.start_iter("make DrawBuildings", map.all_buildings().len());
        let buildings: Vec<DrawBuilding> = map
            .all_buildings()
            .iter()
            .map(|b| {
                timer.next();
                DrawBuilding::new(b)
            })
            .collect();
        let parcels: Vec<DrawParcel> = map
            .all_parcels()
            .iter()
            .map(|p| DrawParcel::new(p))
            .collect();

        let mut extra_shapes: Vec<DrawExtraShape> = Vec::new();
        if !raw_extra_shapes.is_empty() {
            // Match shapes with the nearest road + direction (true for forwards)
            let mut closest: FindClosest<(RoadID, bool)> =
                map_model::FindClosest::new(&map.get_bounds());
            for r in map.all_roads().iter() {
                closest.add((r.id, true), &r.center_pts.shift_blindly(LANE_THICKNESS));
                closest.add(
                    (r.id, false),
                    &r.center_pts.reversed().shift_blindly(LANE_THICKNESS),
                );
            }

            let gps_bounds = map.get_gps_bounds();
            for s in raw_extra_shapes.into_iter() {
                if let Some(es) =
                    DrawExtraShape::new(ExtraShapeID(extra_shapes.len()), s, gps_bounds, &closest)
                {
                    extra_shapes.push(es);
                }
            }
        }

        let mut bus_stops: HashMap<BusStopID, DrawBusStop> = HashMap::new();
        for s in map.all_bus_stops().values() {
            bus_stops.insert(s.id, DrawBusStop::new(s, map));
        }
        let areas: Vec<DrawArea> = map.all_areas().iter().map(|a| DrawArea::new(a)).collect();

        timer.start("create quadtree");
        let mut quadtree = QuadTree::default(map.get_bounds().as_bbox());
        // TODO use iter chain if everything was boxed as a renderable...
        for obj in &lanes {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds().as_bbox());
        }
        for obj in &intersections {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds().as_bbox());
        }
        for obj in &buildings {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds().as_bbox());
        }
        for obj in &parcels {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds().as_bbox());
        }
        for obj in &extra_shapes {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds().as_bbox());
        }
        for obj in bus_stops.values() {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds().as_bbox());
        }
        for obj in &areas {
            quadtree.insert_with_box(obj.get_id(), obj.get_bounds().as_bbox());
        }
        timer.stop("create quadtree");

        DrawMap {
            lanes,
            intersections,
            turns,
            buildings,
            parcels,
            extra_shapes,
            bus_stops,
            areas,

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

    pub fn edit_lane_type(&mut self, id: LaneID, map: &Map) {
        // No need to edit the quadtree; the bbox shouldn't depend on lane type.
        self.lanes[id.0] = DrawLane::new(map.get_l(id), map);
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

    // A greatly simplified form of get_objects_onscreen
    pub fn get_matching_lanes(&self, bounds: Bounds) -> Vec<LaneID> {
        let mut results: Vec<LaneID> = Vec::new();
        for &(id, _, _) in &self.quadtree.query(bounds.as_bbox()) {
            if let ID::Lane(id) = id {
                results.push(*id);
            }
        }
        results
    }

    // Returns in back-to-front order
    // The second pair is ephemeral objects (cars, pedestrians) that we can't borrow --
    // conveniently they're the front-most layer, so the caller doesn't have to do anything strange
    // to merge.
    // TODO alternatively, we could return IDs in order, then the caller could turn around and call
    // a getter... except they still have to deal with DrawCar and DrawPedestrian not being
    // borrowable. Could move contains_pt and draw calls here directly, but that might be weird?
    // But maybe not.
    pub fn get_objects_onscreen<T: ShowTurnIcons>(
        &self,
        screen_bounds: Bounds,
        debug_mode: &DebugMode,
        map: &Map,
        sim: &GetDrawAgents,
        show_turn_icons: &T,
    ) -> (Vec<Box<&Renderable>>, Vec<Box<Renderable>>) {
        // From background to foreground Z-order
        let mut areas: Vec<Box<&Renderable>> = Vec::new();
        let mut parcels: Vec<Box<&Renderable>> = Vec::new();
        let mut lanes: Vec<Box<&Renderable>> = Vec::new();
        let mut intersections: Vec<Box<&Renderable>> = Vec::new();
        let mut buildings: Vec<Box<&Renderable>> = Vec::new();
        let mut extra_shapes: Vec<Box<&Renderable>> = Vec::new();
        let mut bus_stops: Vec<Box<&Renderable>> = Vec::new();
        let mut turn_icons: Vec<Box<&Renderable>> = Vec::new();

        let mut cars: Vec<Box<Renderable>> = Vec::new();
        let mut peds: Vec<Box<Renderable>> = Vec::new();

        for &(id, _, _) in &self.quadtree.query(screen_bounds.as_bbox()) {
            if debug_mode.show(*id) {
                match id {
                    ID::Area(id) => areas.push(Box::new(self.get_a(*id))),
                    ID::Parcel(id) => parcels.push(Box::new(self.get_p(*id))),
                    ID::Lane(id) => {
                        lanes.push(Box::new(self.get_l(*id)));
                        if !show_turn_icons.show_icons_for(map.get_l(*id).dst_i) {
                            for c in sim.get_draw_cars(Traversable::Lane(*id), map).into_iter() {
                                cars.push(draw_vehicle(c, map));
                            }
                            for p in sim.get_draw_peds(Traversable::Lane(*id), map).into_iter() {
                                peds.push(Box::new(DrawPedestrian::new(p, map)));
                            }
                        }
                    }
                    ID::Intersection(id) => {
                        intersections.push(Box::new(self.get_i(*id)));
                        for t in &map.get_i(*id).turns {
                            if show_turn_icons.show_icons_for(*id) {
                                turn_icons.push(Box::new(self.get_t(*t)));
                            } else {
                                for c in sim.get_draw_cars(Traversable::Turn(*t), map).into_iter() {
                                    cars.push(draw_vehicle(c, map));
                                }
                                for p in sim.get_draw_peds(Traversable::Turn(*t), map).into_iter() {
                                    peds.push(Box::new(DrawPedestrian::new(p, map)));
                                }
                            }
                        }
                    }
                    // TODO front paths will get drawn over buildings, depending on quadtree order.
                    // probably just need to make them go around other buildings instead of having
                    // two passes through buildings.
                    ID::Building(id) => buildings.push(Box::new(self.get_b(*id))),
                    ID::ExtraShape(id) => extra_shapes.push(Box::new(self.get_es(*id))),
                    ID::BusStop(id) => bus_stops.push(Box::new(self.get_bs(*id))),

                    ID::Turn(_) | ID::Car(_) | ID::Pedestrian(_) | ID::Trip(_) => {
                        panic!("{:?} shouldn't be in the quadtree", id)
                    }
                }
            }
        }

        let mut borrows: Vec<Box<&Renderable>> = Vec::new();
        borrows.extend(areas);
        borrows.extend(parcels);
        borrows.extend(lanes);
        borrows.extend(intersections);
        borrows.extend(buildings);
        borrows.extend(extra_shapes);
        borrows.extend(bus_stops);
        borrows.extend(turn_icons);

        let mut returns: Vec<Box<Renderable>> = Vec::new();
        returns.extend(cars);
        returns.extend(peds);

        (borrows, returns)
    }
}
