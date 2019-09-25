use abstutil::{read_binary, Timer};
use ezgui::world::{Object, ObjectID, World};
use ezgui::{Color, EventCtx, GfxCtx, Line, Prerender, Text};
use geom::{Bounds, Circle, Distance, PolyLine, Polygon, Pt2D};
use map_model::raw::{
    MapFixes, OriginalIntersection, OriginalRoad, RawBuilding, RawIntersection, RawMap, RawRoad,
    StableBuildingID, StableIntersectionID, StableRoadID,
};
use map_model::{osm, IntersectionType, LaneType, RoadSpec, LANE_THICKNESS};
use std::collections::{BTreeMap, BTreeSet};
use std::mem;

const INTERSECTION_RADIUS: Distance = Distance::const_meters(5.0);
const BUILDING_LENGTH: Distance = Distance::const_meters(30.0);
const CENTER_LINE_THICKNESS: Distance = Distance::const_meters(0.5);

pub type Direction = bool;
const FORWARDS: Direction = true;
const BACKWARDS: Direction = false;

pub struct Model {
    pub map: RawMap,
    // TODO Not sure this should be pub...
    pub showing_pts: Option<StableRoadID>,

    include_bldgs: bool,
    fixes: MapFixes,
    edit_fixes: Option<String>,
    world: World<ID>,
}

// Construction
impl Model {
    pub fn blank() -> Model {
        Model {
            map: RawMap::blank(String::new()),
            showing_pts: None,

            include_bldgs: false,
            fixes: MapFixes::new(),
            edit_fixes: None,
            world: World::new(&Bounds::new()),
        }
    }

    pub fn import(
        path: &str,
        include_bldgs: bool,
        edit_fixes: Option<String>,
        prerender: &Prerender,
    ) -> Model {
        let mut timer = Timer::new("import map");
        let mut model = Model::blank();
        model.include_bldgs = include_bldgs;
        model.edit_fixes = edit_fixes;
        model.map = read_binary(path, &mut timer).unwrap();

        let mut all_fixes = MapFixes::load(&mut timer);
        model.map.apply_fixes(&all_fixes, &mut timer);
        if let Some(ref name) = model.edit_fixes {
            if let Some(fixes) = all_fixes.remove(name) {
                model.fixes = fixes;
            }
        }

        model.world = World::new(&model.compute_bounds());
        if model.include_bldgs {
            for id in model.map.buildings.keys().cloned().collect::<Vec<_>>() {
                model.bldg_added(id, prerender);
            }
        }
        for id in model.map.intersections.keys().cloned().collect::<Vec<_>>() {
            model.intersection_added(id, prerender);
        }
        for id in model.map.roads.keys().cloned().collect::<Vec<_>>() {
            model.road_added(id, prerender);
        }

        model
    }
}

// General
impl Model {
    pub fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::BLACK);
        g.draw_polygon(Color::rgb(242, 239, 233), &self.map.boundary_polygon);
        self.world.draw(g);
    }

    pub fn handle_mouseover(&mut self, ctx: &EventCtx) {
        self.world.handle_mouseover(ctx);
    }

    pub fn get_selection(&self) -> Option<ID> {
        self.world.get_selection()
    }

    // TODO Only for truly synthetic maps...
    pub fn export(&mut self) {
        assert!(self.map.name != "");
        // TODO Or maybe we should do this more regularly?
        self.map.boundary_polygon = self.compute_bounds().get_rectangle();

        let path = abstutil::path_raw_map(&self.map.name);
        abstutil::write_binary(&path, &self.map).expect(&format!("Saving {} failed", path));
        println!("Exported {}", path);
    }

    pub fn save_fixes(&mut self) {
        let name = if let Some(ref n) = self.edit_fixes {
            n.clone()
        } else {
            println!("Not editing any fixes, so can't save them");
            return;
        };

        // It's easiest to just go back and detect all of the added roads and intersections. But we
        // have to avoid picking up changes from other fixes.
        // TODO Ideally fixes would have a Polygon of where they influence, and all of the polygons
        // would be disjoint. Nothing prevents fixes from being saved in the wrong group, or a
        // created road from one set to be deleted in another -- we're just sure that a fix isn't
        // repeated.
        let mut ignore_roads: BTreeSet<OriginalRoad> = BTreeSet::new();
        let mut ignore_intersections: BTreeSet<OriginalIntersection> = BTreeSet::new();

        for (n, f) in MapFixes::load(&mut Timer::throwaway()) {
            if n != name {
                let (r, i) = f.all_touched_ids();
                ignore_roads.extend(r);
                ignore_intersections.extend(i);
            }
        }

        self.fixes.add_intersections.clear();
        self.fixes.add_roads.clear();
        for i in self.map.intersections.values() {
            if i.synthetic && !ignore_intersections.contains(&i.orig_id) {
                self.fixes.add_intersections.push(i.clone());
            }
        }
        for r in self.map.roads.values() {
            if r.synthetic() && !ignore_roads.contains(&r.orig_id) {
                self.fixes.add_roads.push(r.clone());
            }
        }

        let path = abstutil::path_fixes(&name);
        abstutil::write_json(&path, &self.fixes).unwrap();
        println!("Wrote {}", path);
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
}

