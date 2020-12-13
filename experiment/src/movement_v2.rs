use std::collections::HashMap;

use abstutil::{wraparound_get, MultiMap};
use geom::{Angle, Distance, PolyLine, Pt2D, Speed};
use map_model::{
    BuildingID, DirectedRoadID, Direction, IntersectionID, LaneType, Map, RoadID,
    NORMAL_LANE_THICKNESS,
};
use widgetry::{Color, EventCtx, GfxCtx, Key};

use crate::App;

pub const ZOOM: f64 = 10.0;
// TODO The timestep accumulation seems fine. What's wrong? Clamping errors repeated?
const HACK: f64 = 5.0;

pub struct Player {
    on: On,
    pos: Pt2D,
    angle: Angle,
    bldgs_along_road: BuildingsAlongRoad,
}

impl Player {
    pub fn new(ctx: &mut EventCtx, app: &App, start: IntersectionID) -> Player {
        ctx.canvas.cam_zoom = ZOOM;
        let dr = all_connections(start, &app.map)[0].0;
        let on = On::Road {
            dr,
            dist: Distance::ZERO,
            next_road: default_connection(dr, &app.map),
        };
        let (pos, angle) = on.get_pos(app);
        ctx.canvas.center_on_map_pt(pos);

        Player {
            on,
            pos,
            angle,
            bldgs_along_road: BuildingsAlongRoad::new(app),
        }
    }

    /// Returns any buildings we passed
    pub fn update_with_speed(
        &mut self,
        ctx: &mut EventCtx,
        app: &App,
        speed: Speed,
    ) -> Vec<BuildingID> {
        let mut buildings_passed = Vec::new();

        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            let travelled = HACK * dt * speed;
            let pl = self.on.pl(app);
            match self.on {
                On::Road {
                    dr,
                    ref mut dist,
                    next_road,
                } => {
                    if *dist + travelled > pl.length() {
                        if dr_pl(dr, &app.map).last_pt() == dr_pl(next_road, &app.map).first_pt() {
                            // 0-len turn, skip straight to the road.
                            self.on = On::Road {
                                dr: next_road,
                                dist: Distance::ZERO,
                                next_road: default_connection(next_road, &app.map),
                            };
                        } else {
                            self.on = On::Intersection {
                                from: dr,
                                to: next_road,
                                // TODO Carryover distance... and jump multiple steps in one
                                // timestep?
                                dist: Distance::ZERO,
                            };
                        }
                    } else {
                        let mut dist1 = *dist;
                        *dist += travelled;
                        let mut dist2 = *dist;
                        if dr.dir == Direction::Back {
                            dist1 = pl.length() - dist1;
                            dist2 = pl.length() - dist2;
                        }
                        // Find all buildings in this range of distance along
                        buildings_passed
                            .extend(self.bldgs_along_road.query_range(dr.id, dist1, dist2));
                    }
                }
                On::Intersection {
                    to, ref mut dist, ..
                } => {
                    if *dist + travelled > pl.length() {
                        self.on = On::Road {
                            dr: to,
                            dist: Distance::ZERO,
                            next_road: default_connection(to, &app.map),
                        };
                    } else {
                        *dist += travelled;
                    }
                }
            }

            let (pos, angle) = self.on.get_pos(app);
            self.pos = pos;
            self.angle = angle;
            ctx.canvas.center_on_map_pt(pos);
        }
        buildings_passed
    }

    pub fn event(&mut self, ctx: &mut EventCtx, app: &App) {
        if ctx.input.pressed(Key::LeftArrow) {
            if let On::Road {
                dr,
                ref mut next_road,
                ..
            } = self.on
            {
                let all = all_connections(dr.dst_i(&app.map), &app.map);
                let idx = all.iter().position(|(x, _)| x == next_road).unwrap() as isize;
                *next_road = wraparound_get(&all, idx - 1).0;
            }
        } else if ctx.input.pressed(Key::RightArrow) {
            if let On::Road {
                dr,
                ref mut next_road,
                ..
            } = self.on
            {
                let all = all_connections(dr.dst_i(&app.map), &app.map);
                let idx = all.iter().position(|(x, _)| x == next_road).unwrap() as isize;
                *next_road = wraparound_get(&all, idx + 1).0;
            }
        }
    }

    pub fn get_pos(&self) -> Pt2D {
        self.pos
    }

    pub fn get_angle(&self) -> Angle {
        // Woohoo y inversion and player sprites facing the wrong way!
        self.angle.opposite()
    }

    /// Is the player currently on a road with a bus or bike lane?
    pub fn on_good_road(&self, app: &App) -> bool {
        if let On::Road { dr, .. } = self.on {
            for (_, _, lt) in app.map.get_r(dr.id).lanes_ltr() {
                if lt == LaneType::Biking || lt == LaneType::Bus {
                    return true;
                }
            }
        }
        false
    }

    pub fn draw_next_step(&self, g: &mut GfxCtx, app: &App) {
        if let On::Road { next_road, .. } = self.on {
            g.draw_polygon(
                Color::YELLOW.alpha(0.5),
                dr_pl(next_road, &app.map).make_polygons(NORMAL_LANE_THICKNESS),
            );
        }
    }
}

