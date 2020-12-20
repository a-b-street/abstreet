use std::collections::{HashMap, HashSet};

use abstutil::MultiMap;
use geom::{Angle, Circle, Distance, PolyLine, Pt2D, Speed};
use map_gui::ID;
use map_model::{BuildingID, Direction, IntersectionID, LaneType, RoadID};
use widgetry::EventCtx;

use crate::controls::InstantController;
use crate::App;

const ZOOM: f64 = 10.0;

pub struct Player {
    pos: Pt2D,
    facing: Angle,
    on: On,
    bldgs_along_road: BuildingsAlongRoad,

    controls: InstantController,
}

impl Player {
    pub fn new(ctx: &mut EventCtx, app: &App, start: IntersectionID) -> Player {
        ctx.canvas.cam_zoom = ZOOM;
        let pos = app.map.get_i(start).polygon.center();
        ctx.canvas.center_on_map_pt(pos);

        Player {
            pos,
            facing: Angle::ZERO,
            on: On::Intersection(start),
            bldgs_along_road: BuildingsAlongRoad::new(app),

            controls: InstantController::new(),
        }
    }

    /// Returns any buildings we passed
    pub fn update_with_speed(
        &mut self,
        ctx: &mut EventCtx,
        app: &App,
        speed: Speed,
    ) -> Vec<BuildingID> {
        if let Some((dx, dy)) = self.controls.displacement(ctx, speed) {
            self.apply_displacement(ctx, app, dx, dy, true)
        // TODO Do the center_on_map_pt here, actually
        } else {
            Vec::new()
        }
    }

    fn pos_to_on(&self, app: &App, pos: Pt2D) -> Option<On> {
        // Make sure we only move between roads/intersections that're actually connected. Don't
        // warp to bridges/tunnels.
        let (valid_roads, valid_intersections) = self.on.get_connections(app);

        // Make sure we're still on the road
        for id in app
            .draw_map
            .get_matching_objects(Circle::new(pos, Distance::meters(3.0)).get_bounds())
        {
            if let ID::Intersection(i) = id {
                if valid_intersections.contains(&i) && app.map.get_i(i).polygon.contains_pt(pos) {
                    return Some(On::Intersection(i));
                }
            } else if let ID::Road(r) = id {
                let road = app.map.get_r(r);
                if valid_roads.contains(&r)
                    && !road.is_light_rail()
                    && road.get_thick_polygon(&app.map).contains_pt(pos)
                {
                    // Where along the road are we?
                    let pt_on_center_line = road.center_pts.project_pt(pos);
                    if let Some((dist, _)) = road.center_pts.dist_along_of_point(pt_on_center_line)
                    {
                        // We'll adjust the direction at the call-site if we're moving along the
                        // same road. This heuristic is reasonable for moving from intersections to
                        // roads.
                        let dir = if dist < road.center_pts.length() / 2.0 {
                            Direction::Fwd
                        } else {
                            Direction::Back
                        };
                        return Some(On::Road(r, dist, dir));
                    } else {
                        error!(
                            "{} snapped to {} on {}, but dist_along_of_point failed",
                            pos, pt_on_center_line, r
                        );
                        return None;
                    }
                }
            }
        }
        None
    }

    fn apply_displacement(
        &mut self,
        ctx: &mut EventCtx,
        app: &App,
        dx: f64,
        dy: f64,
        recurse: bool,
    ) -> Vec<BuildingID> {
        let new_pos = self.pos.offset(dx, dy);
        let mut buildings_passed = Vec::new();
        if let Some(mut new_on) = self.pos_to_on(app, new_pos) {
            self.pos = new_pos;
            ctx.canvas.center_on_map_pt(self.pos);

            if let (On::Road(r1, dist1, _), On::Road(r2, dist2, _)) =
                (self.on.clone(), new_on.clone())
            {
                if r1 == r2 {
                    // Find all buildings in this range of distance along
                    buildings_passed.extend(self.bldgs_along_road.query_range(r1, dist1, dist2));
                    if dist1 < dist2 {
                        new_on = On::Road(r2, dist2, Direction::Fwd);
                    } else {
                        new_on = On::Road(r2, dist2, Direction::Back);
                    }
                }
            }
            self.on = new_on;
        } else {
            // We went out of bounds. Undo this movement.

            // Apply horizontal and vertical movement independently, so we "slide" along boundaries
            // if possible
            if recurse {
                let orig = self.pos;
                if dx != 0.0 {
                    buildings_passed.extend(self.apply_displacement(ctx, app, dx, 0.0, false));
                }
                if dy != 0.0 {
                    buildings_passed.extend(self.apply_displacement(ctx, app, 0.0, dy, false));
                }

                // Are we stuck?
                if self.pos == orig {
                    if true {
                        // Resolve by just bouncing in the opposite direction. Jittery, but we keep
                        // moving.
                        buildings_passed.extend(self.apply_displacement(ctx, app, -dx, -dy, false));
                    } else {
                        // Find the exact point on the boundary where we go out of bounds
                        let old_ring = match self.on {
                            On::Intersection(i) => app.map.get_i(i).polygon.clone().into_ring(),
                            On::Road(r, _, _) => {
                                let road = app.map.get_r(r);
                                road.center_pts
                                    .to_thick_ring(2.0 * road.get_half_width(&app.map))
                            }
                        };
                        // TODO Brittle order, but should be the first from the PolyLine's
                        // perspective
                        if let Some(pt) = old_ring
                            .all_intersections(&PolyLine::must_new(vec![self.pos, new_pos]))
                            .get(0)
                        {
                            buildings_passed.extend(self.apply_displacement(
                                ctx,
                                app,
                                pt.x() - self.pos.x(),
                                pt.y() - self.pos.y(),
                                false,
                            ));
                        }
                    }
                }
            }
        }

        // Snap to the center of the road
        if let On::Road(r, dist, dir) = self.on {
            let (pt, angle) = app.map.get_r(r).center_pts.must_dist_along(dist);
            self.pos = pt;
            self.facing = if dir == Direction::Fwd {
                angle.opposite()
            } else {
                angle
            };
            ctx.canvas.center_on_map_pt(self.pos);
        } else {
            self.facing = self.controls.facing;
        }

        buildings_passed
    }

