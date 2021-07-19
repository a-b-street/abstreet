use std::collections::{BTreeMap, HashMap};
use std::io::Write;

use abstio::{CityName, MapName};
use abstutil::{Tags, Timer};
use geom::{Bounds, Circle, Distance, FindClosest, GPSBounds, HashablePt2D, LonLat, Polygon, Pt2D};
use map_model::raw::{OriginalRoad, RawBuilding, RawIntersection, RawMap, RawRoad};
use map_model::{osm, IntersectionType};
use widgetry::{Color, EventCtx};

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
    intersection_geom: bool,
}

// Construction
impl Model {
    pub fn blank() -> Model {
        Model {
            map: RawMap::blank(MapName {
                city: CityName {
                    country: String::new(),
                    city: String::new(),
                },
                map: String::new(),
            }),
            showing_pts: None,

            include_bldgs: false,
            world: World::new(),
            intersection_geom: false,
        }
    }

    pub fn import(ctx: &EventCtx, path: String, include_bldgs: bool) -> Model {
        let mut timer = Timer::new("import map");
        let mut model = Model::blank();
        model.include_bldgs = include_bldgs;

        model.map = if path.ends_with(".osm") {
            convert_osm::convert(
                convert_osm::Options {
                    name: MapName::new("zz", "oneshot", &abstutil::basename(&path)),
                    osm_input: path,
                    clip: None,
                    map_config: map_model::MapConfig {
                        driving_side: map_model::DrivingSide::Right,
                        bikes_can_use_bus_lanes: true,
                        inferred_sidewalks: true,
                        street_parking_spot_length: Distance::meters(8.0),
                    },
                    onstreet_parking: convert_osm::OnstreetParking::JustOSM,
                    public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
                    private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(
                        0,
                    ),
                    include_railroads: true,
                    extra_buildings: None,
                    skip_local_roads: false,
                },
                &mut timer,
            )
        } else {
            abstio::read_binary(path, &mut timer)
        };

        if model.include_bldgs {
            for id in model.map.buildings.keys().cloned().collect::<Vec<_>>() {
                model.bldg_added(ctx, id);
            }
        }
        timer.start_iter(
            "fill out world with intersections",
            model.map.intersections.len(),
        );
        for id in model.map.intersections.keys().cloned().collect::<Vec<_>>() {
            timer.next();
            model.intersection_added(ctx, id);
        }
        timer.start_iter("fill out world with roads", model.map.roads.len());
        for id in model.map.roads.keys().cloned().collect::<Vec<_>>() {
            timer.next();
            model.road_added(ctx, id);
        }

        model
    }
}

// General
impl Model {
    pub fn export_to_osm(&mut self) {
        dump_to_osm(&self.map).unwrap();
    }

    pub fn set_boundary(&mut self, ctx: &EventCtx, top_left: Pt2D, bottom_right: Pt2D) {
        // Shift the map to treat top_left as (0, 0)
        for b in self.map.buildings.values_mut() {
            b.polygon = b.polygon.translate(-top_left.x(), -top_left.y());
        }
        for i in self.map.intersections.values_mut() {
            i.point = i.point.offset(-top_left.x(), -top_left.y());
        }
        for r in self.map.roads.values_mut() {
            for pt in &mut r.center_points {
                *pt = pt.offset(-top_left.x(), -top_left.y());
            }
        }
        let pt1 = Pt2D::new(0.0, 0.0);
        let pt2 = bottom_right.offset(-top_left.x(), -top_left.y());

        self.map.boundary_polygon = Polygon::rectangle_two_corners(pt1, pt2).unwrap();

        // Make gps_bounds sane
        let mut seattle_bounds = GPSBounds::new();
        seattle_bounds.update(LonLat::new(-122.453224, 47.723277));
        seattle_bounds.update(LonLat::new(-122.240505, 47.495342));

        self.map.gps_bounds = GPSBounds::new();
        self.map.gps_bounds.update(pt1.to_gps(&seattle_bounds));
        self.map.gps_bounds.update(pt2.to_gps(&seattle_bounds));

        // Re-add everything to the world, since we just shifted coordinates around
        self.world = World::new();
        for id in self.map.buildings.keys().cloned().collect::<Vec<_>>() {
            self.bldg_added(ctx, id);
        }
        for id in self.map.intersections.keys().cloned().collect::<Vec<_>>() {
            self.intersection_added(ctx, id);
        }
        for id in self.map.roads.keys().cloned().collect::<Vec<_>>() {
            self.road_added(ctx, id);
        }
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
}

// Intersections
impl Model {
    fn intersection_added(&mut self, ctx: &EventCtx, id: osm::NodeID) {
        let i = &self.map.intersections[&id];
        let color = match i.intersection_type {
            IntersectionType::TrafficSignal => Color::GREEN,
            IntersectionType::StopSign => Color::RED,
            IntersectionType::Border => Color::BLUE,
            IntersectionType::Construction => Color::ORANGE,
        };

        let poly = if self.intersection_geom && !self.map.roads_per_intersection(id).is_empty() {
            let (poly, _, _) = self.map.preview_intersection(id);
            poly
        } else {
            Circle::new(i.point, INTERSECTION_RADIUS).to_polygon()
        };

        self.world
            .add(ctx, Object::new(ID::Intersection(id), color, poly));
    }

