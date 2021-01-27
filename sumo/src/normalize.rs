//! Transforms a `raw::Network` into a `Network` that's easier to reason about.

use std::collections::BTreeMap;

use anyhow::Result;

use abstutil::Timer;
use geom::{Distance, PolyLine, Pt2D, Ring};

use crate::{
    raw, Edge, InternalEdge, InternalLane, InternalLaneID, Junction, Lane, LaneID, Network,
};

impl Network {
    /// Reads a .net.xml file and return the normalized SUMO network.
    pub fn load(path: &str, timer: &mut Timer) -> Result<Network> {
        let raw = raw::Network::parse(path, timer)?;
        timer.start("normalize");
        let network = Network::from_raw(raw);
        timer.stop("normalize");
        Ok(network)
    }

    fn from_raw(raw: raw::Network) -> Network {
        let mut network = Network {
            location: raw.location,
            normal_edges: BTreeMap::new(),
            internal_edges: BTreeMap::new(),
            junctions: BTreeMap::new(),
            connections: raw.connections,
        };

        let types: BTreeMap<String, raw::Type> =
            raw.types.into_iter().map(|t| (t.id.clone(), t)).collect();

        for junction in raw.junctions {
            if junction.junction_type == "internal" {
                continue;
            }
            network.junctions.insert(
                junction.id.clone(),
                Junction {
                    pt: junction.pt(),
                    id: junction.id,
                    junction_type: junction.junction_type,
                    incoming_lanes: junction.incoming_lanes,
                    internal_lanes: junction.internal_lanes,
                    shape: junction.shape.unwrap(),
                },
            );
        }

        for edge in raw.edges {
            if edge.function == raw::Function::Internal {
                let mut lanes = Vec::new();
                for lane in edge.lanes {
                    lanes.push(InternalLane {
                        id: InternalLaneID(lane.id),
                        index: lane.index,
                        speed: lane.speed,
                        length: lane.length,
                        center_line: lane.shape.ok(),
                        allow: lane.allow,
                    });
                }
                network
                    .internal_edges
                    .insert(edge.id.clone(), InternalEdge { id: edge.id, lanes });
                continue;
            }

            let from = edge.from.unwrap();
            let to = edge.to.unwrap();
            let template = &types[edge.edge_type.as_ref().unwrap()];

            let raw_center_line = match edge.shape {
                Some(pl) => pl,
                None => {
                    PolyLine::must_new(vec![network.junctions[&from].pt, network.junctions[&to].pt])
                }
            };
            // TODO I tried interpreting the docs and shifting left/right by 1x or 0.5x of the total
            // road width, but the results don't look right.
            let center_line = match edge.spread_type {
                raw::SpreadType::Center => raw_center_line,
                raw::SpreadType::Right => raw_center_line,
                raw::SpreadType::RoadCenter => raw_center_line,
            };

            let mut lanes = Vec::new();
            for lane in edge.lanes {
                lanes.push(Lane {
                    id: LaneID(lane.id),
                    index: lane.index,
                    speed: lane.speed,
                    length: lane.length,
                    // https://sumo.dlr.de/docs/Simulation/SublaneModel.html
                    width: lane.width.unwrap_or(Distance::meters(3.2)),
                    center_line: lane.shape.unwrap(),
                    allow: lane.allow,
                });
            }

            network.normal_edges.insert(
                edge.id.clone(),
                Edge {
                    id: edge.id,
                    edge_type: edge.edge_type.unwrap(),
                    name: edge.name,
                    from,
                    to,
                    priority: edge.priority.unwrap_or_else(|| template.priority),
                    lanes,
                    center_line,
                },
            );
        }

        network.fix_coordinates();
        network
    }

    /// Normalize coordinates to map-space, with Y increasing down.
    fn fix_coordinates(&mut self) {
        // I tried netconvert's --flip-y-axis option, but it makes all of the y coordinates
        // extremely negative.

        let max_y = self.location.converted_boundary.max_y;
        let fix = |pt: &Pt2D| Pt2D::new(pt.x(), max_y - pt.y());

        for junction in self.junctions.values_mut() {
            junction.pt = fix(&junction.pt);
            junction.shape =
                Ring::must_new(junction.shape.points().iter().map(fix).collect()).to_polygon();
        }
        for edge in self.normal_edges.values_mut() {
            edge.center_line =
                PolyLine::must_new(edge.center_line.points().iter().map(fix).collect());
            for lane in &mut edge.lanes {
                lane.center_line =
                    PolyLine::must_new(lane.center_line.points().iter().map(fix).collect());
            }
        }
        for edge in self.internal_edges.values_mut() {
            for lane in &mut edge.lanes {
                if let Some(pl) = lane.center_line.take() {
                    lane.center_line =
                        Some(PolyLine::must_new(pl.points().iter().map(fix).collect()));
                }
            }
        }
    }
}
