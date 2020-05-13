use crate::world::{Object, ObjectID, World};
use abstutil::Timer;
use ezgui::{Color, Line, Prerender, Text};
use geom::{ArrowCap, Bounds, Circle, Distance, FindClosest, GPSBounds, PolyLine, Polygon, Pt2D};
use map_model::raw::{
    OriginalBuilding, OriginalIntersection, OriginalRoad, RawBuilding, RawIntersection, RawMap,
    RawRoad, RestrictionType, TurnRestriction,
};
use map_model::{
    osm, IntersectionType, LaneType, RoadSpec, NORMAL_LANE_THICKNESS, SIDEWALK_THICKNESS,
};
use std::collections::{BTreeMap, BTreeSet};
use std::mem;

const INTERSECTION_RADIUS: Distance = Distance::const_meters(5.0);
const BUILDING_LENGTH: Distance = Distance::const_meters(30.0);
const CENTER_LINE_THICKNESS: Distance = Distance::const_meters(0.5);

pub struct Model {
    // map and world are pub. The main crate should use them directly for simple stuff, to avoid
    // boilerplate delegation methods. Complex changes should be proper methods on the model.
    pub map: RawMap,
    showing_pts: Option<OriginalRoad>,
    pub world: World<ID>,

    include_bldgs: bool,
    pub intersection_geom: bool,
}

// Construction
impl Model {
    pub fn blank() -> Model {
        Model {
            map: RawMap::blank("", ""),
            showing_pts: None,

            include_bldgs: false,
            world: World::new(),
            intersection_geom: false,
        }
    }

    pub fn import(
        path: String,
        include_bldgs: bool,
        intersection_geom: bool,
        prerender: &Prerender,
    ) -> Model {
        let mut timer = Timer::new("import map");
        let mut model = Model::blank();
        model.include_bldgs = include_bldgs;
        if path.starts_with(&abstutil::path_all_raw_maps()) {
            model.map = abstutil::read_binary(path, &mut timer);
        } else {
            // Synthetic map!
            model.map = abstutil::read_json(path, &mut timer);
        }
        model.intersection_geom = intersection_geom;

        if model.include_bldgs {
            for id in model.map.buildings.keys().cloned().collect::<Vec<_>>() {
                model.bldg_added(id, prerender);
            }
        }
        timer.start_iter(
            "fill out world with intersections",
            model.map.intersections.len(),
        );
        for id in model.map.intersections.keys().cloned().collect::<Vec<_>>() {
            timer.next();
            model.intersection_added(id, prerender);
        }
        timer.start_iter("fill out world with roads", model.map.roads.len());
        for id in model.map.roads.keys().cloned().collect::<Vec<_>>() {
            timer.next();
            model.road_added(id, prerender);
        }

        model
    }
}

// General
impl Model {
    // TODO Only for truly synthetic maps...
    pub fn export(&mut self) {
        assert!(self.map.name != "");

        // Shift the map to start at (0, 0)
        let bounds = self.compute_bounds();
        if bounds.min_x != 0.0 || bounds.min_y != 0.0 {
            for b in self.map.buildings.values_mut() {
                b.polygon = Polygon::new(
                    &b.polygon
                        .points()
                        .iter()
                        .map(|pt| pt.offset(-bounds.min_x, -bounds.min_y))
                        .collect(),
                );
            }
            for i in self.map.intersections.values_mut() {
                i.point = i.point.offset(-bounds.min_x, -bounds.min_y);
            }
            for r in self.map.roads.values_mut() {
                for pt in &mut r.center_points {
                    *pt = pt.offset(-bounds.min_x, -bounds.min_y);
                }
            }
        }

        let bounds = self.compute_bounds();
        self.map.boundary_polygon = bounds.get_rectangle();
        // Make gps_bounds sane
        self.map.gps_bounds = GPSBounds::new();
        self.map.gps_bounds.update(
            Pt2D::new(bounds.min_x, bounds.min_y).forcibly_to_gps(&GPSBounds::seattle_bounds()),
        );
        self.map.gps_bounds.update(
            Pt2D::new(bounds.max_x, bounds.max_y).forcibly_to_gps(&GPSBounds::seattle_bounds()),
        );

        abstutil::write_json(abstutil::path_synthetic_map(&self.map.name), &self.map);
    }