    pub fn create_i(&mut self, ctx: &EventCtx, point: Pt2D) {
        let id = self.map.new_osm_node_id(time_to_id());
        self.map.intersections.insert(
            id,
            RawIntersection {
                point,
                intersection_type: IntersectionType::StopSign,
                elevation: Distance::ZERO,
                trim_roads_for_merging: BTreeMap::new(),
            },
        );
        self.intersection_added(ctx, id);
    }

    pub fn move_i(&mut self, ctx: &EventCtx, id: osm::NodeID, point: Pt2D) {
        self.world.delete(ID::Intersection(id));
        for r in self.map.move_intersection(id, point).unwrap() {
            self.road_deleted(r);
            self.road_added(ctx, r);
        }
        self.intersection_added(ctx, id);
    }

    pub fn delete_i(&mut self, id: osm::NodeID) {
        if !self.map.can_delete_intersection(id) {
            error!("Can't delete intersection used by roads");
            return;
        }
        self.map.delete_intersection(id);
        self.world.delete(ID::Intersection(id));
    }

    pub fn toggle_i(&mut self, ctx: &EventCtx, id: osm::NodeID) {
        self.world.delete(ID::Intersection(id));

        let i = self.map.intersections.get_mut(&id).unwrap();
        if i.intersection_type == IntersectionType::TrafficSignal {
            i.intersection_type = IntersectionType::StopSign;
        } else if i.intersection_type == IntersectionType::StopSign {
            i.intersection_type = IntersectionType::TrafficSignal;
        }

        self.intersection_added(ctx, id);
    }

    pub fn show_intersection_geometry(&mut self, ctx: &EventCtx, show: bool) {
        self.intersection_geom = show;

        for id in self.map.intersections.keys().cloned().collect::<Vec<_>>() {
            self.world.delete(ID::Intersection(id));
            self.intersection_added(ctx, id);
        }
    }
}

// Roads
impl Model {
    pub fn road_added(&mut self, ctx: &EventCtx, id: OriginalRoad) {
        self.world.add(ctx, self.road_object(id));
    }

    pub fn road_deleted(&mut self, id: OriginalRoad) {
        self.world.delete(ID::Road(id));
    }

    pub fn create_r(&mut self, ctx: &EventCtx, i1: osm::NodeID, i2: osm::NodeID) {
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
        let mut osm_tags = Tags::empty();
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

        self.world.delete(ID::Intersection(i1));
        self.world.delete(ID::Intersection(i2));

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
                percent_incline: 0.0,
            },
        );
        self.road_added(ctx, id);

