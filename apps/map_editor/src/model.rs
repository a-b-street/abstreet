use std::collections::HashMap;
use std::io::Write;

use abstio::{CityName, MapName};
use abstutil::{Tags, Timer};
use geom::{
    Bounds, Circle, Distance, FindClosest, GPSBounds, HashablePt2D, LonLat, PolyLine, Polygon, Pt2D,
};
use osm2streets::{
    osm, ControlType, Intersection, IntersectionComplexity, OriginalRoad, Road, Transformation,
};
use raw_map::{RawBuilding, RawMap};
use widgetry::mapspace::{ObjectID, World};
use widgetry::{Color, EventCtx, GeomBatch, Key};

const INTERSECTION_RADIUS: Distance = Distance::const_meters(2.5);
const BUILDING_LENGTH: Distance = Distance::const_meters(30.0);

// The caller should generally call world.initialize_hover after a mutation.
pub struct Model {
    // map and world are pub. The main crate should use them directly for simple stuff, to avoid
    // boilerplate delegation methods. Complex changes should be proper methods on the model.
    pub map: RawMap,
    showing_pts: Option<OriginalRoad>,
    pub world: World<ID>,

    pub include_bldgs: bool,
    pub intersection_geom: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ID {
    Building(osm::OsmID),
    Intersection(osm::NodeID),
    Road(OriginalRoad),
    RoadPoint(OriginalRoad, usize),
}
impl ObjectID for ID {}

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
            world: World::unbounded(),
            intersection_geom: false,
        }
    }

    pub fn from_map(ctx: &EventCtx, map: RawMap, include_bldgs: bool, timer: &mut Timer) -> Model {
        let mut model = Model::blank();
        model.include_bldgs = include_bldgs;
        model.map = map;
        model.recreate_world(ctx, timer);
        model
    }

    pub fn recreate_world(&mut self, ctx: &EventCtx, timer: &mut Timer) {
        self.showing_pts = None;
        self.world = World::unbounded();

        if self.include_bldgs {
            for id in self.map.buildings.keys().cloned().collect::<Vec<_>>() {
                self.bldg_added(ctx, id);
            }
        }
        timer.start_iter(
            "fill out world with intersections",
            self.map.streets.intersections.len(),
        );
        for id in self
            .map
            .streets
            .intersections
            .keys()
            .cloned()
            .collect::<Vec<_>>()
        {
            timer.next();
            self.intersection_added(ctx, id);
        }
        timer.start_iter("fill out world with roads", self.map.streets.roads.len());
        for id in self.map.streets.roads.keys().cloned().collect::<Vec<_>>() {
            timer.next();
            self.road_added(ctx, id);
        }

        self.world.initialize_hover(ctx);
        // No need to restore dragging
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
        for i in self.map.streets.intersections.values_mut() {
            i.point = i.point.offset(-top_left.x(), -top_left.y());
        }
        for r in self.map.streets.roads.values_mut() {
            r.untrimmed_center_line = PolyLine::must_new(
                r.untrimmed_center_line
                    .points()
                    .iter()
                    .map(|pt| pt.offset(-top_left.x(), -top_left.y()))
                    .collect(),
            );
        }
        let pt1 = Pt2D::new(0.0, 0.0);
        let pt2 = bottom_right.offset(-top_left.x(), -top_left.y());

        self.map.streets.boundary_polygon = Polygon::rectangle_two_corners(pt1, pt2).unwrap();

        // Make gps_bounds sane
        let mut seattle_bounds = GPSBounds::new();
        seattle_bounds.update(LonLat::new(-122.453224, 47.723277));
        seattle_bounds.update(LonLat::new(-122.240505, 47.495342));

        self.map.streets.gps_bounds = GPSBounds::new();
        self.map
            .streets
            .gps_bounds
            .update(pt1.to_gps(&seattle_bounds));
        self.map
            .streets
            .gps_bounds
            .update(pt2.to_gps(&seattle_bounds));

        self.recreate_world(ctx, &mut Timer::throwaway());
    }

    fn compute_bounds(&self) -> Bounds {
        let mut bounds = Bounds::new();
        for b in self.map.buildings.values() {
            for pt in b.polygon.get_outer_ring().points() {
                bounds.update(*pt);
            }
        }
        for i in self.map.streets.intersections.values() {
            bounds.update(i.point);
        }
        for r in self.map.streets.roads.values() {
            for pt in r.untrimmed_center_line.points() {
                bounds.update(*pt);
            }
        }
        bounds
    }
}