    fn compute_bounds(&self) -> Bounds {
        let mut bounds = Bounds::new();
        for b in self.map.buildings.values() {
            for pt in b.polygon.points() {
                bounds.update(*pt);
            }
        }
        for i in self.map.intersections.values() {
            bounds.update(i.point);
        }
        for r in self.map.roads.values() {
            for pt in &r.center_points {
                bounds.update(*pt);
            }
        }
        bounds
    }

    pub fn delete_everything_inside(&mut self, area: Polygon) {
        if self.include_bldgs {
            for id in self.map.buildings.keys().cloned().collect::<Vec<_>>() {
                if area.contains_pt(self.map.buildings[&id].polygon.center()) {
                    self.delete_b(id);
                }
            }
        }

        for id in self.map.roads.keys().cloned().collect::<Vec<_>>() {
            if self.map.roads[&id]
                .center_points
                .iter()
                .any(|pt| area.contains_pt(*pt))
            {
                self.delete_r(id);
            }
        }

        for id in self.map.intersections.keys().cloned().collect::<Vec<_>>() {
            if area.contains_pt(self.map.intersections[&id].point) {
                self.delete_i(id);
            }
        }
    }

    pub fn describe_obj(&self, id: ID) -> Text {
        let mut txt = Text::new().with_bg();
        match id {
            ID::Building(b) => {
                txt.add_highlighted(Line(b.to_string()), Color::BLUE);
                for (k, v) in &self.map.buildings[&b].osm_tags {
                    txt.add_appended(vec![
                        Line(k).fg(Color::RED),
                        Line(" = "),
                        Line(v).fg(Color::CYAN),
                    ]);
                }
            }
            ID::Intersection(i) => {
                txt.add_highlighted(Line(i.to_string()), Color::BLUE);
                for r in self.map.roads_per_intersection(i) {
                    txt.add(Line(format!("- {}", r)));
                }
            }
            ID::Road(r) => {
                txt.add_highlighted(Line(r.to_string()), Color::BLUE);
                let road = &self.map.roads[&r];

                if let Some(name) = road.osm_tags.get(osm::NAME) {
                    txt.add(Line(name));
                } else if let Some(name) = road.osm_tags.get("ref") {
                    txt.add(Line(name));
                } else {
                    txt.add(Line("some road"));
                }

                for (k, v) in &road.osm_tags {
                    txt.add_appended(vec![
                        Line(k).fg(Color::RED),
                        Line(" = "),
                        Line(v).fg(Color::CYAN),
                    ]);
                }
                for (restriction, dst) in &road.turn_restrictions {
                    txt.add_appended(vec![
                        Line("Restriction: "),
                        Line(format!("{:?}", restriction)).fg(Color::RED),
                        Line(" to "),
                        Line(format!("way {}", dst)).fg(Color::CYAN),
                    ]);
                }

                // (MAX_CAR_LENGTH + sim::FOLLOWING_DISTANCE) from sim, but without the dependency
                txt.add(Line(format!(
                    "Can fit ~{} cars",
                    (PolyLine::new(road.center_points.clone()).length()
                        / (Distance::meters(6.5 + 1.0)))
                    .floor() as usize
                )));
            }
            ID::RoadPoint(r, idx) => {
                txt.add_highlighted(Line(format!("Point {}", idx)), Color::BLUE);
                txt.add(Line(format!("of {}", r)));
            }
            ID::TurnRestriction(TurnRestriction(from, restriction, to)) => {
                txt.add_highlighted(Line(format!("{:?}", restriction)), Color::BLUE);
                txt.add(Line(format!("from {}", from)));
                txt.add(Line(format!("to {}", to)));
            }
        }
        txt
    }
}

// Intersections
impl Model {
    fn intersection_added(&mut self, id: OriginalIntersection, prerender: &Prerender) {
        let i = &self.map.intersections[&id];
        let color = match i.intersection_type {
            IntersectionType::TrafficSignal => Color::GREEN,
            IntersectionType::StopSign => Color::RED,
            IntersectionType::Border => Color::BLUE,
            IntersectionType::Construction => Color::ORANGE,
        };

        let poly = if self.intersection_geom && !self.map.roads_per_intersection(id).is_empty() {
            let (poly, _, _) = self.map.preview_intersection(id, &mut Timer::throwaway());
            poly
        } else {
            Circle::new(i.point, INTERSECTION_RADIUS).to_polygon()
        };

        self.world
            .add(prerender, Object::new(ID::Intersection(id), color, poly));
    }

