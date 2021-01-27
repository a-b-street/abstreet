//! Converts a SUMO .net.xml into an A/B Street map.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};

use abstio::MapName;
use abstutil::{CmdArgs, Tags, Timer};
use geom::{Distance, PolyLine};
use map_model::{
    osm, raw, AccessRestrictions, Intersection, IntersectionID, IntersectionType, Lane, LaneID,
    LaneType, Map, Road, RoadID, Turn, TurnID, TurnType,
};

use sumo::{Direction, InternalLaneID, Network, NodeID, VehicleClass};

fn main() -> Result<()> {
    let mut timer = Timer::new("convert SUMO network");
    let mut args = CmdArgs::new();
    let input = args.required_free();
    args.done();

    let network = Network::load(&input, &mut timer).unwrap();
    let map = convert(&input, network)?;
    map.save();
    Ok(())
}

fn convert(orig_path: &str, network: Network) -> Result<Map> {
    let mut intersections = Vec::new();
    let mut ids_intersections: BTreeMap<NodeID, IntersectionID> = BTreeMap::new();
    for (_, junction) in network.junctions {
        let id = IntersectionID(intersections.len());
        intersections.push(Intersection {
            id,
            polygon: junction.shape,
            turns: BTreeSet::new(),
            elevation: Distance::ZERO,
            intersection_type: IntersectionType::StopSign,
            // TODO Temporary ID. We could consider squeezing SUMO IDs into this scheme.
            orig_id: osm::NodeID(123),
            incoming_lanes: Vec::new(),
            outgoing_lanes: Vec::new(),
            roads: BTreeSet::new(),
            merged: false,
        });
        ids_intersections.insert(junction.id, id);
    }
    let mut roads: Vec<Road> = Vec::new();
    let mut lanes = Vec::new();
    let mut ids_lanes: BTreeMap<sumo::LaneID, LaneID> = BTreeMap::new();
    for (_, edge) in network.normal_edges {
        let src_i = ids_intersections[&edge.from];
        let dst_i = ids_intersections[&edge.to];
        // SUMO has one edge in each direction, but ABST has bidirectional roads. Detect if this
        // edge is the reverse of one we've already handled.
        let (road_id, direction) =
            if let Some(r) = roads.iter().find(|r| r.dst_i == src_i && r.src_i == dst_i) {
                (r.id, map_model::Direction::Back)
            } else {
                (RoadID(roads.len()), map_model::Direction::Fwd)
            };

        let mut lanes_rtl: Vec<(LaneID, map_model::Direction, LaneType)> = Vec::new();
        for lane in &edge.lanes {
            let lane_id = LaneID(lanes.len());
            ids_lanes.insert(lane.id.clone(), lane_id);
            let lane_type = if lane.allow == vec![VehicleClass::Pedestrian] {
                LaneType::Sidewalk
            } else if lane.allow == vec![VehicleClass::Bicycle] {
                LaneType::Biking
            } else if lane.allow == vec![VehicleClass::RailUrban] {
                LaneType::LightRail
            } else {
                LaneType::Driving
            };
            lanes.push(Lane {
                id: lane_id,
                parent: road_id,
                lane_type,
                lane_center_pts: lane.center_line.clone(),
                width: lane.width,

                src_i,
                dst_i,

                bus_stops: BTreeSet::new(),

                driving_blackhole: false,
                biking_blackhole: false,
            });
            // These seem to appear in the XML from right to left
            lanes_rtl.push((lane_id, direction, lane_type));
        }

        if direction == map_model::Direction::Fwd {
            // Make a new road
            intersections[src_i.0].roads.insert(road_id);
            intersections[dst_i.0].roads.insert(road_id);
            let speed_limit = edge.lanes[0].speed;

            let mut tags = BTreeMap::new();
            tags.insert("id".to_string(), edge.id.0.clone());
            if let Some(name) = &edge.name {
                tags.insert("name".to_string(), name.clone());
            }
            let parts: Vec<&str> = edge.edge_type.split(".").collect();
            // "highway.footway"
            if parts.len() != 2 {
                bail!("Unknown edge_type {}", edge.edge_type);
            }
            tags.insert(parts[0].to_string(), parts[1].to_string());
            let mut lanes_ltr = lanes_rtl;
            lanes_ltr.reverse();

            roads.push(Road {
                id: road_id,
                osm_tags: Tags::new(tags),
                turn_restrictions: Vec::new(),
                complicated_turn_restrictions: Vec::new(),
                // TODO Temporary ID. We could consider squeezing SUMO IDs into this scheme.
                orig_id: raw::OriginalRoad::new(123, (456, 789)),
                speed_limit,
                access_restrictions: AccessRestrictions::new(),
                zorder: 0,

                lanes_ltr,

                center_pts: edge.center_line,

                src_i,
                dst_i,
            });
        } else {
            // There's an existing road with the forward direction lanes. We're currently
            // processing the reverse direction of that road, so lanes_rtl oriented in the forwards
            // direction is already left-to-right.
            let mut lanes_ltr = lanes_rtl;
            lanes_ltr.extend(roads[road_id.0].lanes_ltr.clone());
            // TODO Should we check that the attributes are the same for both directions?
            roads[road_id.0].lanes_ltr = lanes_ltr;
        }
    }

    let mut internal_lane_geometry: BTreeMap<InternalLaneID, PolyLine> = BTreeMap::new();
    for (_, edge) in network.internal_edges {
        for lane in edge.lanes {
            if let Some(pl) = lane.center_line {
                internal_lane_geometry.insert(lane.id, pl);
            }
        }
    }

    let mut turns = Vec::new();
    for connection in network.connections {
        match (
            ids_lanes.get(&connection.from_lane()),
            ids_lanes.get(&connection.to_lane()),
            connection.via,
        ) {
            (Some(from), Some(to), Some(via)) => {
                let id = TurnID {
                    parent: lanes[from.0].dst_i,
                    src: *from,
                    dst: *to,
                };
                if let Some(geom) = internal_lane_geometry.remove(&via).or_else(|| {
                    PolyLine::new(vec![
                        lanes[from.0].lane_center_pts.last_pt(),
                        lanes[to.0].lane_center_pts.first_pt(),
                    ])
                    .ok()
                }) {
                    turns.push(Turn {
                        id,
                        // TODO Crosswalks and sidewalk corners
                        turn_type: match connection.dir {
                            Direction::Straight => TurnType::Straight,
                            Direction::Left | Direction::PartiallyLeft => TurnType::Left,
                            Direction::Right | Direction::PartiallyRight => TurnType::Right,
                            // Not sure
                            _ => TurnType::Straight,
                        },
                        geom,
                        other_crosswalk_ids: BTreeSet::new(),
                    });
                    intersections[id.parent.0].turns.insert(id);
                }
            }
            _ => {}
        }
    }

    Ok(Map::import_minimal(
        // Double basename because "foo.net.xml" just becomes "foo.net"
        MapName::new("sumo", &abstutil::basename(abstutil::basename(orig_path))),
        network.location.converted_boundary,
        network.location.orig_boundary,
        intersections,
        roads,
        lanes,
        turns,
    ))
}