// Intersections
impl Model {
    fn intersection_added(&mut self, id: StableIntersectionID, prerender: &Prerender) {
        let i = &self.map.intersections[&id];
        let color = match i.intersection_type {
            IntersectionType::TrafficSignal => Color::GREEN,
            IntersectionType::StopSign => Color::RED,
            IntersectionType::Border => Color::BLUE,
        };
        self.world.add(
            prerender,
            Object::new(
                ID::Intersection(id),
                if i.synthetic { color.alpha(0.5) } else { color },
                Circle::new(i.point, INTERSECTION_RADIUS).to_polygon(),
            )
            .maybe_label(i.label.clone()),
        );
    }

    pub fn create_i(&mut self, point: Pt2D, prerender: &Prerender) {
        let id = self
            .map
            .create_intersection(RawIntersection {
                point,
                intersection_type: IntersectionType::StopSign,
                label: None,
                orig_id: OriginalIntersection {
                    osm_node_id: self.map.new_osm_node_id(),
                },
                synthetic: true,
            })
            .unwrap();
        self.intersection_added(id, prerender);
    }

    pub fn move_i(&mut self, id: StableIntersectionID, point: Pt2D, prerender: &Prerender) {
        self.world.delete(ID::Intersection(id));
        for r in self.map.move_intersection(id, point).unwrap() {
            self.road_deleted(r);
            self.road_added(r, prerender);
        }
        self.intersection_added(id, prerender);
    }

    pub fn set_i_label(&mut self, id: StableIntersectionID, label: String, prerender: &Prerender) {
        self.world.delete(ID::Intersection(id));
        self.map.modify_intersection(
            id,
            self.map.intersections[&id].intersection_type,
            Some(label),
        );
        self.intersection_added(id, prerender);
    }

    pub fn get_i_label(&self, id: StableIntersectionID) -> Option<String> {
        self.map.intersections[&id].label.clone()
    }

    pub fn toggle_i_type(&mut self, id: StableIntersectionID, prerender: &Prerender) {
        self.world.delete(ID::Intersection(id));
        let (it, label) = {
            let i = &self.map.intersections[&id];
            let it = match i.intersection_type {
                IntersectionType::StopSign => IntersectionType::TrafficSignal,
                IntersectionType::TrafficSignal => {
                    if self.map.roads_per_intersection(id).len() == 1 {
                        IntersectionType::Border
                    } else {
                        IntersectionType::StopSign
                    }
                }
                IntersectionType::Border => IntersectionType::StopSign,
            };
            (it, i.label.clone())
        };
        self.map.modify_intersection(id, it, label);
        self.intersection_added(id, prerender);
    }

    pub fn delete_i(&mut self, id: StableIntersectionID) {
        if !self.map.can_delete_intersection(id) {
            println!("Can't delete intersection used by roads");
            return;
        }
        self.map.delete_intersection(id, &mut self.fixes);
        self.world.delete(ID::Intersection(id));
    }

    pub fn get_i_center(&self, id: StableIntersectionID) -> Pt2D {
        self.map.intersections[&id].point
    }
}

// Roads
impl Model {
    fn road_added(&mut self, id: StableRoadID, prerender: &Prerender) {
        for obj in self.lanes(id) {
            self.world.add(prerender, obj);
        }
    }

    fn road_deleted(&mut self, id: StableRoadID) {
        for obj in self.lanes(id) {
            self.world.delete(obj.get_id());
        }
    }

