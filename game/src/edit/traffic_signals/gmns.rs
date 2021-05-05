// TODO Move to map_model

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use maplit::btreeset;
use serde::{Deserialize, Deserializer};

use geom::{Distance, Duration, FindClosest, LonLat, Polygon, Pt2D};
use map_model::{
    osm, ControlTrafficSignal, DirectedRoadID, IntersectionID, Map, Movement, MovementID, Stage,
    StageType,
};

pub fn import(map: &Map, i: IntersectionID, path: &str) -> Result<ControlTrafficSignal> {
    let i = map.get_i(i);
    let mut matches_per_time_window: BTreeMap<String, Vec<Record>> = BTreeMap::new();
    for rec in csv::Reader::from_reader(std::fs::File::open(path)?).deserialize() {
        let rec: Record = rec?;
        if !rec.osm_ids.contains(&i.orig_id) {
            continue;
        }
        matches_per_time_window
            .entry(rec.time_window.clone())
            .or_insert_with(Vec::new)
            .push(rec);
    }

    // For now, just take the arbitrary first set of stages for any time window
    let records = matches_per_time_window
        .into_iter()
        .next()
        .ok_or(anyhow!("no matches for {}", i.orig_id))?
        .1;

    let snapper = Snapper::new(map, i.id);

    let mut tsig = ControlTrafficSignal::new(map, i.id);
    tsig.stages.clear();
    for rec in records {
        let mvmnt = snapper.get_mvmnt(map.get_gps_bounds().convert(&rec.geometry))?;

        tsig.stages.push(Stage {
            protected_movements: btreeset! {mvmnt},
            yield_movements: BTreeSet::new(),
            stage_type: StageType::Fixed(Duration::seconds(30.0)),
        });
    }

    Ok(tsig)
}

#[derive(Debug, Deserialize)]
struct Record {
    #[serde(deserialize_with = "parse_osm_ids", rename = "oms_node_id")]
    osm_ids: Vec<osm::NodeID>,
    time_window: String,
    green_time: usize,
    stage: usize,
    #[serde(deserialize_with = "parse_linestring")]
    geometry: Vec<LonLat>,
}

fn parse_linestring<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<LonLat>, D::Error> {
    let raw = <String>::deserialize(d)?;
    LonLat::parse_wkt_linestring(&raw)
        .ok_or(serde::de::Error::custom(format!("bad linestring {}", raw)))
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

/// Snaps line-strings to a vehicle movement across an intersection. It matches points to a road
/// preferably by the thickened road polygon containing the point, but since the GMNS source may
/// disagree about the road endpoint (due to things like service roads being included or excluded
/// differently), fall back to the closest polygon. OSM IDs aren't used to snap, because of the
/// same service road issue, and since the IDs may change over time.
struct Snapper {
    i: IntersectionID,
    roads_incoming: Vec<(DirectedRoadID, Polygon)>,
    roads_outgoing: Vec<(DirectedRoadID, Polygon)>,
    closest_incoming: FindClosest<DirectedRoadID>,
    closest_outgoing: FindClosest<DirectedRoadID>,
    movements: BTreeMap<MovementID, Movement>,
}

impl Snapper {
    fn new(map: &Map, i: IntersectionID) -> Snapper {
        let mut roads_incoming: Vec<(DirectedRoadID, Polygon)> = Vec::new();
        let mut roads_outgoing: Vec<(DirectedRoadID, Polygon)> = Vec::new();
        let mut closest_incoming = FindClosest::new(map.get_bounds());
        let mut closest_outgoing = FindClosest::new(map.get_bounds());
        for r in &map.get_i(i).roads {
            let r = map.get_r(*r);
            let poly = r.get_thick_polygon(map);

            closest_incoming.add(r.directed_id_to(i), poly.points());
            closest_outgoing.add(r.directed_id_from(i), poly.points());

            roads_incoming.push((r.directed_id_to(i), poly.clone()));
            roads_outgoing.push((r.directed_id_from(i), poly));
        }

        Snapper {
            i,
            roads_incoming,
            roads_outgoing,
            closest_incoming,
            closest_outgoing,
            movements: ControlTrafficSignal::new(map, i).movements,
        }
    }

    fn get_mvmnt(&self, pts: Vec<Pt2D>) -> Result<MovementID> {
        let threshold = Distance::meters(1000.0);

        let from = self
            .roads_incoming
            .iter()
            .find(|(_, poly)| poly.contains_pt(pts[0]))
            .map(|(r, _)| *r)
            .or_else(|| {
                self.closest_incoming
                    .closest_pt(pts[0], threshold)
                    .map(|(r, _)| r)
            })
            .ok_or(anyhow!("no road has start point {}", pts[0]))?;
        let last_pt = *pts.last().unwrap();
        let to = self
            .roads_outgoing
            .iter()
            .find(|(_, poly)| poly.contains_pt(last_pt))
            .map(|(r, _)| *r)
            .or_else(|| {
                self.closest_outgoing
                    .closest_pt(last_pt, threshold)
                    .map(|(r, _)| r)
            })
            .ok_or(anyhow!("no road has end point {}", last_pt))?;
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