// Intersections
impl Model {
    fn intersection_added(&mut self, ctx: &EventCtx, id: osm::NodeID) {
        let i = &self.map.streets.intersections[&id];
        let color = match i.control {
            ControlType::TrafficSignal => Color::GREEN,
            ControlType::StopSign | ControlType::Uncontrolled => Color::RED,
            ControlType::Border => Color::BLUE,
            ControlType::Construction => Color::ORANGE,
        };

        let poly =
            if self.intersection_geom && !self.map.streets.roads_per_intersection(id).is_empty() {
                self.map.streets.intersections[&id].polygon.clone()
            } else {
                Circle::new(i.point, INTERSECTION_RADIUS).to_polygon()
            };

        self.world
            .add(ID::Intersection(id))
            .hitbox(poly)
            .zorder(1)
            .draw_color(color)
            .hover_alpha(0.5)
            .draggable()
            .hotkey(Key::R, "start a road here")
            .hotkey(Key::Backspace, "delete")
            .hotkey(Key::T, "toggle stop sign / traffic signal")
            .hotkey(Key::P, "debug intersection geometry")
            .hotkey(Key::D, "debug in OSM")
            .build(ctx);
    }

    pub fn create_i(&mut self, ctx: &EventCtx, point: Pt2D) {
        let id = self.map.new_osm_node_id(time_to_id());
        // The complexity will change as we connect things to this intersection
        self.map.streets.intersections.insert(
            id,
            Intersection::new(
                id,
                point,
                IntersectionComplexity::Crossing,
                ControlType::StopSign,
            ),
        );
        self.intersection_added(ctx, id);
    }

    pub fn move_i(&mut self, ctx: &EventCtx, id: osm::NodeID, point: Pt2D) {
        self.world.delete_before_replacement(ID::Intersection(id));
        for r in self.map.streets.move_intersection(id, point).unwrap() {
            self.road_deleted(r);
            self.road_added(ctx, r);
        }
        self.intersection_added(ctx, id);
    }

    pub fn delete_i(&mut self, id: osm::NodeID) {
        if !self.map.streets.can_delete_intersection(id) {
            error!("Can't delete intersection used by roads");
            return;
        }
        self.map.streets.delete_intersection(id);
        self.world.delete(ID::Intersection(id));
    }

    pub fn toggle_i(&mut self, ctx: &EventCtx, id: osm::NodeID) {
        self.world.delete_before_replacement(ID::Intersection(id));

        let i = self.map.streets.intersections.get_mut(&id).unwrap();
        if i.control == ControlType::TrafficSignal {
            i.control = ControlType::StopSign;
        } else if i.control == ControlType::StopSign {
            i.control = ControlType::TrafficSignal;
        }

        self.intersection_added(ctx, id);
    }

    pub fn show_intersection_geometry(&mut self, ctx: &mut EventCtx, show: bool) {
        self.intersection_geom = show;

        ctx.loading_screen("show intersection geometry", |ctx, timer| {
            if self.intersection_geom {
                self.map.streets.apply_transformations(
                    vec![Transformation::GenerateIntersectionGeometry],
                    timer,
                );
            }

            timer.start_iter(
                "intersection geometry",
                self.map.streets.intersections.len(),
            );
            for id in self
                .map
                .streets
                .intersections
                .keys()
                .cloned()
                .collect::<Vec<_>>()
            {
                timer.next();
                self.world.delete_before_replacement(ID::Intersection(id));
                self.intersection_added(ctx, id);
            }
        });
    }
}

// Roads
impl Model {
    pub fn road_added(&mut self, ctx: &EventCtx, id: OriginalRoad) {
        let road = &self.map.streets.roads[&id];
        let (center, total_width) = road.untrimmed_road_geometry();
        let hitbox = center.make_polygons(total_width);
        let mut draw = GeomBatch::new();
        draw.push(
            if road.osm_tags.is("junction", "intersection") {
                Color::PINK
            } else {
                Color::grey(0.8)
            },
            hitbox.clone(),
        );
        if let Some(outline) = center.to_thick_boundary(total_width, Distance::meters(1.0)) {
            draw.push(Color::BLACK, outline);
        }

        self.world
            .add(ID::Road(id))
            .hitbox(hitbox)
            .zorder(0)
            .draw(draw)
            .hover_alpha(0.5)
            .clickable()
            .hotkey(Key::Backspace, "delete")
            .hotkey(Key::P, "insert a new point here")
            .hotkey(Key::X, "remove interior points")
            .hotkey(Key::M, "merge")
            .hotkey(Key::J, "mark/unmark as a junction")
            .hotkey(Key::D, "debug in OSM")
            .build(ctx);
    }

