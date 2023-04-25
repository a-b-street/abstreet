// TODO Move to map_model

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Cursor;

use anyhow::Result;
use serde::{Deserialize, Deserializer};

use geom::{Angle, Duration, LonLat, Pt2D};
use map_model::{
    osm, ControlTrafficSignal, DirectedRoadID, DrivingSide, EditCmd, EditIntersection,
    IntersectionID, Map, Movement, MovementID, Stage, StageType, TurnPriority, TurnType,
};
use widgetry::tools::PopupMsg;
use widgetry::{EventCtx, State};

use crate::edit::apply_map_edits;
use crate::App;

/// This imports timing.csv from https://github.com/asu-trans-ai-lab/Vol2Timing. It operates in a
/// best-effort / permissive mode, skipping over mismatched movements and other problems and should
/// still be considered experimental.
pub fn import(map: &Map, i: IntersectionID, bytes: &Vec<u8>) -> Result<ControlTrafficSignal> {
    let i = map.get_i(i);
    let mut matches_per_plan: BTreeMap<String, Vec<Record>> = BTreeMap::new();
    for rec in csv::Reader::from_reader(Cursor::new(bytes)).deserialize() {
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
        .ok_or_else(|| anyhow!("no matches for {}", i.orig_id))?
        .1;
    records.sort_by_key(|rec| rec.stage);

    let snapper = Snapper::new(map, i.id)?;

    let mut signal = ControlTrafficSignal::new(map, i.id);
    signal.stages.clear();
    for rec in records {
        let stage_idx = rec.stage - 1;
        match signal.stages.len().cmp(&stage_idx) {
            std::cmp::Ordering::Equal => {
                signal.stages.push(Stage {
                    protected_movements: BTreeSet::new(),
                    yield_movements: BTreeSet::new(),
                    stage_type: StageType::Fixed(Duration::seconds(rec.green_time as f64)),
                });
            }
            std::cmp::Ordering::Less => {
                bail!("missing intermediate stage");
            }
            std::cmp::Ordering::Greater => {}
        }
        let stage = &mut signal.stages[stage_idx];

        if stage.stage_type.simple_duration() != Duration::seconds(rec.green_time as f64) {
            bail!(
                "Stage {} has green_times {} and {}",
                rec.stage,
                stage.stage_type.simple_duration(),
                rec.green_time
            );
        }

        let mvmnt = match snapper.get_mvmnt(
            (
                rec.geometry.0.to_pt(map.get_gps_bounds()),
                rec.geometry.1.to_pt(map.get_gps_bounds()),
            ),
            &rec.mvmt_txt_id,
            map,
        ) {
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

    add_crosswalks(&mut signal, map);

    Ok(signal)
}

pub fn import_all(
    ctx: &mut EventCtx,
    app: &mut App,
    path: &str,
    bytes: Vec<u8>,
) -> Box<dyn State<App>> {
    let all_signals: Vec<IntersectionID> = app
        .primary
        .map
        .all_intersections()
        .iter()
        .filter_map(|i| {
            if i.is_traffic_signal() {
                Some(i.id)
            } else {
                None
            }
        })
        .collect();
    let mut successes = 0;
    let mut failures_no_match = 0;
    let mut failures_other = 0;
    let mut edits = app.primary.map.get_edits().clone();

    ctx.loading_screen("import signal timing", |_, timer| {
        timer.start_iter("import", all_signals.len());
        for i in all_signals {
            timer.next();
            match import(&app.primary.map, i, &bytes)
                .and_then(|signal| signal.validate(app.primary.map.get_i(i)).map(|_| signal))
            {
                Ok(signal) => {
                    info!("Success at {}", i);
                    successes += 1;
                    edits.commands.push(EditCmd::ChangeIntersection {
                        i,
                        old: app.primary.map.get_i_edit(i),
                        new: EditIntersection::TrafficSignal(signal.export(&app.primary.map)),
                    });
                }
                Err(err) => {
                    error!("Failure at {}: {}", i, err);
                    if err.to_string().contains("no matches for") {
                        failures_no_match += 1;
                    } else {
                        failures_other += 1;
                    }
                }
            }
        }
    });

    apply_map_edits(ctx, app, edits);

    PopupMsg::new_state(
        ctx,
        &format!("Import from {}", path),
        vec![
            format!("{} traffic signals successfully imported", successes),
            format!("{} intersections without any data", failures_no_match),
            format!("{} other failures", failures_other),
        ],
    )
}

#[derive(Debug, Deserialize)]
struct Record {
    #[serde(deserialize_with = "parse_osm_ids", rename = "osm_node_id")]
    osm_ids: Vec<osm::NodeID>,
    timing_plan_id: String,
    green_time: usize,
    #[serde(rename = "stage_no")]
    stage: usize,
    #[serde(deserialize_with = "parse_linestring")]
    geometry: (LonLat, LonLat),
    protection: String,
    // Something like EBL or NBT -- eastbound left, northbound through.
    mvmt_txt_id: String,
}

fn parse_linestring<'de, D: Deserializer<'de>>(d: D) -> Result<(LonLat, LonLat), D::Error> {
    let raw = <String>::deserialize(d)?;
    let pts = LonLat::parse_wkt_linestring(&raw)
        .ok_or_else(|| serde::de::Error::custom(format!("bad linestring {}", raw)))?;
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
    for id in raw.split('_') {
        ids.push(osm::NodeID(id.parse::<i64>().map_err(|_| {
            serde::de::Error::custom(format!("bad ID {}", id))
        })?));
    }
    Ok(ids)
}

/// Snaps a line to a vehicle movement across an intersection. It uses movement endpoints and a
/// hint about turn type to match.
///
/// OSM IDs aren't used to snap, because GMNS and A/B Street may disagree about where a road
/// segment begins/ends. This could happen from OSM IDs changing over time or from different rules
/// about importing things like service roads.
struct Snapper {
    roads_incoming: HashMap<DirectedRoadID, Pt2D>,
    roads_outgoing: HashMap<DirectedRoadID, Pt2D>,
    movements: BTreeMap<MovementID, Movement>,
}

impl Snapper {
    fn new(map: &Map, i: IntersectionID) -> Result<Snapper> {
        let mut roads_incoming = HashMap::new();
        let mut roads_outgoing = HashMap::new();
        for r in &map.get_i(i).roads {
            let r = map.get_r(*r);

            let incoming_id = r.directed_id_to(i);
            let outgoing_id = r.directed_id_from(i);

            // TODO There are a few methods for finding the "middle" of a directed road; here's yet
            // another.
            let mut incoming_pts = Vec::new();
            let mut outgoing_pts = Vec::new();

            for l in &r.lanes {
                if l.lane_type.is_walkable() {
                    continue;
                }
                if l.dir == incoming_id.dir {
                    incoming_pts.push(l.lane_center_pts.last_pt());
                } else {
                    outgoing_pts.push(l.lane_center_pts.first_pt());
                }
            }

            if !incoming_pts.is_empty() {
                roads_incoming.insert(incoming_id, Pt2D::center(&incoming_pts));
            }
            if !outgoing_pts.is_empty() {
                roads_outgoing.insert(outgoing_id, Pt2D::center(&outgoing_pts));
            }
        }
        if roads_incoming.is_empty() || roads_outgoing.is_empty() {
            bail!("{} has no incoming or outgoing roads", i);
        }

        Ok(Snapper {
            roads_incoming,
            roads_outgoing,
            movements: map
                .get_i(i)
                .movements
                .iter()
                .filter(|(id, _)| !id.crosswalk)
                .map(|(k, v)| (*k, v.clone()))
                .collect(),
        })
    }

    fn get_mvmnt(&self, pair: (Pt2D, Pt2D), code: &str, map: &Map) -> Result<MovementID> {
        // Code is something like "WBT", westbound through.
        let code_turn_type = match code.chars().last() {
            Some('T') => TurnType::Straight,
            Some('L') => TurnType::Left,
            Some('R') => TurnType::Right,
            x => bail!("Weird movement_str {:?}", x),
        };
        let code_direction = &code[0..2];

        let (id, mvmnt) = self
            .movements
            .iter()
            .min_by_key(|(id, mvmnt)| {
                let from_cost = pair.0.dist_to(self.roads_incoming[&id.from]);
                let to_cost = pair.1.dist_to(self.roads_outgoing[&id.to]);
                let direction = cardinal_direction(
                    map.get_l(mvmnt.members[0].src)
                        .lane_center_pts
                        .overall_angle(),
                );

                // Arbitrary parameters, tuned to make weird geometry at University/Mill in Tempe
                // work.
                let type_cost = if mvmnt.turn_type == code_turn_type {
                    1.0
                } else {
                    2.0
                };
                // TODO This one is way more important than the geometry! Maybe JUST use the code?
                let direction_cost = if direction == code_direction {
                    1.0
                } else {
                    10.0
                };
                type_cost * direction_cost * (from_cost + to_cost)
            })
            .unwrap();

        // Debug if the we didn't agree
        let direction = cardinal_direction(
            map.get_l(mvmnt.members[0].src)
                .lane_center_pts
                .overall_angle(),
        );
        if mvmnt.turn_type != code_turn_type || direction != code_direction {
            warn!(
                "A {} snapped to a {} {:?}",
                code, direction, mvmnt.turn_type
            );
        }

        Ok(*id)
    }
}

fn cardinal_direction(angle: Angle) -> &'static str {
    // Note Y inversion, as usual
    let deg = angle.normalized_degrees();
    if deg >= 335.0 || deg <= 45.0 {
        return "EB";
    }
    if (45.0..=135.0).contains(&deg) {
        return "SB";
    }
    if (135.0..=225.0).contains(&deg) {
        return "WB";
    }
    "NB"
}

// The GMNS input doesn't include crosswalks yet -- and even once it does, it's likely the two map
// models will disagree about where sidewalks exist. Try to add all crosswalks to the stage where
// they're compatible. Downgrade right turns from protected to permitted as needed.
fn add_crosswalks(signal: &mut ControlTrafficSignal, map: &Map) {
    let downgrade_type = if map.get_config().driving_side == DrivingSide::Right {
        TurnType::Right
    } else {
        TurnType::Left
    };

    let i = map.get_i(signal.id);
    let mut crosswalks: Vec<MovementID> = Vec::new();
    for id in i.movements.keys() {
        if id.crosswalk {
            crosswalks.push(*id);
        }
    }

    // We could try to look for straight turns parallel to the crosswalk, but... just brute-force
    // it
    for stage in &mut signal.stages {
        crosswalks.retain(|id| {
            if stage.could_be_protected(*id, i) {
                stage.edit_movement(&i.movements[id], TurnPriority::Protected);
                false
            } else {
                // There may be conflicting right turns that we can downgrade. Try that.
                let mut stage_copy = stage.clone();
                for maybe_right_turn in stage.protected_movements.clone() {
                    if i.movements[&maybe_right_turn].turn_type == downgrade_type {
                        stage.protected_movements.remove(&maybe_right_turn);
                        stage.yield_movements.insert(maybe_right_turn);
                    }
                }
                if stage_copy.could_be_protected(*id, i) {
                    stage_copy.edit_movement(&i.movements[id], TurnPriority::Protected);
                    *stage = stage_copy;
                    false
                } else {
                    true
                }
            }
        });
    }
}
