// TODO Move to map_model

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde::{Deserialize, Deserializer};

use geom::{Duration, LonLat, Pt2D};
use map_model::{
    osm, ControlTrafficSignal, DirectedRoadID, IntersectionID, Map, Movement, MovementID, Stage,
    StageType,
};

/// This imports timing.csv from https://github.com/asu-trans-ai-lab/Vol2Timing. It operates in a
/// best-effort / permissive mode, skipping over mismatched movements and other problems and should
/// still be considered experimental.
pub fn import(map: &Map, i: IntersectionID, path: &str) -> Result<ControlTrafficSignal> {
    let i = map.get_i(i);
    let mut matches_per_plan: BTreeMap<String, Vec<Record>> = BTreeMap::new();
    for rec in csv::Reader::from_reader(std::fs::File::open(path)?).deserialize() {
        let rec: Record = rec?;
        if !rec.osm_ids.contains(&i.orig_id) {
            continue;
        }
        matches_per_plan
            .entry(rec.timing_plan_id.clone())
            .or_insert_with(Vec::new)
            .push(rec);
    }

    // For now, just use any arbitrary plan
    let mut records = matches_per_plan
        .into_iter()
        .next()
        .ok_or(anyhow!("no matches for {}", i.orig_id))?
        .1;
    records.sort_by_key(|rec| rec.stage);

    let snapper = Snapper::new(map, i.id)?;

    let mut tsig = ControlTrafficSignal::new(map, i.id);
    tsig.stages.clear();
    for rec in records {
        let stage_idx = rec.stage - 1;
        if tsig.stages.len() == stage_idx {
            tsig.stages.push(Stage {
                protected_movements: BTreeSet::new(),
                yield_movements: BTreeSet::new(),
                stage_type: StageType::Fixed(Duration::seconds(rec.green_time as f64)),
            });
        } else if stage_idx > tsig.stages.len() {
            bail!("missing intermediate stage");
        }
        let stage = &mut tsig.stages[stage_idx];

        if stage.stage_type.simple_duration() != Duration::seconds(rec.green_time as f64) {
            bail!(
                "Stage {} has green_times {} and {}",
                rec.stage,
                stage.stage_type.simple_duration(),
                rec.green_time
            );
        }

        let mvmnt = match snapper.get_mvmnt((
            rec.geometry.0.to_pt(map.get_gps_bounds()),
            rec.geometry.1.to_pt(map.get_gps_bounds()),
        )) {
            Ok(x) => x,
            Err(err) => {
                error!(
                    "Skipping {} -> {} for stage {}: {}",
                    rec.geometry.0, rec.geometry.1, rec.stage, err
                );
                continue;
            }
        };
        if rec.protection == "protected" {
            stage.protected_movements.insert(mvmnt);
        } else {
            stage.yield_movements.insert(mvmnt);
        }
    }

    Ok(tsig)
}

#[derive(Debug, Deserialize)]
struct Record {
    #[serde(deserialize_with = "parse_osm_ids", rename = "oms_node_id")]
    osm_ids: Vec<osm::NodeID>,
    timing_plan_id: String,
    green_time: usize,
    #[serde(rename = "stage_no")]
    stage: usize,
    #[serde(deserialize_with = "parse_linestring", rename = "geometory")]
    geometry: (LonLat, LonLat),
    protection: String,
}

fn parse_linestring<'de, D: Deserializer<'de>>(d: D) -> Result<(LonLat, LonLat), D::Error> {
    let raw = <String>::deserialize(d)?;
    let pts = LonLat::parse_wkt_linestring(&raw)
        .ok_or(serde::de::Error::custom(format!("bad linestring {}", raw)))?;
    if pts.len() != 2 {
        return Err(serde::de::Error::custom(format!(
            "{} points, expecting 2",
            pts.len()
        )));
    }
    Ok((pts[0], pts[1]))
}

fn parse_osm_ids<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<osm::NodeID>, D::Error> {
    let raw = <String>::deserialize(d)?;
    let mut ids = Vec::new();
    for id in raw.split(";") {
        ids.push(osm::NodeID(id.parse::<i64>().map_err(|_| {
            serde::de::Error::custom(format!("bad ID {}", id))
        })?));
    }
    Ok(ids)
}

/// Snaps a line to a vehicle movement across an intersection. It matches each endpoint to the
/// closest end of a directed road.
///
/// OSM IDs aren't used to snap, because GMNS and A/B Street may disagree about where a road
/// segment begins/ends. This could happen from OSM IDs changing over time or from different rules
/// about importing things like service roads.
struct Snapper {
    i: IntersectionID,
    roads_incoming: Vec<(DirectedRoadID, Pt2D)>,
    roads_outgoing: Vec<(DirectedRoadID, Pt2D)>,
    movements: BTreeMap<MovementID, Movement>,
}

impl Snapper {
    fn new(map: &Map, i: IntersectionID) -> Result<Snapper> {
        let mut roads_incoming = Vec::new();
        let mut roads_outgoing = Vec::new();
        for r in &map.get_i(i).roads {
            let r = map.get_r(*r);

            let incoming_id = r.directed_id_to(i);
            let outgoing_id = r.directed_id_from(i);

            // TODO There are a few methods for finding the "middle" of a directed road; here's yet
            // another.
            let mut incoming_pts = Vec::new();
            let mut outgoing_pts = Vec::new();

            for (l, dir, lt) in r.lanes_ltr() {
                if lt.is_walkable() {
                    continue;
                }
                if dir == incoming_id.dir {
                    incoming_pts.push(map.get_l(l).lane_center_pts.last_pt());
                } else {
                    outgoing_pts.push(map.get_l(l).lane_center_pts.first_pt());
                }
            }

            if !incoming_pts.is_empty() {
                roads_incoming.push((incoming_id, Pt2D::center(&incoming_pts)));
            }
            if !outgoing_pts.is_empty() {
                roads_outgoing.push((outgoing_id, Pt2D::center(&outgoing_pts)));
            }
        }
        if roads_incoming.is_empty() || roads_outgoing.is_empty() {
            bail!("{} has no incoming or outgoing roads", i);
        }

        Ok(Snapper {
            i,
            roads_incoming,
            roads_outgoing,
            movements: ControlTrafficSignal::new(map, i).movements,
        })
    }

    fn get_mvmnt(&self, pair: (Pt2D, Pt2D)) -> Result<MovementID> {
        let from = self
            .roads_incoming
            .iter()
            .min_by_key(|(_, pt)| pt.dist_to(pair.0))
            .unwrap()
            .0;
        let to = self
            .roads_outgoing
            .iter()
            .min_by_key(|(_, pt)| pt.dist_to(pair.1))
            .unwrap()
            .0;
        if from == to {
            bail!("loop on {}", from);
        }
        let mvmnt = MovementID {
            from,
            to,
            parent: self.i,
            crosswalk: false,
        };
        if !self.movements.contains_key(&mvmnt) {
            bail!("Matched non-existent {:?}", mvmnt);
        }
        Ok(mvmnt)
    }
}