    pub fn road_deleted(&mut self, id: OriginalRoad) {
        self.world.delete(ID::Road(id));
    }

    pub fn create_r(&mut self, ctx: &EventCtx, i1: osm::NodeID, i2: osm::NodeID) {
        // Ban cul-de-sacs, since they get stripped out later anyway.
        if self
            .map
            .streets
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

        let untrimmed_center_line = match PolyLine::new(vec![
            self.map.streets.intersections[&i1].point,
            self.map.streets.intersections[&i2].point,
        ]) {
            Ok(pl) => pl,
            Err(err) => {
                error!("Can't create road: {err}");
                return;
            }
        };
        let road = Road::new(
            id,
            untrimmed_center_line,
            osm_tags,
            &self.map.streets.config,
        );

        self.world.delete_before_replacement(ID::Intersection(i1));
        self.world.delete_before_replacement(ID::Intersection(i2));

        self.map.streets.insert_road(id, road);
        self.road_added(ctx, id);

        self.intersection_added(ctx, i1);
        self.intersection_added(ctx, i2);
    }

    pub fn delete_r(&mut self, ctx: &EventCtx, id: OriginalRoad) {
        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world
            .delete_before_replacement(ID::Intersection(id.i1));
        self.world
            .delete_before_replacement(ID::Intersection(id.i2));
        self.map.streets.remove_road(&id);

        self.intersection_added(ctx, id.i1);
        self.intersection_added(ctx, id.i2);
    }

    pub fn show_r_points(&mut self, ctx: &EventCtx, id: OriginalRoad) {
        if self.showing_pts == Some(id) {
            return;
        }
        if let Some(other) = self.showing_pts {
            self.stop_showing_pts(other);
        }
        self.showing_pts = Some(id);

        let r = &self.map.streets.roads[&id];
        for (idx, pt) in r.untrimmed_center_line.points().iter().enumerate() {
            // Don't show handles for the intersections
            if idx != 0 && idx != r.untrimmed_center_line.points().len() - 1 {
                self.world
                    .add(ID::RoadPoint(id, idx))
                    .hitbox(Circle::new(*pt, INTERSECTION_RADIUS / 2.0).to_polygon())
                    .zorder(3)
                    .draw_color(Color::GREEN)
                    .hover_alpha(0.5)
                    .draggable()
                    .hotkey(Key::Backspace, "delete")
                    .build(ctx);
            }
        }
    }

    pub fn stop_showing_pts(&mut self, id: OriginalRoad) {
        if self.showing_pts != Some(id) {
            return;
        }
        self.showing_pts = None;
        for idx in 1..=self.map.streets.roads[&id]
            .untrimmed_center_line
            .points()
            .len()
            - 2
        {
            self.world.delete(ID::RoadPoint(id, idx));
        }
    }

    pub fn move_r_pt(&mut self, ctx: &EventCtx, id: OriginalRoad, idx: usize, point: Pt2D) {
        assert_eq!(self.showing_pts, Some(id));
        // stop_showing_pts deletes the points, but we want to use delete_before_replacement
        self.showing_pts = None;
        for idx in 1..=self.map.streets.roads[&id]
            .untrimmed_center_line
            .points()
            .len()
            - 2
        {
            self.world.delete_before_replacement(ID::RoadPoint(id, idx));
        }

        self.road_deleted(id);
        self.world
            .delete_before_replacement(ID::Intersection(id.i1));
        self.world
            .delete_before_replacement(ID::Intersection(id.i2));

        let mut pts = self.map.streets.roads[&id]
            .untrimmed_center_line
            .clone()
            .into_points();
        pts[idx] = point;
        self.map
            .streets
            .roads
            .get_mut(&id)
            .unwrap()
            .untrimmed_center_line = PolyLine::must_new(pts);

        self.road_added(ctx, id);
        self.intersection_added(ctx, id.i1);
        self.intersection_added(ctx, id.i2);
        self.show_r_points(ctx, id);
    }

