use std::collections::{BTreeSet, HashSet};

use abstutil::{Counter, Timer};
use map_gui::tools::ColorNetwork;
use map_model::{
    DirectedRoadID, IntersectionID, LaneID, Map, PathConstraints, PathRequest, PathStepV2, PathV2,
    Pathfinder, Position, RoadID,
};
use widgetry::GeomBatch;

use crate::{App, Cell, Edits, Neighbourhood};

pub struct Shortcuts {
    pub paths: Vec<PathV2>,
    pub count_per_road: Counter<RoadID>,
    pub count_per_intersection: Counter<IntersectionID>,
}

impl Shortcuts {
    // For temporary use
    pub fn empty() -> Self {
        Self {
            paths: Vec::new(),
            count_per_road: Counter::new(),
            count_per_intersection: Counter::new(),
        }
    }

    pub fn new(map: &Map, edits: &Edits, neighbourhood: &Neighbourhood, timer: &mut Timer) -> Self {
        // The overall approach: look for all possible paths from an entrance to an exit, only if they
        // connect to different major roads.
        //
        // But an entrance and exit to _what_? If we try to route from the entrance to one cell to the
        // exit of another, then the route will make strange U-turns and probably use the perimeter. By
        // definition, two cells aren't reachable without using the perimeter. So restrict our search
        // to pairs of entrances/exits in the _same_ cell.
        let mut requests = Vec::new();

        for cell in &neighbourhood.cells {
            let entrances = find_entrances_or_exits(map, neighbourhood, cell, true);
            let exits = find_entrances_or_exits(map, neighbourhood, cell, false);

            for entrance in &entrances {
                for exit in &exits {
                    // Most of the time, an entrance/exit connects to only "one" major road. But where
                    // a road name changes, or two major roads meet, we'll have multiple. If two
                    // corners meet and share one main road, still consider that a shortcut -- it might
                    // be a "false positive" or there could be some legitimate reason for a driver to
                    // attempt the shortcut.
                    if entrance.major_road_names != exit.major_road_names {
                        requests.push(PathRequest::vehicle(
                            Position::start(entrance.lane),
                            Position::end(exit.lane, map),
                            PathConstraints::Car,
                        ));
                    }
                }
            }
        }

        // Short-circuit for performance. This happens for "degenerate" neighbourhoods without any
        // internal roads, usually near the map edge, between dual carriageways, etc.
        if requests.is_empty() {
            return Self::empty();
        }

        let mut params = map.routing_params().clone();
        edits.update_routing_params(&mut params);

        // Restrict the pathfinding to the interior of the neighbourhood only. Don't allow using
        // perimeter roads or leaving and re-entering at all.
        //
        // The point of this view is to show possible detours people might try to take in response to
        // one filter. Note the original "demand model" input is bogus anyway; it's all possible
        // entrances and exits to the neighbourhood, without regards for the larger path somebody
        // actually wants to take.
        params.only_use_roads = neighbourhood.interior_roads.clone();

        // Also can't use private roads
        for r in &neighbourhood.interior_roads {
            if !crate::is_driveable(map.get_r(*r), map) {
                params.avoid_roads.insert(*r);
            }
        }

        // TODO Perf: when would it be worth creating a CH? Especially if we could subset just this
        // part of the graph, it'd probably be helpful.
        let pathfinder = Pathfinder::new_dijkstra(map, params, vec![PathConstraints::Car], timer);
        let paths: Vec<PathV2> = timer
            .parallelize(
                "calculate paths between entrances and exits",
                requests,
                |req| pathfinder.pathfind_v2(req, map),
            )
            .into_iter()
            .flatten()
            .collect();

        // TODO Rank the likeliness of each shortcut by
        // 1) Calculating a path between similar start/endpoints -- travelling along the perimeter,
        //    starting and ending on a specific road that makes sense. (We have to pick the 'direction'
        //    along the perimeter roads that's sensible.)
        // 2) Comparing that time to the time for cutting through

        Shortcuts::from_paths(neighbourhood, paths)
    }

    pub fn from_paths(neighbourhood: &Neighbourhood, paths: Vec<PathV2>) -> Self {
        // How many shortcuts pass through each street?
        let mut count_per_road = Counter::new();
        let mut count_per_intersection = Counter::new();
        for path in &paths {
            for step in path.get_steps() {
                match step {
                    PathStepV2::Along(dr) => {
                        if neighbourhood.interior_roads.contains(&dr.road) {
                            count_per_road.inc(dr.road);
                        }
                    }
                    PathStepV2::Movement(m) => {
                        if neighbourhood.interior_intersections.contains(&m.parent) {
                            count_per_intersection.inc(m.parent);
                        }
                    }
                    // Car paths don't make contraflow movements
                    _ => unreachable!(),
                }
            }
        }

        Self {
            paths,
            count_per_road,
            count_per_intersection,
        }
    }

    pub fn quiet_and_total_streets(&self, neighbourhood: &Neighbourhood) -> (usize, usize) {
        let quiet_streets = neighbourhood
            .interior_roads
            .iter()
            .filter(|r| self.count_per_road.get(**r) == 0)
            .count();
        let total_streets = neighbourhood.interior_roads.len();
        (quiet_streets, total_streets)
    }

    pub fn subset(&self, neighbourhood: &Neighbourhood, r: RoadID) -> Self {
        let paths = self
            .paths
            .iter()
            .filter(|path| path.crosses_road(r))
            .cloned()
            .collect();
        Self::from_paths(neighbourhood, paths)
    }

    pub fn draw_heatmap(&self, app: &App) -> GeomBatch {
        let mut colorer = ColorNetwork::no_fading(app);
        colorer.ranked_roads(self.count_per_road.clone(), &app.cs.good_to_bad_red);
        // TODO These two will be on different scales, which may look weird
        colorer.ranked_intersections(self.count_per_intersection.clone(), &app.cs.good_to_bad_red);
        colorer.draw.unzoomed
    }
}

struct EntryExit {
    // Really this is a DirectedRoadID, but since the pathfinding request later needs to know
    // lanes, just use this
    lane: LaneID,
    major_road_names: BTreeSet<String>,
}

fn find_entrances_or_exits(
    map: &Map,
    neighbourhood: &Neighbourhood,
    cell: &Cell,
    entrances: bool,
) -> Vec<EntryExit> {
    let mut entry_exits = Vec::new();
    for i in &cell.borders {
        let major_road_names = find_major_road_names(map, neighbourhood, *i);
        let mut seen: HashSet<DirectedRoadID> = HashSet::new();
        let lanes = if entrances {
            map.get_i(*i).get_outgoing_lanes(map, PathConstraints::Car)
        } else {
            map.get_i(*i).get_incoming_lanes(map, PathConstraints::Car)
        };
        for l in lanes {
            let dr = map.get_l(l).get_directed_parent();
            if !seen.contains(&dr) && cell.roads.contains_key(&dr.road) {
                entry_exits.push(EntryExit {
                    lane: l,
                    major_road_names: major_road_names.clone(),
                });
                seen.insert(dr);
            }
        }
    }
    entry_exits
}

fn find_major_road_names(
    map: &Map,
    neighbourhood: &Neighbourhood,
    i: IntersectionID,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for r in &map.get_i(i).roads {
        if !neighbourhood.interior_roads.contains(r) {
            names.insert(map.get_r(*r).get_name(None));
        }
    }
    names
}