    pub fn create_r(
        &mut self,
        i1: StableIntersectionID,
        i2: StableIntersectionID,
        prerender: &Prerender,
    ) {
        // Ban cul-de-sacs, since they get stripped out later anyway.
        if self
            .map
            .roads
            .values()
            .any(|r| (r.i1 == i1 && r.i2 == i2) || (r.i1 == i2 && r.i2 == i1))
        {
            println!("Road already exists");
            return;
        }

        let osm_way_id = self.map.new_osm_way_id();
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
        osm_tags.insert(osm::OSM_WAY_ID.to_string(), osm_way_id.to_string());
        // Reasonable defaults.
        osm_tags.insert(osm::NAME.to_string(), "Streety McStreetFace".to_string());
        osm_tags.insert(osm::MAXSPEED.to_string(), "25 mph".to_string());

        let id = self
            .map
            .create_road(RawRoad {
                i1,
                i2,
                orig_id: OriginalRoad {
                    osm_way_id,
                    node1: self.map.intersections[&i1].orig_id.osm_node_id,
                    node2: self.map.intersections[&i2].orig_id.osm_node_id,
                },
                center_points: vec![
                    self.map.intersections[&i1].point,
                    self.map.intersections[&i2].point,
                ],
                osm_tags,
            })
            .unwrap();
        self.road_added(id, prerender);
    }

    pub fn edit_lanes(&mut self, id: StableRoadID, spec: String, prerender: &Prerender) {
        self.road_deleted(id);

        if let Some(s) = RoadSpec::parse(spec.clone()) {
            let mut osm_tags = self.map.roads[&id].osm_tags.clone();
            osm_tags.insert(osm::SYNTHETIC_LANES.to_string(), s.to_string());
            self.map.override_tags(id, osm_tags, &mut self.fixes);
        } else {
            println!("Bad RoadSpec: {}", spec);
        }

        self.road_added(id, prerender);
    }

    pub fn swap_lanes(&mut self, id: StableRoadID, prerender: &Prerender) {
        self.road_deleted(id);

        let (mut lanes, mut osm_tags) = {
            let r = &self.map.roads[&id];
            (r.get_spec(), r.osm_tags.clone())
        };
        mem::swap(&mut lanes.fwd, &mut lanes.back);
        osm_tags.insert(osm::SYNTHETIC_LANES.to_string(), lanes.to_string());

        let fwd_label = osm_tags.remove(osm::FWD_LABEL);
        let back_label = osm_tags.remove(osm::BACK_LABEL);
        if let Some(l) = fwd_label {
            osm_tags.insert(osm::BACK_LABEL.to_string(), l);
        }
        if let Some(l) = back_label {
            osm_tags.insert(osm::FWD_LABEL.to_string(), l);
        }

        self.map.override_tags(id, osm_tags, &mut self.fixes);
        self.road_added(id, prerender);
    }

    pub fn set_r_label(
        &mut self,
        pair: (StableRoadID, Direction),
        label: String,
        prerender: &Prerender,
    ) {
        self.road_deleted(pair.0);

        let mut osm_tags = self.map.roads[&pair.0].osm_tags.clone();
        if pair.1 {
            osm_tags.insert(osm::FWD_LABEL.to_string(), label.to_string());
        } else {
            osm_tags.insert(osm::BACK_LABEL.to_string(), label.to_string());
        }

        self.map.override_tags(pair.0, osm_tags, &mut self.fixes);
        self.road_added(pair.0, prerender);
    }

    pub fn get_r_label(&self, pair: (StableRoadID, Direction)) -> Option<String> {
        let r = &self.map.roads[&pair.0];
        if pair.1 {
            r.osm_tags.get(osm::FWD_LABEL).cloned()
        } else {
            r.osm_tags.get(osm::BACK_LABEL).cloned()
        }
    }

    pub fn set_r_name_and_speed(
        &mut self,
        id: StableRoadID,
        name: String,
        speed: String,
        prerender: &Prerender,
    ) {
        self.road_deleted(id);

        let mut osm_tags = self.map.roads[&id].osm_tags.clone();
        osm_tags.insert(osm::NAME.to_string(), name);
        osm_tags.insert(osm::MAXSPEED.to_string(), speed);

        self.map.override_tags(id, osm_tags, &mut self.fixes);
        self.road_added(id, prerender);
    }

    pub fn get_r_name_and_speed(&self, id: StableRoadID) -> (String, String) {
        let r = &self.map.roads[&id];
        (
            r.osm_tags
                .get(osm::NAME)
                .cloned()
                .unwrap_or_else(String::new),
            r.osm_tags
                .get(osm::MAXSPEED)
                .cloned()
                .unwrap_or_else(String::new),
        )
    }

    pub fn delete_r(&mut self, id: StableRoadID) {
        assert!(self.showing_pts != Some(id));
        self.road_deleted(id);
        self.map.delete_road(id, &mut self.fixes);
    }

    pub fn get_road_spec(&self, id: StableRoadID) -> String {
        self.map.roads[&id].get_spec().to_string()
    }

    pub fn get_tags(&self, id: StableRoadID) -> &BTreeMap<String, String> {
        &self.map.roads[&id].osm_tags
    }

