use std::collections::HashMap;
use std::io::Write;

use abstio::{CityName, MapName};
use abstutil::{Tags, Timer};
use geom::{
    Bounds, Circle, Distance, FindClosest, GPSBounds, HashablePt2D, LonLat, PolyLine, Polygon, Pt2D,
};
use osm2streets::{
    osm, IntersectionControl, IntersectionID, IntersectionKind, OriginalRoad, Road, RoadID,
    Transformation,
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
    showing_pts: Option<RoadID>,
    pub world: World<ID>,

    pub include_bldgs: bool,
    pub intersection_geom: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ID {
    Building(osm::OsmID),
    Intersection(IntersectionID),
    Road(RoadID),
    RoadPoint(RoadID, usize),
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
    fn intersection_added(&mut self, ctx: &EventCtx, id: IntersectionID) {
        let i = &self.map.streets.intersections[&id];
        let color = if i.kind == IntersectionKind::MapEdge {
            Color::BLUE
        } else {
            match i.control {
                IntersectionControl::Signalled => Color::GREEN,
                IntersectionControl::Signed | IntersectionControl::Uncontrolled => Color::RED,
                IntersectionControl::Construction => Color::ORANGE,
            }
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
        let id = self.map.streets.insert_intersection(
            Vec::new(),
            point,
            // The kind will change as we connect things to this intersection
            IntersectionKind::Intersection,
            IntersectionControl::Signed,
        );
        self.intersection_added(ctx, id);
    }

    pub fn move_i(&mut self, ctx: &EventCtx, id: IntersectionID, point: Pt2D) {
        self.world.delete_before_replacement(ID::Intersection(id));

        self.map.streets.intersections.get_mut(&id).unwrap().point = point;

        // Update all the roads.
        let mut fixed = Vec::new();
        for r in &self.map.streets.intersections[&id].roads {
            fixed.push(*r);
            let road = self.map.streets.roads.get_mut(r).unwrap();
            let mut pts = road.untrimmed_center_line.clone().into_points();
            if road.src_i == id {
                pts[0] = point;
            } else {
                assert_eq!(road.dst_i, id);
                *pts.last_mut().unwrap() = point;
            }
            // TODO This could panic if someone moves the intersection a certain way
            road.untrimmed_center_line = PolyLine::must_new(pts);
        }

        for r in fixed {
            self.road_deleted(r);
            self.road_added(ctx, r);
        }
        self.intersection_added(ctx, id);
    }

    pub fn delete_i(&mut self, id: IntersectionID) {
        if !self.map.streets.intersections[&id].roads.is_empty() {
            error!("Can't delete intersection used by roads");
            return;
        }
        self.map.streets.remove_intersection(id);
        self.world.delete(ID::Intersection(id));
    }

    pub fn toggle_i(&mut self, ctx: &EventCtx, id: IntersectionID) {
        self.world.delete_before_replacement(ID::Intersection(id));

        let i = self.map.streets.intersections.get_mut(&id).unwrap();
        if i.control == IntersectionControl::Signalled {
            i.control = IntersectionControl::Signed;
        } else if i.control == IntersectionControl::Signed {
            i.control = IntersectionControl::Signalled;
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
    pub fn road_added(&mut self, ctx: &EventCtx, id: RoadID) {
        let road = &self.map.streets.roads[&id];
        let (center, total_width) = road.untrimmed_road_geometry();
        let hitbox = center.make_polygons(total_width);
        let mut draw = GeomBatch::new();
        draw.push(
            if road.internal_junction_road {
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

    pub fn road_deleted(&mut self, id: RoadID) {
        self.world.delete(ID::Road(id));
    }

    pub fn create_r(&mut self, ctx: &EventCtx, i1: IntersectionID, i2: IntersectionID) {
        // Ban cul-de-sacs, since they get stripped out later anyway.
        if self
            .map
            .streets
            .roads
            .values()
            .any(|r| (r.src_i == i1 && r.dst_i == i2) || (r.src_i == i2 && r.dst_i == i1))
        {
            error!("Road already exists");
            return;
        }

        let mut osm_tags = Tags::empty();
        osm_tags.insert("highway", "residential");
        osm_tags.insert("parking:both:lane", "parallel");
        osm_tags.insert("sidewalk", "both");
        osm_tags.insert("lanes", "2");
        // Reasonable defaults.
        osm_tags.insert("name", "Streety McStreetFace");
        osm_tags.insert("maxspeed", "25 mph");

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

        self.world.delete_before_replacement(ID::Intersection(i1));
        self.world.delete_before_replacement(ID::Intersection(i2));

        let id = self.map.streets.next_road_id();
        self.map.streets.insert_road(Road::new(
            id,
            // Dummy
            OriginalRoad::new(0, (1, 2)),
            i1,
            i2,
            untrimmed_center_line,
            osm_tags,
            &self.map.streets.config,
        ));
        self.road_added(ctx, id);

        self.intersection_added(ctx, i1);
        self.intersection_added(ctx, i2);
    }

    pub fn delete_r(&mut self, ctx: &EventCtx, id: RoadID) {
        self.stop_showing_pts(id);
        self.road_deleted(id);
        let road = self.map.streets.remove_road(id);
        self.world
            .delete_before_replacement(ID::Intersection(road.src_i));
        self.world
            .delete_before_replacement(ID::Intersection(road.dst_i));

        self.intersection_added(ctx, road.src_i);
        self.intersection_added(ctx, road.dst_i);
    }

    pub fn show_r_points(&mut self, ctx: &EventCtx, id: RoadID) {
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

    pub fn stop_showing_pts(&mut self, id: RoadID) {
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

    pub fn move_r_pt(&mut self, ctx: &EventCtx, id: RoadID, idx: usize, point: Pt2D) {
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
        let endpts = self.map.streets.roads[&id].endpoints();
        self.world
            .delete_before_replacement(ID::Intersection(endpts[0]));
        self.world
            .delete_before_replacement(ID::Intersection(endpts[1]));

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
        self.intersection_added(ctx, endpts[0]);
        self.intersection_added(ctx, endpts[1]);
        self.show_r_points(ctx, id);
    }

    fn change_r_points<F: FnMut(&mut Vec<Pt2D>)>(
        &mut self,
        ctx: &EventCtx,
        id: RoadID,
        mut transform: F,
    ) {
        assert_eq!(self.showing_pts, Some(id));

        self.stop_showing_pts(id);
        self.road_deleted(id);
        let endpts = self.map.streets.roads[&id].endpoints();
        self.world
            .delete_before_replacement(ID::Intersection(endpts[0]));
        self.world
            .delete_before_replacement(ID::Intersection(endpts[1]));

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
        self.intersection_added(ctx, endpts[0]);
        self.intersection_added(ctx, endpts[1]);
        self.show_r_points(ctx, id);
    }

    pub fn delete_r_pt(&mut self, ctx: &EventCtx, id: RoadID, idx: usize) {
        self.change_r_points(ctx, id, |pts| {
            pts.remove(idx);
        });
    }

    pub fn insert_r_pt(&mut self, ctx: &EventCtx, id: RoadID, pt: Pt2D) {
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

    pub fn clear_r_pts(&mut self, ctx: &EventCtx, id: RoadID) {
        self.change_r_points(ctx, id, |pts| {
            *pts = vec![pts[0], *pts.last().unwrap()];
        });
    }

    pub fn merge_r(&mut self, ctx: &EventCtx, id: RoadID) {
        self.stop_showing_pts(id);

        let (retained_i, deleted_i) = match self.map.streets.collapse_short_road(id) {
            Ok(pair) => pair,
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

        self.world.delete(ID::Road(id));
        // Geometry might've changed for roads connected to the endpoints
        for r in self.map.streets.intersections[&retained_i].roads.clone() {
            self.road_added(ctx, r);
        }

        info!("Merged {id}");
    }

    pub fn toggle_junction(&mut self, ctx: &EventCtx, id: RoadID) {
        self.road_deleted(id);

        let road = self.map.streets.roads.get_mut(&id).unwrap();
        road.internal_junction_road = !road.internal_junction_road;

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
        // Bit brittle, but not a big deal
        let id = osm::OsmID::Way(osm::WayID(-1 * self.map.buildings.len() as i64));
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
    let mut pt_to_id: HashMap<HashablePt2D, IntersectionID> = HashMap::new();
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
        writeln!(f, r#"    <way id="{}">"#, id.0)?;
        for pt in r.untrimmed_center_line.points() {
            // TODO Make new IDs if needed
            writeln!(
                f,
                r#"        <nd ref="{}"/>"#,
                pt_to_id[&pt.to_hashable()].0
            )?;
        }
        // TODO Brittle. Instead we should effectively do lanes2osm
        if let Some(tags) = map.road_to_osm_tags(*id) {
            for (k, v) in tags.inner() {
                writeln!(f, r#"        <tag k="{}" v="{}"/>"#, k, v)?;
            }
        }
        writeln!(f, r#"    </way>"#)?;
    }
    writeln!(f, r#"</osm>"#)?;
    Ok(())
}
