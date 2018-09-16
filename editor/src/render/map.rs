// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::{Point, Rect};
use aabb_quadtree::QuadTree;
use control::ControlMap;
use geom::{LonLat, Pt2D};
use kml::{ExtraShape, ExtraShapeID};
use map_model::{BuildingID, BusStopID, IntersectionID, Lane, LaneID, Map, ParcelID, Turn, TurnID};
use objects::ID;
use plugins::hider::Hider;
use render::building::DrawBuilding;
use render::bus_stop::DrawBusStop;
use render::car::DrawCar;
use render::extra_shape::DrawExtraShape;
use render::intersection::DrawIntersection;
use render::lane::DrawLane;
use render::parcel::DrawParcel;
use render::pedestrian::DrawPedestrian;
use render::turn::DrawTurn;
use render::Renderable;
use sim::Sim;
use std::collections::HashMap;
use ui::{ShowTurnIcons, ToggleableLayers};

pub struct DrawMap {
    pub lanes: Vec<DrawLane>,
    pub intersections: Vec<DrawIntersection>,
    pub turns: HashMap<TurnID, DrawTurn>,
    pub buildings: Vec<DrawBuilding>,
    pub parcels: Vec<DrawParcel>,
    pub extra_shapes: Vec<DrawExtraShape>,
    pub bus_stops: HashMap<BusStopID, DrawBusStop>,

    quadtree: QuadTree<ID>,
}