    fn change_r_points<F: FnMut(&mut Vec<Pt2D>)>(
        &mut self,
        ctx: &EventCtx,
        id: OriginalRoad,
        mut transform: F,
    ) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        self.world
            .delete_before_replacement(ID::Intersection(id.i1));
        self.world
            .delete_before_replacement(ID::Intersection(id.i2));

        let mut pts = self.map.streets.roads[&id]
            .untrimmed_center_line
            .clone()
            .into_points();
        transform(&mut pts);
        self.map
            .streets
            .roads
            .get_mut(&id)
            .unwrap()
            .untrimmed_center_line = PolyLine::must_new(pts);

        self.road_added(ctx, id);
        self.intersection_added(ctx, id.i1);
        self.intersection_added(ctx, id.i2);
        self.show_r_points(ctx, id);
    }

    pub fn delete_r_pt(&mut self, ctx: &EventCtx, id: OriginalRoad, idx: usize) {
        self.change_r_points(ctx, id, |pts| {
            pts.remove(idx);
        });
    }

    pub fn insert_r_pt(&mut self, ctx: &EventCtx, id: OriginalRoad, pt: Pt2D) {
        let mut closest = FindClosest::new(&self.compute_bounds());
        self.change_r_points(ctx, id, move |pts| {
            for (idx, pair) in pts.windows(2).enumerate() {
                closest.add(idx + 1, &[pair[0], pair[1]]);
            }
            if let Some((idx, _)) = closest.closest_pt(pt, Distance::meters(5.0)) {
                pts.insert(idx, pt);
            } else {
                warn!("Couldn't figure out where to insert new point");
            }
        });
    }

    pub fn clear_r_pts(&mut self, ctx: &EventCtx, id: OriginalRoad) {
        self.change_r_points(ctx, id, |pts| {
            *pts = vec![pts[0], *pts.last().unwrap()];
        });
    }

    pub fn merge_r(&mut self, ctx: &EventCtx, id: OriginalRoad) {
        self.stop_showing_pts(id);

        let (retained_i, deleted_i, deleted_roads, created_roads) =
            match self.map.streets.merge_short_road(id) {
                Ok((retained_i, deleted_i, deleted_roads, created_roads)) => {
                    (retained_i, deleted_i, deleted_roads, created_roads)
                }
                Err(err) => {
                    warn!("Can't merge this road: {}", err);
                    self.show_r_points(ctx, id);
                    return;
                }
            };

        self.world
            .delete_before_replacement(ID::Intersection(retained_i));
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

        let road = self.map.streets.roads.get_mut(&id).unwrap();
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
        self.world
            .add(ID::Building(id))
            .hitbox(b.polygon.clone())
            .zorder(2)
            .draw_color(Color::BLUE)
            .hover_alpha(0.5)
            .draggable()
            .hotkey(Key::Backspace, "delete")
            .build(ctx);
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

    pub fn move_b(&mut self, ctx: &EventCtx, id: osm::OsmID, dx: f64, dy: f64) {
        self.world.delete_before_replacement(ID::Building(id));

        let b = self.map.buildings.get_mut(&id).unwrap();
        b.polygon = b.polygon.translate(dx, dy);

        self.bldg_added(ctx, id);
    }

    pub fn delete_b(&mut self, id: osm::OsmID) {
        self.world.delete(ID::Building(id));
        self.map.buildings.remove(&id).unwrap();
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
    let mut f = fs_err::File::create("synthetic_export.osm")?;
    writeln!(f, r#"<?xml version='1.0' encoding='UTF-8'?>"#)?;
    writeln!(f, r#"<osm>"#)?;
    writeln!(
        f,
        r#"<!-- If you couldn't tell, this is a fake .osm file not representing the real world. -->"#
    )?;
    let b = &map.streets.gps_bounds;
    writeln!(
        f,
        r#"    <bounds minlon="{}" maxlon="{}" minlat="{}" maxlat="{}"/>"#,
        b.min_lon, b.max_lon, b.min_lat, b.max_lat
    )?;
    let mut pt_to_id: HashMap<HashablePt2D, osm::NodeID> = HashMap::new();
    for (id, i) in &map.streets.intersections {
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
    for (id, r) in &map.streets.roads {
        writeln!(f, r#"    <way id="{}">"#, id.osm_way_id.0)?;
        for pt in r.untrimmed_center_line.points() {
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