        self.intersection_added(ctx, i1);
        self.intersection_added(ctx, i2);
    }

    pub fn delete_r(&mut self, ctx: &EventCtx, id: OriginalRoad) {
        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));
        self.map.roads.remove(&id).unwrap();

        self.intersection_added(ctx, id.i1);
        self.intersection_added(ctx, id.i2);
    }

    fn road_object(&self, id: OriginalRoad) -> Object<ID> {
        let road = &self.map.roads[&id];
        let (center, total_width) = road.get_geometry(id, &self.map.config).unwrap();
        Object::new(
            ID::Road(id),
            if road.osm_tags.is("junction", "intersection") {
                Color::PINK
            } else {
                Color::grey(0.8)
            },
            center.make_polygons(total_width),
        )
    }

    pub fn show_r_points(&mut self, ctx: &EventCtx, id: OriginalRoad) {
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

    pub fn move_r_pt(&mut self, ctx: &EventCtx, id: OriginalRoad, idx: usize, point: Pt2D) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));

        let pts = &mut self.map.roads.get_mut(&id).unwrap().center_points;
        pts[idx] = point;

        self.road_added(ctx, id);
        self.intersection_added(ctx, id.i1);
        self.intersection_added(ctx, id.i2);
        self.show_r_points(ctx, id);
    }

    pub fn delete_r_pt(&mut self, ctx: &EventCtx, id: OriginalRoad, idx: usize) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));

        let pts = &mut self.map.roads.get_mut(&id).unwrap().center_points;
        pts.remove(idx);

        self.road_added(ctx, id);
        self.intersection_added(ctx, id.i1);
        self.intersection_added(ctx, id.i2);
        self.show_r_points(ctx, id);
    }

    pub fn insert_r_pt(&mut self, ctx: &EventCtx, id: OriginalRoad, pt: Pt2D) -> Option<ID> {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));

        let mut closest = FindClosest::new(&self.compute_bounds());
        let pts = &mut self.map.roads.get_mut(&id).unwrap().center_points;
        for (idx, pair) in pts.windows(2).enumerate() {
            closest.add(idx + 1, &[pair[0], pair[1]]);
        }
        let new_id = if let Some((idx, _)) = closest.closest_pt(pt, Distance::meters(5.0)) {
            pts.insert(idx, pt);
            Some(ID::RoadPoint(id, idx))
        } else {
            error!("Couldn't figure out where to insert new point");
            None
        };

        self.road_added(ctx, id);
        self.intersection_added(ctx, id.i1);
        self.intersection_added(ctx, id.i2);
        self.show_r_points(ctx, id);

        new_id
    }

    pub fn clear_r_pts(&mut self, ctx: &EventCtx, id: OriginalRoad) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world.delete(ID::Intersection(id.i1));
        self.world.delete(ID::Intersection(id.i2));

        let r = &mut self.map.roads.get_mut(&id).unwrap();
        r.center_points = vec![r.center_points[0], *r.center_points.last().unwrap()];

        self.road_added(ctx, id);
        self.intersection_added(ctx, id.i1);
        self.intersection_added(ctx, id.i2);
        self.show_r_points(ctx, id);
    }

    // TODO Need to show_r_points of the thing we wind up selecting after this.
    pub fn merge_r(&mut self, ctx: &EventCtx, id: OriginalRoad) {
        self.stop_showing_pts(id);

        let (retained_i, deleted_i, deleted_roads, created_roads) =
            match self.map.merge_short_road(id) {
                Ok((retained_i, deleted_i, deleted_roads, created_roads)) => {
                    (retained_i, deleted_i, deleted_roads, created_roads)
                }
                Err(err) => {
                    warn!("Can't merge this road: {}", err);
                    self.show_r_points(ctx, id);
                    return;
                }
            };

        self.world.delete(ID::Intersection(retained_i));
        self.intersection_added(ctx, retained_i);

        self.world.delete(ID::Intersection(deleted_i));

        for r in deleted_roads {
            self.world.delete(ID::Road(r));
        }
        for r in created_roads {
            self.road_added(ctx, r);
        }

        info!("Merged {}", id.as_string_code());
    }

    pub fn toggle_junction(&mut self, ctx: &EventCtx, id: OriginalRoad) {
        self.road_deleted(id);

        let road = self.map.roads.get_mut(&id).unwrap();
        if road.osm_tags.is("junction", "intersection") {
            road.osm_tags.remove("junction");
        } else {
            road.osm_tags.insert("junction", "intersection");
        }

        self.road_added(ctx, id);
    }
}

// Buildings
impl Model {
    fn bldg_added(&mut self, ctx: &EventCtx, id: osm::OsmID) {
        let b = &self.map.buildings[&id];
        self.world.add(
            ctx,
            Object::new(ID::Building(id), Color::BLUE, b.polygon.clone()),
        );
    }

    pub fn create_b(&mut self, ctx: &EventCtx, center: Pt2D) -> ID {
        let id = osm::OsmID::Way(self.map.new_osm_way_id(time_to_id()));
        self.map.buildings.insert(
            id,
            RawBuilding {
                polygon: Polygon::rectangle_centered(center, BUILDING_LENGTH, BUILDING_LENGTH),
                osm_tags: Tags::empty(),
                public_garage_name: None,
                num_parking_spots: 0,
                amenities: Vec::new(),
            },
        );
        self.bldg_added(ctx, id);
        ID::Building(id)
    }

    pub fn move_b(&mut self, ctx: &EventCtx, id: osm::OsmID, new_center: Pt2D) {
        self.world.delete(ID::Building(id));

        let b = self.map.buildings.get_mut(&id).unwrap();
        let old_center = b.polygon.center();
        b.polygon = b.polygon.translate(
            new_center.x() - old_center.x(),
            new_center.y() - old_center.y(),
        );

        self.bldg_added(ctx, id);
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