    fn lanes(&self, id: StableRoadID) -> Vec<Object<ID>> {
        let r = &self.map.roads[&id];

        let mut tooltip = Text::new();
        if let Some(name) = r.osm_tags.get(osm::NAME) {
            tooltip.add(Line(name));
        } else if let Some(name) = r.osm_tags.get("ref") {
            tooltip.add(Line(name));
        } else {
            tooltip.add(Line("some road"));
        }

        let mut result = Vec::new();
        let synthetic = r.synthetic();
        let spec = r.get_spec();
        let center_pts = PolyLine::new(r.center_points.clone());
        for (idx, lt) in spec.fwd.iter().enumerate() {
            let mut obj = Object::new(
                ID::Lane(id, FORWARDS, idx),
                Model::lt_to_color(*lt, synthetic),
                center_pts
                    .shift_right(LANE_THICKNESS * (0.5 + (idx as f64)))
                    .unwrap()
                    .make_polygons(LANE_THICKNESS),
            );
            if idx == 0 {
                obj = obj.push(
                    Color::YELLOW,
                    center_pts.make_polygons(CENTER_LINE_THICKNESS),
                );
            }
            if idx == spec.fwd.len() / 2 {
                obj = obj.maybe_label(r.osm_tags.get(osm::FWD_LABEL).cloned());
            }
            result.push(obj.tooltip(tooltip.clone()));
        }
        for (idx, lt) in spec.back.iter().enumerate() {
            let mut obj = Object::new(
                ID::Lane(id, BACKWARDS, idx),
                Model::lt_to_color(*lt, synthetic),
                center_pts
                    .reversed()
                    .shift_right(LANE_THICKNESS * (0.5 + (idx as f64)))
                    .unwrap()
                    .make_polygons(LANE_THICKNESS),
            );
            if idx == spec.back.len() / 2 {
                obj = obj.maybe_label(r.osm_tags.get(osm::BACK_LABEL).cloned());
            }
            result.push(obj.tooltip(tooltip.clone()));
        }

        for (turn_id, restriction, to) in self.get_turn_restrictions(id) {
            let polygon = if id == to {
                // TODO Ideally a hollow circle with an arrow
                Circle::new(
                    PolyLine::new(self.map.roads[&id].center_points.clone()).middle(),
                    LANE_THICKNESS,
                )
                .to_polygon()
            } else {
                PolyLine::new(vec![self.get_r_center(id), self.get_r_center(to)])
                    .make_arrow(LANE_THICKNESS)
                    .unwrap()
            };

            result.push(
                Object::new(turn_id, Color::PURPLE, polygon).tooltip(Text::from(Line(restriction))),
            );
        }

        result
    }

    // Copied from render/lane.rs. :(
    fn lt_to_color(lt: LaneType, synthetic: bool) -> Color {
        let color = match lt {
            LaneType::Driving => Color::BLACK,
            LaneType::Bus => Color::rgb(190, 74, 76),
            LaneType::Parking => Color::grey(0.2),
            LaneType::Sidewalk => Color::grey(0.8),
            LaneType::Biking => Color::rgb(15, 125, 75),
        };
        if synthetic {
            color.alpha(0.5)
        } else {
            color
        }
    }

    pub fn show_r_points(&mut self, id: StableRoadID, prerender: &Prerender) {
        assert_eq!(self.showing_pts, None);
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

    pub fn stop_showing_pts(&mut self) {
        let id = self.showing_pts.take().unwrap();

        let r = &self.map.roads[&id];
        for idx in 1..=r.center_points.len() - 2 {
            self.world.delete(ID::RoadPoint(id, idx));
        }
    }

    pub fn move_r_pt(&mut self, id: StableRoadID, idx: usize, point: Pt2D, prerender: &Prerender) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts();
        self.road_deleted(id);

        let mut pts = self.map.roads[&id].center_points.clone();
        pts[idx] = point;
        self.map.override_road_points(id, pts);

        self.road_added(id, prerender);
        self.show_r_points(id, prerender);
    }

    pub fn delete_r_pt(&mut self, id: StableRoadID, idx: usize, prerender: &Prerender) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts();
        self.road_deleted(id);

        let mut pts = self.map.roads[&id].center_points.clone();
        pts.remove(idx);
        self.map.override_road_points(id, pts);