    pub fn create_i(&mut self, point: Pt2D, prerender: &Prerender) {
        let id = OriginalIntersection {
            osm_node_id: self.map.new_osm_node_id(time_to_id()),
        };
        self.map.intersections.insert(
            id,
            RawIntersection {
                point,
                intersection_type: IntersectionType::StopSign,
                // TODO If this isn't a synthetic map, load the elevation data and grab a real
                // value.
                elevation: Distance::ZERO,
            },
        );
        self.intersection_added(id, prerender);
    }

    pub fn move_i(&mut self, id: OriginalIntersection, point: Pt2D, prerender: &Prerender) {
        self.world.delete(ID::Intersection(id));
        for r in self.map.move_intersection(id, point).unwrap() {
            self.road_deleted(r);
            self.road_added(r, prerender);
        }
        self.intersection_added(id, prerender);
    }

    pub fn toggle_i_type(&mut self, id: OriginalIntersection, prerender: &Prerender) {
        self.world.delete(ID::Intersection(id));
        let it = match self.map.intersections[&id].intersection_type {
            IntersectionType::StopSign => IntersectionType::TrafficSignal,
            IntersectionType::TrafficSignal => {
                if self.map.roads_per_intersection(id).len() == 1 {
                    IntersectionType::Border
                } else {
                    IntersectionType::StopSign
                }
            }
            IntersectionType::Border => IntersectionType::StopSign,
            // These shouldn't exist in a basemap!
            IntersectionType::Construction => unreachable!(),
        };
        self.map
            .intersections
            .get_mut(&id)
            .unwrap()
            .intersection_type = it;
        self.intersection_added(id, prerender);
    }

    pub fn delete_i(&mut self, id: OriginalIntersection) {
        if !self.map.can_delete_intersection(id) {
            println!("Can't delete intersection used by roads");
            return;
        }
        self.map.delete_intersection(id);
        self.world.delete(ID::Intersection(id));
    }
}

// Roads
impl Model {
    fn road_added(&mut self, id: OriginalRoad, prerender: &Prerender) {
        for obj in self.road_objects(id) {
            self.world.add(prerender, obj);
        }
    }

    fn road_deleted(&mut self, id: OriginalRoad) {
        for obj in self.road_objects(id) {
            self.world.delete(obj.get_id());
        }
    }

    pub fn create_r(
        &mut self,
        i1: OriginalIntersection,
        i2: OriginalIntersection,
        prerender: &Prerender,
    ) {
        // Ban cul-de-sacs, since they get stripped out later anyway.
        if self
            .map
            .roads
            .keys()
            .any(|r| (r.i1 == i1 && r.i2 == i2) || (r.i1 == i2 && r.i2 == i1))
        {
            println!("Road already exists");
            return;
        }

        let id = OriginalRoad {
            osm_way_id: self.map.new_osm_way_id(time_to_id()),
            i1,
            i2,
        };
        let mut osm_tags = BTreeMap::new();
        osm_tags.insert(osm::SYNTHETIC.to_string(), "true".to_string());
        osm_tags.insert(
            osm::SYNTHETIC_LANES.to_string(),
            RoadSpec {
                fwd: vec![LaneType::Driving, LaneType::Parking, LaneType::Sidewalk],
                back: vec![LaneType::Driving, LaneType::Parking, LaneType::Sidewalk],
            }
            .to_string(),
        );
        osm_tags.insert(osm::ENDPT_FWD.to_string(), "true".to_string());
        osm_tags.insert(osm::ENDPT_BACK.to_string(), "true".to_string());
        osm_tags.insert(osm::OSM_WAY_ID.to_string(), id.osm_way_id.to_string());
        // Reasonable defaults.
        osm_tags.insert(osm::NAME.to_string(), "Streety McStreetFace".to_string());
        osm_tags.insert(osm::MAXSPEED.to_string(), "25 mph".to_string());

        self.map.roads.insert(
            id,
            RawRoad {
                center_points: vec![
                    self.map.intersections[&i1].point,
                    self.map.intersections[&i2].point,
                ],
                osm_tags,
                turn_restrictions: Vec::new(),
                complicated_turn_restrictions: Vec::new(),
            },
        );
        self.road_added(id, prerender);
    }

    pub fn edit_lanes(&mut self, id: OriginalRoad, spec: String, prerender: &Prerender) {
        self.road_deleted(id);

        if let Some(s) = RoadSpec::parse(spec.clone()) {
            self.map
                .roads
                .get_mut(&id)
                .unwrap()
                .osm_tags
                .insert(osm::SYNTHETIC_LANES.to_string(), s.to_string());
        } else {
            println!("Bad RoadSpec: {}", spec);
        }

        self.road_added(id, prerender);
    }

