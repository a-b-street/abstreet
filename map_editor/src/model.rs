use std::collections::{BTreeMap, HashMap};
use std::io::Write;

use abstio::MapName;
use abstutil::{Tags, Timer};
use geom::{
    Bounds, Circle, Distance, FindClosest, GPSBounds, HashablePt2D, LonLat, PolyLine, Polygon, Pt2D,
};
use map_model::raw::{OriginalRoad, RawBuilding, RawIntersection, RawMap, RawRoad};
use map_model::{osm, IntersectionType};
use widgetry::{Color, EventCtx, Line, Text};

use crate::world::{Object, ObjectID, World};

const INTERSECTION_RADIUS: Distance = Distance::const_meters(2.5);
const BUILDING_LENGTH: Distance = Distance::const_meters(30.0);

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
            map: RawMap::blank(MapName {
                city: String::new(),
                map: String::new(),
            }),
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
        ctx: &EventCtx,
    ) -> Model {
        let mut timer = Timer::new("import map");
        let mut model = Model::blank();
        model.include_bldgs = include_bldgs;
        model.map = abstio::read_binary(path, &mut timer);
        model.intersection_geom = intersection_geom;

        if model.include_bldgs {
            for id in model.map.buildings.keys().cloned().collect::<Vec<_>>() {
                model.bldg_added(id, ctx);
            }
        }
        timer.start_iter(
            "fill out world with intersections",
            model.map.intersections.len(),
        );
        for id in model.map.intersections.keys().cloned().collect::<Vec<_>>() {
            timer.next();
            model.intersection_added(id, ctx);
        }
        timer.start_iter("fill out world with roads", model.map.roads.len());
        for id in model.map.roads.keys().cloned().collect::<Vec<_>>() {
            timer.next();
            model.road_added(id, ctx);
        }

        model
    }
}