#[derive(Clone, PartialEq)]
enum On {
    Road {
        dr: DirectedRoadID,
        // Inverted when going backwards -- aka, it always starts at zero and goes up
        dist: Distance,
        next_road: DirectedRoadID,
    },
    Intersection {
        from: DirectedRoadID,
        to: DirectedRoadID,
        dist: Distance,
    },
}

impl On {
    fn pl(&self, app: &App) -> PolyLine {
        match self {
            On::Road { dr, .. } => dr_pl(*dr, &app.map),
            On::Intersection { from, to, .. } => PolyLine::must_new(vec![
                dr_pl(*from, &app.map).last_pt(),
                dr_pl(*to, &app.map).first_pt(),
            ]),
        }
    }

    fn get_pos(&self, app: &App) -> (Pt2D, Angle) {
        // TODO Dedupe code
        match self {
            On::Road { dr, dist, .. } => dr_pl(*dr, &app.map).must_dist_along(*dist),
            On::Intersection { from, to, dist } => PolyLine::must_new(vec![
                dr_pl(*from, &app.map).last_pt(),
                dr_pl(*to, &app.map).first_pt(),
            ])
            .must_dist_along(*dist),
        }
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

/// All roads connected to this intersection, along with a direction assuming we're starting from
/// the intersection. Ordered by angle.
fn all_connections(i: IntersectionID, map: &Map) -> Vec<(DirectedRoadID, Angle)> {
    let mut all = map
        .get_i(i)
        .roads
        .iter()
        .map(|r| {
            let r = map.get_r(*r);
            let dir;
            let angle;
            if r.src_i == i {
                dir = Direction::Fwd;
                angle = r.center_pts.first_line().angle();
            } else {
                dir = Direction::Back;
                angle = r.center_pts.last_line().angle().opposite();
            }
            (DirectedRoadID { id: r.id, dir }, angle)
        })
        .collect::<Vec<_>>();
    all.sort_by_key(|(_, angle)| angle.normalized_degrees() as i64);
    all
}

fn default_connection(from: DirectedRoadID, map: &Map) -> DirectedRoadID {
    let outgoing_angle = if from.dir == Direction::Fwd {
        map.get_r(from.id).center_pts.last_line().angle()
    } else {
        map.get_r(from.id)
            .center_pts
            .first_line()
            .angle()
            .opposite()
    };

    // Do the thing closest to "go straight"
    all_connections(from.dst_i(map), map)
        .into_iter()
        .min_by_key(|(_, angle)| {
            outgoing_angle
                .simple_shortest_rotation_towards(*angle)
                .abs() as usize
        })
        .unwrap()
        .0
}

fn dr_pl(dr: DirectedRoadID, map: &Map) -> PolyLine {
    let r = map.get_r(dr.id);
    match dr.dir {
        Direction::Fwd => r.center_pts.clone(),
        Direction::Back => r.center_pts.reversed(),
    }
}