    pub fn swap_lanes(&mut self, id: OriginalRoad, prerender: &Prerender) {
        self.road_deleted(id);

        let (mut lanes, osm_tags) = {
            let r = self.map.roads.get_mut(&id).unwrap();
            (r.get_spec(), &mut r.osm_tags)
        };
        mem::swap(&mut lanes.fwd, &mut lanes.back);
        osm_tags.insert(osm::SYNTHETIC_LANES.to_string(), lanes.to_string());

        self.road_added(id, prerender);
    }

    pub fn set_r_name_and_speed(
        &mut self,
        id: OriginalRoad,
        name: String,
        speed: String,
        highway: String,
        prerender: &Prerender,
    ) {
        self.road_deleted(id);

        let osm_tags = &mut self.map.roads.get_mut(&id).unwrap().osm_tags;
        osm_tags.insert(osm::NAME.to_string(), name);
        osm_tags.insert(osm::MAXSPEED.to_string(), speed);
        osm_tags.insert(osm::HIGHWAY.to_string(), highway);

        self.road_added(id, prerender);
    }

    pub fn toggle_r_sidewalks(&mut self, some_id: OriginalRoad, prerender: &Prerender) {
        // Update every road belonging to the way.
        let osm_id = self.map.roads[&some_id].osm_tags[osm::OSM_WAY_ID].clone();
        let matching_roads = self
            .map
            .roads
            .iter()
            .filter_map(|(k, v)| {
                if v.osm_tags[osm::OSM_WAY_ID] == osm_id {
                    Some(*k)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Verify every road has the same sidewalk tags. Hints might've applied to just some parts.
        // If this is really true, then the way has to be split.
        let value = self.map.roads[&some_id]
            .osm_tags
            .get(osm::SIDEWALK)
            .cloned();
        for r in &matching_roads {
            if self.map.roads[r].osm_tags.get(osm::SIDEWALK) != value.as_ref() {
                println!(
                    "WARNING: {} and {} belong to same way, but have different sidewalk tags!",
                    some_id, r
                );
            }
        }

        for id in matching_roads {
            self.road_deleted(id);

            let osm_tags = &mut self.map.roads.get_mut(&id).unwrap().osm_tags;
            osm_tags.remove(osm::INFERRED_SIDEWALKS);
            if value == Some("both".to_string()) {
                osm_tags.insert(osm::SIDEWALK.to_string(), "right".to_string());
            } else if value == Some("right".to_string()) {
                osm_tags.insert(osm::SIDEWALK.to_string(), "left".to_string());
            } else if value == Some("left".to_string()) {
                osm_tags.insert(osm::SIDEWALK.to_string(), "none".to_string());
            } else if value == Some("none".to_string()) {
                osm_tags.insert(osm::SIDEWALK.to_string(), "both".to_string());
            }

            self.road_added(id, prerender);
        }
    }

    pub fn delete_r(&mut self, id: OriginalRoad) {
        self.stop_showing_pts(id);
        self.road_deleted(id);
        for tr in self.map.delete_road(id) {
            // We got these cases above in road_deleted
            if tr.0 != id {
                self.world.delete(ID::TurnRestriction(tr));
            }
        }
    }

    fn road_objects(&self, id: OriginalRoad) -> Vec<Object<ID>> {
        let r = &self.map.roads[&id];
        let unset =
            r.synthetic() && r.osm_tags.get(osm::NAME) == Some(&"Streety McStreetFace".to_string());
        let lanes_unknown = r.osm_tags.contains_key(osm::INFERRED_SIDEWALKS);
        let spec = r.get_spec();
        let center_pts = PolyLine::new(r.center_points.clone());

        let mut obj = Object::blank(ID::Road(id));

        let mut offset = Distance::ZERO;
        for (idx, lt) in spec.fwd.iter().enumerate() {
            let width = if *lt == LaneType::Sidewalk {
                SIDEWALK_THICKNESS
            } else {
                NORMAL_LANE_THICKNESS
            };
            obj.push(
                Model::lt_to_color(*lt, unset, lanes_unknown),
                self.map
                    .driving_side
                    .right_shift(center_pts.clone(), offset + width / 2.0)
                    .unwrap()
                    .make_polygons(width),
            );
            offset += width;
            if idx == 0 {
                obj.push(
                    Color::YELLOW,
                    center_pts.make_polygons(CENTER_LINE_THICKNESS),
                );
            }
        }
        offset = Distance::ZERO;
        for lt in &spec.back {
            let width = if *lt == LaneType::Sidewalk {
                SIDEWALK_THICKNESS
            } else {
                NORMAL_LANE_THICKNESS
            };
            obj.push(
                Model::lt_to_color(*lt, unset, lanes_unknown),
                self.map
                    .driving_side
                    .right_shift(center_pts.reversed(), offset + width / 2.0)
                    .unwrap()
                    .make_polygons(width),
            );
            offset += width;
        }

        let mut result = vec![obj];
        for (restriction, to) in &r.turn_restrictions {
            let polygon = if id == *to {
                // TODO Ideally a hollow circle with an arrow
                Circle::new(
                    PolyLine::new(self.map.roads[&id].center_points.clone()).middle(),
                    NORMAL_LANE_THICKNESS,
                )
                .to_polygon()
            } else {
                if !self.map.roads.contains_key(to) {
                    // TODO Fix. When roads are clipped, need to update IDS.
                    println!("Turn restriction to spot is missing!{}->{}", id, to);
                    continue;
                }
                PolyLine::new(vec![self.get_r_center(id), self.get_r_center(*to)])
                    .make_arrow(NORMAL_LANE_THICKNESS, ArrowCap::Triangle)
                    .unwrap()
            };

            result.push(Object::new(
                ID::TurnRestriction(TurnRestriction(id, *restriction, *to)),
                Color::PURPLE,
                polygon,
            ));
        }

        result
    }

    // Copied from render/lane.rs. :(
    fn lt_to_color(lt: LaneType, unset: bool, lanes_unknown: bool) -> Color {
        let color = match lt {
            LaneType::Driving => Color::BLACK,
            LaneType::Bus => Color::rgb(190, 74, 76),
            LaneType::Parking => Color::grey(0.2),
            LaneType::Sidewalk => Color::grey(0.8),
            LaneType::Biking => Color::rgb(15, 125, 75),
            LaneType::SharedLeftTurn => Color::YELLOW,
            LaneType::Construction => Color::rgb(255, 109, 0),
        };
        if unset {
            Color::rgba_f(0.9, color.g, color.b, 0.5)
        } else if lanes_unknown {
            Color::rgba_f(color.r, color.g, 0.9, 0.5)
        } else {
            color
        }
    }

    pub fn show_r_points(&mut self, id: OriginalRoad, prerender: &Prerender) {
        if self.showing_pts == Some(id) {
            return;
        }
        if let Some(other) = self.showing_pts {
            self.stop_showing_pts(other);
        }
        self.showing_pts = Some(id);

        let r = &self.map.roads[&id];
        for (idx, pt) in r.center_points.iter().enumerate() {
            // Don't show handles for the intersections
            if idx != 0 && idx != r.center_points.len() - 1 {
                self.world.add(
                    prerender,
                    Object::new(
                        ID::RoadPoint(id, idx),
                        Color::GREEN,
                        Circle::new(*pt, INTERSECTION_RADIUS / 2.0).to_polygon(),
                    ),
                );
            }
        }
    }

    pub fn stop_showing_pts(&mut self, id: OriginalRoad) {
        if self.showing_pts != Some(id) {
            return;
        }
        self.showing_pts = None;
        let r = &self.map.roads[&id];
        for idx in 1..=r.center_points.len() - 2 {
            self.world.delete(ID::RoadPoint(id, idx));
        }
    }

    pub fn move_r_pt(&mut self, id: OriginalRoad, idx: usize, point: Pt2D, prerender: &Prerender) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));

        let pts = &mut self.map.roads.get_mut(&id).unwrap().center_points;
        pts[idx] = point;

        self.road_added(id, prerender);
        self.intersection_added(id.i1, prerender);
        self.intersection_added(id.i2, prerender);
        self.show_r_points(id, prerender);
    }

