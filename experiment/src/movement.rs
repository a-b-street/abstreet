use std::collections::HashMap;

use abstutil::MultiMap;
use geom::{Angle, Circle, Distance, Pt2D, Speed};
use map_gui::{SimpleApp, ID};
use map_model::{BuildingID, Direction, IntersectionID, RoadID};
use widgetry::EventCtx;

use crate::controls::InstantController;

pub const ZOOM: f64 = 10.0;

pub struct Player {
    pos: Pt2D,
    on: On,
    bldgs_along_road: BuildingsAlongRoad,

    controls: InstantController,
}

impl Player {
    pub fn new(ctx: &mut EventCtx, app: &SimpleApp, start: IntersectionID) -> Player {
        let pos = app.map.get_i(start).polygon.center();
        ctx.canvas.center_on_map_pt(pos);
        ctx.canvas.cam_zoom = ZOOM;

        Player {
            pos,
            on: On::Intersection(start),
            bldgs_along_road: BuildingsAlongRoad::new(app),

            controls: InstantController::new(),
        }
    }

    /// Returns any buildings we passed
    pub fn update_with_speed(
        &mut self,
        ctx: &mut EventCtx,
        app: &SimpleApp,
        speed: Speed,
    ) -> Vec<BuildingID> {
        let (dx, dy) = self.controls.displacement(ctx, speed);
        if dx != 0.0 || dy != 0.0 {
            self.apply_displacement(ctx, app, dx, dy, true)
        // TODO Do the center_on_map_pt here, actually
        } else {
            Vec::new()
        }
    }

    fn apply_displacement(
        &mut self,
        ctx: &mut EventCtx,
        app: &SimpleApp,
        dx: f64,
        dy: f64,
        recurse: bool,
    ) -> Vec<BuildingID> {
        let new_pos = self.pos.offset(dx, dy);

        // Make sure we're still on the road
        let mut on = None;
        for id in app
            .draw_map
            .get_matching_objects(Circle::new(self.pos, Distance::meters(3.0)).get_bounds())
        {
            if let ID::Intersection(i) = id {
                if app.map.get_i(i).polygon.contains_pt(new_pos) {
                    on = Some(On::Intersection(i));
                    break;
                }
            } else if let ID::Road(r) = id {
                let road = app.map.get_r(r);
                if road.get_thick_polygon(&app.map).contains_pt(new_pos) {
                    // Where along the road are we?
                    let pt_on_center_line = road.center_pts.project_pt(new_pos);
                    if let Some((dist, _)) = road.center_pts.dist_along_of_point(pt_on_center_line)
                    {
                        on = Some(On::Road(r, dist));
                    } else {
                        error!(
                            "{} snapped to {} on {}, but dist_along_of_point failed",
                            new_pos, pt_on_center_line, r
                        );
                    }
                    break;
                }
            }
        }

        let mut buildings_passed = Vec::new();
        if let Some(new_on) = on {
            self.pos = new_pos;
            ctx.canvas.center_on_map_pt(self.pos);

            if let (On::Road(r1, dist1), On::Road(r2, dist2)) = (self.on.clone(), new_on.clone()) {
                if r1 == r2 {
                    // Find all buildings in this range of distance along
                    buildings_passed.extend(self.bldgs_along_road.query_range(r1, dist1, dist2));
                }
            }
            self.on = new_on;
        } else {
            // We went out of bounds. Undo this movement.
            // TODO Draw a line between the old and new position, and snap to the boundary of
            // whatever we hit.

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

                // If we're stuck, try bouncing in the opposite direction.
                // TODO This is jittery and we can sometimes go out of bounds now. :D
                if self.pos == orig {
                    buildings_passed.extend(self.apply_displacement(ctx, app, -dx, -dy, false));
                }
            }
        }
        buildings_passed
    }

    pub fn get_pos(&self) -> Pt2D {
        self.pos
    }

    pub fn get_angle(&self) -> Angle {
        self.controls.facing
    }
}

#[derive(Clone)]
enum On {
    Intersection(IntersectionID),
    // Distance along the center line
    Road(RoadID, Distance),
}

struct BuildingsAlongRoad {
    // For each road, all of the buildings along it. Ascending distance, with the distance matching
    // the road's center points.
    per_road: HashMap<RoadID, Vec<(Distance, BuildingID)>>,
}

impl BuildingsAlongRoad {
    fn new(app: &SimpleApp) -> BuildingsAlongRoad {
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