impl DrawMap {
    // Also returns the center of the map in map-space
    pub fn new(
        map: &Map,
        control_map: &ControlMap,
        raw_extra_shapes: Vec<ExtraShape>,
    ) -> (DrawMap, Pt2D) {
        let mut lanes: Vec<DrawLane> = Vec::new();
        for l in map.all_lanes() {
            lanes.push(DrawLane::new(l, map, control_map));
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
            .map(|i| DrawIntersection::new(i, map, &lanes))
            .collect();
        let buildings: Vec<DrawBuilding> = map
            .all_buildings()
            .iter()
            .map(|b| DrawBuilding::new(b))
            .collect();
        let parcels: Vec<DrawParcel> = map
            .all_parcels()
            .iter()
            .map(|p| DrawParcel::new(p))
            .collect();
        let extra_shapes: Vec<DrawExtraShape> = raw_extra_shapes
            .into_iter()
            .map(|s| DrawExtraShape::new(s))
            .collect();
        let mut bus_stops: HashMap<BusStopID, DrawBusStop> = HashMap::new();
        for s in map.all_bus_stops().values() {
            bus_stops.insert(s.id, DrawBusStop::new(s, map));
        }

        // min_y here due to the wacky y inversion
        let bounds = map.get_gps_bounds();
        let max_screen_pt = Pt2D::from_gps(&LonLat::new(bounds.max_x, bounds.min_y), &bounds);
        let map_bbox = Rect {
            top_left: Point { x: 0.0, y: 0.0 },
            bottom_right: Point {
                x: max_screen_pt.x() as f32,
                y: max_screen_pt.y() as f32,
            },
        };

        let mut quadtree = QuadTree::default(map_bbox);
        // TODO use iter chain if everything was boxed as a renderable...
        for obj in &lanes {
            quadtree.insert_with_box(obj.get_id(), obj.get_bbox());
        }
        for obj in &intersections {
            quadtree.insert_with_box(obj.get_id(), obj.get_bbox());
        }
        for obj in &buildings {
            quadtree.insert_with_box(obj.get_id(), obj.get_bbox());
        }
        for obj in &parcels {
            quadtree.insert_with_box(obj.get_id(), obj.get_bbox());
        }
        for obj in &extra_shapes {
            quadtree.insert_with_box(obj.get_id(), obj.get_bbox());
        }
        for obj in bus_stops.values() {
            quadtree.insert_with_box(obj.get_id(), obj.get_bbox());
        }

        (
            DrawMap {
                lanes,
                intersections,
                turns,
                buildings,
                parcels,
                extra_shapes,
                bus_stops,

                quadtree,
            },
            Pt2D::new(max_screen_pt.x() / 2.0, max_screen_pt.y() / 2.0),
        )
    }

    fn compute_turn_to_lane_offset(result: &mut HashMap<TurnID, usize>, l: &Lane, map: &Map) {
        // Split into two groups, based on the endpoint
        let mut pair: (Vec<&Turn>, Vec<&Turn>) = map
            .get_turns_from_lane(l.id)
            .iter()
            .partition(|t| t.parent == l.dst_i);

        // Sort the turn icons by angle.
        pair.0
            .sort_by_key(|t| t.line.angle().normalized_degrees() as i64);
        pair.1
            .sort_by_key(|t| t.line.angle().normalized_degrees() as i64);

        for (idx, t) in pair.0.iter().enumerate() {
            result.insert(t.id, idx);
        }
        for (idx, t) in pair.1.iter().enumerate() {
            result.insert(t.id, idx);
        }
    }

    pub fn edit_lane_type(&mut self, id: LaneID, map: &Map, control_map: &ControlMap) {
        // No need to edit the quadtree; the bbox shouldn't depend on lane type.
        self.lanes[id.0] = DrawLane::new(map.get_l(id), map, control_map);
    }

    pub fn edit_remove_turn(&mut self, id: TurnID) {
        self.turns.remove(&id);
    }

    pub fn edit_add_turn(&mut self, id: TurnID, map: &Map) {
        let t = map.get_t(id);
        let mut turn_to_lane_offset: HashMap<TurnID, usize> = HashMap::new();
        DrawMap::compute_turn_to_lane_offset(&mut turn_to_lane_offset, map.get_l(t.src), map);
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
        screen_bbox: Rect,
        hider: &Hider,
        map: &Map,
        sim: &Sim,
        layers: &ToggleableLayers,
        show_turn_icons: &T,
    ) -> (Vec<Box<&Renderable>>, Vec<Box<Renderable>>) {
        // From background to foreground Z-order
        let mut parcels: Vec<Box<&Renderable>> = Vec::new();
        let mut lanes: Vec<Box<&Renderable>> = Vec::new();
        let mut intersections: Vec<Box<&Renderable>> = Vec::new();
        let mut buildings: Vec<Box<&Renderable>> = Vec::new();
        let mut extra_shapes: Vec<Box<&Renderable>> = Vec::new();
        let mut bus_stops: Vec<Box<&Renderable>> = Vec::new();
        let mut turn_icons: Vec<Box<&Renderable>> = Vec::new();

        let mut cars: Vec<Box<Renderable>> = Vec::new();
        let mut peds: Vec<Box<Renderable>> = Vec::new();

        for &(id, _, _) in &self.quadtree.query(screen_bbox) {
            if hider.show(*id) && layers.show(*id) {
                match id {
                    ID::Parcel(id) => parcels.push(Box::new(self.get_p(*id))),
                    ID::Lane(id) => {
                        lanes.push(Box::new(self.get_l(*id)));
                        for c in sim.get_draw_cars_on_lane(*id, map).into_iter() {
                            cars.push(Box::new(DrawCar::new(c, map)));
                        }
                        for p in sim.get_draw_peds_on_lane(*id, map).into_iter() {
                            peds.push(Box::new(DrawPedestrian::new(p, map)));
                        }
                    }
                    ID::Intersection(id) => {
                        intersections.push(Box::new(self.get_i(*id)));
                        for t in &map.get_i(*id).turns {
                            if show_turn_icons.show_icons_for(*id) {
                                turn_icons.push(Box::new(self.get_t(*t)));
                            }
                            for c in sim.get_draw_cars_on_turn(*t, map).into_iter() {
                                cars.push(Box::new(DrawCar::new(c, map)));
                            }
                            for p in sim.get_draw_peds_on_turn(*t, map).into_iter() {
                                peds.push(Box::new(DrawPedestrian::new(p, map)));
                            }
                        }
                    }
                    // TODO front paths will get drawn over buildings, depending on quadtree order.
                    // probably just need to make them go around other buildings instead of having
                    // two passes through buildings.
                    ID::Building(id) => buildings.push(Box::new(self.get_b(*id))),
                    ID::ExtraShape(id) => extra_shapes.push(Box::new(self.get_es(*id))),
                    ID::BusStop(id) => bus_stops.push(Box::new(self.get_bs(*id))),

                    ID::Turn(_) | ID::Car(_) | ID::Pedestrian(_) => {
                        panic!("{:?} shouldn't be in the quadtree", id)
                    }
                }
            }
        }

        let mut borrows: Vec<Box<&Renderable>> = Vec::new();
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