    pub fn delete_r_pt(&mut self, id: OriginalRoad, idx: usize, prerender: &Prerender) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));

        let pts = &mut self.map.roads.get_mut(&id).unwrap().center_points;
        pts.remove(idx);

        self.road_added(id, prerender);
        self.intersection_added(id.i1, prerender);
        self.intersection_added(id.i2, prerender);
        self.show_r_points(id, prerender);
    }

    pub fn insert_r_pt(&mut self, id: OriginalRoad, pt: Pt2D, prerender: &Prerender) -> Option<ID> {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));

        let mut closest = FindClosest::new(&self.compute_bounds());
        let pts = &mut self.map.roads.get_mut(&id).unwrap().center_points;
        for (idx, pair) in pts.windows(2).enumerate() {
            closest.add(idx + 1, &vec![pair[0], pair[1]]);
        }
        let new_id = if let Some((idx, _)) = closest.closest_pt(pt, Distance::meters(5.0)) {
            pts.insert(idx, pt);
            Some(ID::RoadPoint(id, idx))
        } else {
            println!("Couldn't figure out where to insert new point");
            None
        };

        self.road_added(id, prerender);
        self.intersection_added(id.i1, prerender);
        self.intersection_added(id.i2, prerender);
        self.show_r_points(id, prerender);

        new_id
    }

    pub fn clear_r_pts(&mut self, id: OriginalRoad, prerender: &Prerender) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));

        let r = &mut self.map.roads.get_mut(&id).unwrap();
        r.center_points = vec![r.center_points[0], *r.center_points.last().unwrap()];

        self.road_added(id, prerender);
        self.intersection_added(id.i1, prerender);
        self.intersection_added(id.i2, prerender);
        self.show_r_points(id, prerender);
    }

    pub fn get_r_center(&self, id: OriginalRoad) -> Pt2D {
        PolyLine::new(self.map.roads[&id].center_points.clone()).middle()
    }
}