// General
impl Model {
    // TODO Only for truly synthetic maps...
    pub fn export_to_osm(&mut self) {
        // Shift the map to start at (0, 0)
        let bounds = self.compute_bounds();
        if bounds.min_x != 0.0 || bounds.min_y != 0.0 {
            for b in self.map.buildings.values_mut() {
                b.polygon = b.polygon.translate(-bounds.min_x, -bounds.min_y);
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
        let mut seattle_bounds = GPSBounds::new();
        seattle_bounds.update(LonLat::new(-122.453224, 47.723277));
        seattle_bounds.update(LonLat::new(-122.240505, 47.495342));

        self.map
            .gps_bounds
            .update(Pt2D::new(bounds.min_x, bounds.min_y).to_gps(&seattle_bounds));
        self.map
            .gps_bounds
            .update(Pt2D::new(bounds.max_x, bounds.max_y).to_gps(&seattle_bounds));

        dump_to_osm(&self.map).unwrap();
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

    pub fn describe_obj(&self, id: ID) -> Text {
        let mut txt = Text::new().with_bg();
        match id {
            ID::Building(b) => {
                txt.add_highlighted(Line(b.to_string()), Color::BLUE);
                for (k, v) in self.map.buildings[&b].osm_tags.inner() {
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

                for (k, v) in road.osm_tags.inner() {
                    txt.add_appended(vec![
                        Line(k).fg(Color::RED),
                        Line(" = "),
                        Line(v).fg(Color::CYAN),
                    ]);
                }

                // (MAX_CAR_LENGTH + sim::FOLLOWING_DISTANCE) from sim, but without the dependency
                txt.add(Line(format!(
                    "Can fit ~{} cars",
                    (PolyLine::must_new(road.center_points.clone()).length()
                        / (Distance::meters(6.5 + 1.0)))
                    .floor() as usize
                )));
            }
            ID::RoadPoint(r, idx) => {
                txt.add_highlighted(Line(format!("Point {}", idx)), Color::BLUE);
                txt.add(Line(format!("of {}", r)));
            }
        }
        txt
    }
}

// Intersections
impl Model {
    fn intersection_added(&mut self, id: osm::NodeID, ctx: &EventCtx) {
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
            .add(ctx, Object::new(ID::Intersection(id), color, poly));
    }

    pub fn create_i(&mut self, point: Pt2D, ctx: &EventCtx) {
        let id = self.map.new_osm_node_id(time_to_id());
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
        self.intersection_added(id, ctx);
    }

    pub fn move_i(&mut self, id: osm::NodeID, point: Pt2D, ctx: &EventCtx) {
        self.world.delete(ID::Intersection(id));
        for r in self.map.move_intersection(id, point).unwrap() {
            self.road_deleted(r);
            self.road_added(r, ctx);
        }
        self.intersection_added(id, ctx);
    }

    pub fn delete_i(&mut self, id: osm::NodeID) {
        if !self.map.can_delete_intersection(id) {
            error!("Can't delete intersection used by roads");
            return;
        }
        self.map.delete_intersection(id);
        self.world.delete(ID::Intersection(id));
    }
}

// Roads
impl Model {
    fn road_added(&mut self, id: OriginalRoad, ctx: &EventCtx) {
        self.world.add(ctx, self.road_object(id));
    }

    fn road_deleted(&mut self, id: OriginalRoad) {
        self.world.delete(ID::Road(id));
    }

    pub fn create_r(&mut self, i1: osm::NodeID, i2: osm::NodeID, ctx: &EventCtx) {
        // Ban cul-de-sacs, since they get stripped out later anyway.
        if self
            .map
            .roads
            .keys()
            .any(|r| (r.i1 == i1 && r.i2 == i2) || (r.i1 == i2 && r.i2 == i1))
        {
            error!("Road already exists");
            return;
        }

        let id = OriginalRoad {
            osm_way_id: self.map.new_osm_way_id(time_to_id()),
            i1,
            i2,
        };
        let mut osm_tags = Tags::new(BTreeMap::new());
        osm_tags.insert(osm::HIGHWAY, "residential");
        osm_tags.insert(osm::PARKING_BOTH, "parallel");
        osm_tags.insert(osm::SIDEWALK, "both");
        osm_tags.insert("lanes", "2");
        osm_tags.insert(osm::ENDPT_FWD, "true");
        osm_tags.insert(osm::ENDPT_BACK, "true");
        osm_tags.insert(osm::OSM_WAY_ID, id.osm_way_id.to_string());
        // Reasonable defaults.
        osm_tags.insert(osm::NAME, "Streety McStreetFace");
        osm_tags.insert(osm::MAXSPEED, "25 mph");

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
        self.road_added(id, ctx);
    }

    pub fn delete_r(&mut self, id: OriginalRoad) {
        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.map.roads.remove(&id).unwrap();
    }

    fn road_object(&self, id: OriginalRoad) -> Object<ID> {
        let (center, total_width) = self.map.roads[&id].get_geometry(id, &self.map.config);
        Object::new(
            ID::Road(id),
            Color::grey(0.8),
            center.make_polygons(total_width),
        )
    }

    pub fn show_r_points(&mut self, id: OriginalRoad, ctx: &EventCtx) {
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
                    ctx,
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

    pub fn move_r_pt(&mut self, id: OriginalRoad, idx: usize, point: Pt2D, ctx: &EventCtx) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));

        let pts = &mut self.map.roads.get_mut(&id).unwrap().center_points;
        pts[idx] = point;

        self.road_added(id, ctx);
        self.intersection_added(id.i1, ctx);
        self.intersection_added(id.i2, ctx);
        self.show_r_points(id, ctx);
    }

    pub fn delete_r_pt(&mut self, id: OriginalRoad, idx: usize, ctx: &EventCtx) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));

        let pts = &mut self.map.roads.get_mut(&id).unwrap().center_points;
        pts.remove(idx);

        self.road_added(id, ctx);
        self.intersection_added(id.i1, ctx);
        self.intersection_added(id.i2, ctx);
        self.show_r_points(id, ctx);
    }

    pub fn insert_r_pt(&mut self, id: OriginalRoad, pt: Pt2D, ctx: &EventCtx) -> Option<ID> {
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
            error!("Couldn't figure out where to insert new point");
            None
        };

        self.road_added(id, ctx);
        self.intersection_added(id.i1, ctx);
        self.intersection_added(id.i2, ctx);
        self.show_r_points(id, ctx);

        new_id
    }

    pub fn clear_r_pts(&mut self, id: OriginalRoad, ctx: &EventCtx) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));

        let r = &mut self.map.roads.get_mut(&id).unwrap();
        r.center_points = vec![r.center_points[0], *r.center_points.last().unwrap()];

        self.road_added(id, ctx);
        self.intersection_added(id.i1, ctx);
        self.intersection_added(id.i2, ctx);
        self.show_r_points(id, ctx);
    }

    // TODO Need to show_r_points of the thing we wind up selecting after this.
    pub fn merge_r(&mut self, id: OriginalRoad, ctx: &EventCtx) {
        self.stop_showing_pts(id);

        let (retained_i, deleted_i, deleted_roads, created_roads) =
            match self.map.merge_short_road(id) {
                Ok((retained_i, deleted_i, deleted_roads, created_roads)) => {
                    (retained_i, deleted_i, deleted_roads, created_roads)
                }
                Err(err) => {
                    warn!("Can't merge this road: {}", err);
                    self.show_r_points(id, ctx);
                    return;
                }
            };

        self.world.delete(ID::Intersection(retained_i));
        self.intersection_added(retained_i, ctx);

        self.world.delete(ID::Intersection(deleted_i));

        for r in deleted_roads {
            self.world.delete(ID::Road(r));
        }
        for r in created_roads {
            self.road_added(r, ctx);
        }

        info!("Merged {}", id.to_string_code());
    }
}