        self.road_added(id, prerender);
        self.show_r_points(id, prerender);
    }

    pub fn merge_r(&mut self, id: StableRoadID, prerender: &Prerender) {
        assert!(self.showing_pts != Some(id));

        if !self.map.can_merge_short_road(id, &self.fixes) {
            println!("Can't merge this road; intersection types must differ or there must be synthetic stuff");
            return;
        }

        // TODO Bit hacky, but we have to do this before doing the mutation, so we know the number
        // of lanes and can generate all the IDs.
        self.road_deleted(id);

        let (deleted_i, changed_roads) = self.map.merge_short_road(id, &mut self.fixes).unwrap();

        self.world.delete(ID::Intersection(deleted_i));
        for r in changed_roads {
            self.road_deleted(r);
            self.road_added(r, prerender);
        }
    }

    pub fn get_r_center(&self, id: StableRoadID) -> Pt2D {
        PolyLine::new(self.map.roads[&id].center_points.clone()).middle()
    }
}

// Turn restrictions
impl Model {
    pub fn get_turn_restrictions(&self, id: StableRoadID) -> Vec<(ID, String, StableRoadID)> {
        self.map
            .get_turn_restrictions(id)
            .into_iter()
            .enumerate()
            .map(|(idx, (r, to))| (ID::TurnRestriction(id, to, idx), r, to))
            .collect()
    }

    pub fn add_tr(
        &mut self,
        from: StableRoadID,
        restriction: String,
        to: StableRoadID,
        prerender: &Prerender,
    ) {
        self.road_deleted(from);

        self.map.add_turn_restriction(from, restriction, to);

        self.road_added(from, prerender);
    }

    pub fn delete_tr(
        &mut self,
        from: StableRoadID,
        to: StableRoadID,
        idx: usize,
        prerender: &Prerender,
    ) {
        self.road_deleted(from);

        let (_, ref restriction, _) = self.get_turn_restrictions(from)[idx];
        self.map
            .delete_turn_restriction(from, restriction.clone(), to);

        self.road_added(from, prerender);
    }
}

// Buildings
impl Model {
    fn bldg_added(&mut self, id: StableBuildingID, prerender: &Prerender) {
        let b = &self.map.buildings[&id];
        self.world.add(
            prerender,
            Object::new(ID::Building(id), Color::BLUE, b.polygon.clone())
                .maybe_label(b.osm_tags.get(osm::LABEL).cloned()),
        );
    }

    pub fn create_b(&mut self, center: Pt2D, prerender: &Prerender) {
        let id = self
            .map
            .create_building(RawBuilding {
                polygon: Polygon::rectangle(center, BUILDING_LENGTH, BUILDING_LENGTH),
                osm_tags: BTreeMap::new(),
                osm_way_id: self.map.new_osm_way_id(),
                parking: None,
            })
            .unwrap();
        self.bldg_added(id, prerender);
    }

    pub fn move_b(&mut self, id: StableBuildingID, new_center: Pt2D, prerender: &Prerender) {
        self.world.delete(ID::Building(id));

        let (polygon, osm_tags) = {
            let b = &self.map.buildings[&id];
            let old_center = b.polygon.center();
            let polygon = b.polygon.translate(
                Distance::meters(new_center.x() - old_center.x()),
                Distance::meters(new_center.y() - old_center.y()),
            );
            (polygon, b.osm_tags.clone())
        };
        self.map.modify_building(id, polygon, osm_tags);

        self.bldg_added(id, prerender);
    }

    pub fn set_b_label(&mut self, id: StableBuildingID, label: String, prerender: &Prerender) {
        self.world.delete(ID::Building(id));

        let (polygon, osm_tags) = {
            let b = &self.map.buildings[&id];
            let mut osm_tags = b.osm_tags.clone();
            osm_tags.insert(osm::LABEL.to_string(), label);
            (b.polygon.clone(), osm_tags)
        };
        self.map.modify_building(id, polygon, osm_tags);

        self.bldg_added(id, prerender);
    }

    pub fn get_b_label(&self, id: StableBuildingID) -> Option<String> {
        self.map.buildings[&id].osm_tags.get(osm::LABEL).cloned()
    }

    pub fn delete_b(&mut self, id: StableBuildingID) {
        self.world.delete(ID::Building(id));

        self.map.delete_building(id);
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ID {
    Building(StableBuildingID),
    Intersection(StableIntersectionID),
    Lane(StableRoadID, Direction, usize),
    RoadPoint(StableRoadID, usize),
    TurnRestriction(StableRoadID, StableRoadID, usize),
}

impl ObjectID for ID {
    fn zorder(&self) -> usize {
        match self {
            ID::Lane(_, _, _) => 0,
            ID::Intersection(_) => 1,
            ID::Building(_) => 2,
            ID::RoadPoint(_, _) => 3,
            ID::TurnRestriction(_, _, _) => 4,
        }
    }
}