// Turn restrictions
impl Model {
    pub fn add_tr(
        &mut self,
        from: OriginalRoad,
        restriction: RestrictionType,
        to: OriginalRoad,
        prerender: &Prerender,
    ) {
        self.road_deleted(from);

        assert!(self.map.can_add_turn_restriction(from, to));
        // TODO Worry about dupes
        self.map
            .roads
            .get_mut(&from)
            .unwrap()
            .turn_restrictions
            .push((restriction, to));

        self.road_added(from, prerender);
    }

    pub fn delete_tr(&mut self, tr: TurnRestriction) {
        self.map.delete_turn_restriction(tr);
        self.world.delete(ID::TurnRestriction(tr));
    }
}

// Buildings
impl Model {
    fn bldg_added(&mut self, id: OriginalBuilding, prerender: &Prerender) {
        let b = &self.map.buildings[&id];
        self.world.add(
            prerender,
            Object::new(ID::Building(id), Color::BLUE, b.polygon.clone()),
        );
    }

    pub fn create_b(&mut self, center: Pt2D, prerender: &Prerender) -> ID {
        let id = OriginalBuilding {
            osm_way_id: self.map.new_osm_way_id(time_to_id()),
        };
        self.map.buildings.insert(
            id,
            RawBuilding {
                polygon: Polygon::rectangle_centered(center, BUILDING_LENGTH, BUILDING_LENGTH),
                osm_tags: BTreeMap::new(),
                public_garage_name: None,
                num_parking_spots: 0,
                amenities: BTreeSet::new(),
            },
        );
        self.bldg_added(id, prerender);
        ID::Building(id)
    }

    pub fn move_b(&mut self, id: OriginalBuilding, new_center: Pt2D, prerender: &Prerender) {
        self.world.delete(ID::Building(id));

        let b = self.map.buildings.get_mut(&id).unwrap();
        let old_center = b.polygon.center();
        b.polygon = b.polygon.translate(
            new_center.x() - old_center.x(),
            new_center.y() - old_center.y(),
        );

        self.bldg_added(id, prerender);
    }

    pub fn delete_b(&mut self, id: OriginalBuilding) {
        self.world.delete(ID::Building(id));
        self.map.buildings.remove(&id).unwrap();
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ID {
    Building(OriginalBuilding),
    Intersection(OriginalIntersection),
    Road(OriginalRoad),
    RoadPoint(OriginalRoad, usize),
    TurnRestriction(TurnRestriction),
}

impl ObjectID for ID {
    fn zorder(&self) -> usize {
        match self {
            ID::Road(_) => 0,
            ID::Intersection(_) => 1,
            ID::Building(_) => 2,
            ID::RoadPoint(_, _) => 3,
            ID::TurnRestriction(_) => 4,
        }
    }
}

// Don't conflict with the synthetic IDs generated by map clipping.
#[cfg(not(target_arch = "wasm32"))]
fn time_to_id() -> i64 {
    -(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64)
}

#[cfg(target_arch = "wasm32")]
fn time_to_id() -> i64 {
    // TODO This is correct, just probably kind of annoying and slow. Having trouble getting
    // current time as seconds in wasm.
    -5000
}