    pub fn get_pos(&self) -> Pt2D {
        self.pos
    }

    pub fn get_angle(&self) -> Angle {
        self.facing
    }

    /// Is the player currently on a road with a bus or bike lane?
    pub fn on_good_road(&self, app: &App) -> bool {
        let roads = match self.on {
            On::Road(r, _, _) => vec![r],
            On::Intersection(i) => app.map.get_i(i).roads.iter().cloned().collect(),
        };
        for r in roads {
            for (_, _, lt) in app.map.get_r(r).lanes_ltr() {
                if lt == LaneType::Biking || lt == LaneType::Bus {
                    return true;
                }
            }
        }
        false
    }

    /// For the game over animation
    pub fn override_pos(&mut self, pos: Pt2D) {
        self.pos = pos;
    }
}

#[derive(Clone, PartialEq)]
enum On {
    Intersection(IntersectionID),
    // Distance along the center line, are we facing the same direction as the road
    Road(RoadID, Distance, Direction),
}

impl On {
    fn get_connections(&self, app: &App) -> (HashSet<RoadID>, HashSet<IntersectionID>) {
        let mut valid_roads = HashSet::new();
        let mut valid_intersections = HashSet::new();
        match self {
            On::Road(r, _, _) => {
                let r = app.map.get_r(*r);
                valid_intersections.insert(r.src_i);
                valid_intersections.insert(r.dst_i);
                // Intersections might be pretty small
                valid_roads.extend(app.map.get_i(r.src_i).roads.clone());
                valid_roads.extend(app.map.get_i(r.dst_i).roads.clone());
            }
            On::Intersection(i) => {
                let i = app.map.get_i(*i);
                for r in &i.roads {
                    valid_roads.insert(*r);
                    // Roads can be small
                    let r = app.map.get_r(*r);
                    valid_intersections.insert(r.src_i);
                    valid_intersections.insert(r.dst_i);
                }
            }
        }
        (valid_roads, valid_intersections)
    }
}

struct BuildingsAlongRoad {
    // For each road, all of the buildings along it. Ascending distance, with the distance matching
    // the road's center points.
    per_road: HashMap<RoadID, Vec<(Distance, BuildingID)>>,
}

impl BuildingsAlongRoad {
    fn new(app: &App) -> BuildingsAlongRoad {
        let mut raw: MultiMap<RoadID, (Distance, BuildingID)> = MultiMap::new();
        for b in app.map.all_buildings() {
            // TODO Happily assuming road and lane length is roughly the same
            let road = app.map.get_parent(b.sidewalk_pos.lane());
            let dist = match road.dir(b.sidewalk_pos.lane()) {
                Direction::Fwd => b.sidewalk_pos.dist_along(),
                Direction::Back => road.center_pts.length() - b.sidewalk_pos.dist_along(),
            };
            raw.insert(road.id, (dist, b.id));
        }

        let mut per_road = HashMap::new();
        for (road, list) in raw.consume() {
            // BTreeSet will sort by the distance
            per_road.insert(road, list.into_iter().collect());
        }

        BuildingsAlongRoad { per_road }
    }

    fn query_range(&self, road: RoadID, dist1: Distance, dist2: Distance) -> Vec<BuildingID> {
        if dist1 > dist2 {
            return self.query_range(road, dist2, dist1);
        }

        let mut results = Vec::new();
        if let Some(list) = self.per_road.get(&road) {
            // TODO Binary search to find start?
            for (dist, b) in list {
                if *dist >= dist1 && *dist <= dist2 {
                    results.push(*b);
                }
            }
        }
        results
    }
}