// Buildings
impl Model {
    fn bldg_added(&mut self, id: osm::OsmID, ctx: &EventCtx) {
        let b = &self.map.buildings[&id];
        self.world.add(
            ctx,
            Object::new(ID::Building(id), Color::BLUE, b.polygon.clone()),
        );
    }

    pub fn create_b(&mut self, center: Pt2D, ctx: &EventCtx) -> ID {
        let id = osm::OsmID::Way(self.map.new_osm_way_id(time_to_id()));
        self.map.buildings.insert(
            id,
            RawBuilding {
                polygon: Polygon::rectangle_centered(center, BUILDING_LENGTH, BUILDING_LENGTH),
                osm_tags: Tags::new(BTreeMap::new()),
                public_garage_name: None,
                num_parking_spots: 0,
                amenities: Vec::new(),
            },
        );
        self.bldg_added(id, ctx);
        ID::Building(id)
    }

    pub fn move_b(&mut self, id: osm::OsmID, new_center: Pt2D, ctx: &EventCtx) {
        self.world.delete(ID::Building(id));

        let b = self.map.buildings.get_mut(&id).unwrap();
        let old_center = b.polygon.center();
        b.polygon = b.polygon.translate(
            new_center.x() - old_center.x(),
            new_center.y() - old_center.y(),
        );

        self.bldg_added(id, ctx);
    }

    pub fn delete_b(&mut self, id: osm::OsmID) {
        self.world.delete(ID::Building(id));
        self.map.buildings.remove(&id).unwrap();
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ID {
    Building(osm::OsmID),
    Intersection(osm::NodeID),
    Road(OriginalRoad),
    RoadPoint(OriginalRoad, usize),
}

impl ObjectID for ID {
    fn zorder(&self) -> usize {
        match self {
            ID::Road(_) => 0,
            ID::Intersection(_) => 1,
            ID::Building(_) => 2,
            ID::RoadPoint(_, _) => 3,
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

/// Express a RawMap as a .osm file. Why not just save the RawMap? The format may change over time,
/// and even if a RawMap is saved as JSON, manually updating it is annoying. This is used to create
/// synthetic maps that will never go bad -- there will always be a pipeline to import a .osm file,
/// so actually, .osm is a stable-over-time format.
fn dump_to_osm(map: &RawMap) -> Result<(), std::io::Error> {
    let mut f = std::fs::File::create("synthetic_export.osm")?;
    writeln!(f, r#"<?xml version='1.0' encoding='UTF-8'?>"#)?;
    writeln!(f, r#"<osm>"#)?;
    writeln!(
        f,
        r#"<!-- If you couldn't tell, this is a fake .osm file not representing the real world. -->"#
    )?;
    let b = &map.gps_bounds;
    writeln!(
        f,
        r#"    <bounds minlon="{}" maxlon="{}" minlat="{}" maxlat="{}"/>"#,
        b.min_lon, b.max_lon, b.min_lat, b.max_lat
    )?;
    let mut pt_to_id: HashMap<HashablePt2D, osm::NodeID> = HashMap::new();
    for (id, i) in &map.intersections {
        pt_to_id.insert(i.point.to_hashable(), *id);
        let pt = i.point.to_gps(b);
        writeln!(
            f,
            r#"    <node id="{}" lon="{}" lat="{}"/>"#,
            id.0,
            pt.x(),
            pt.y()
        )?;
    }
    for (id, r) in &map.roads {
        writeln!(f, r#"    <way id="{}">"#, id.osm_way_id.0)?;
        for pt in &r.center_points {
            // TODO Make new IDs if needed
            writeln!(
                f,
                r#"        <nd ref="{}"/>"#,
                pt_to_id[&pt.to_hashable()].0
            )?;
        }
        for (k, v) in r.osm_tags.inner() {
            if !k.starts_with("abst:") {
                writeln!(f, r#"        <tag k="{}" v="{}"/>"#, k, v)?;
            }
        }
        writeln!(f, r#"    </way>"#)?;
    }
    writeln!(f, r#"</osm>"#)?;
    Ok(())
}
