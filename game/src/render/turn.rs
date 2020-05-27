use geom::{Angle, ArrowCap, Distance, PolyLine, Polygon};
use map_model::{IntersectionCluster, IntersectionID, LaneID, Map, TurnGroupID, UberTurnGroup};
use std::collections::{HashMap, HashSet};

const TURN_ICON_ARROW_LENGTH: Distance = Distance::const_meters(1.5);

pub struct DrawTurnGroup {
    pub id: TurnGroupID,
    pub block: Polygon,
    pub arrow: Polygon,
}

impl DrawTurnGroup {
    pub fn for_i(i: IntersectionID, map: &Map) -> Vec<DrawTurnGroup> {
        // TODO Sort by angle here if we want some consistency
        let mut offset_per_lane: HashMap<LaneID, usize> = HashMap::new();
        let mut draw = Vec::new();
        for group in map.get_traffic_signal(i).turn_groups.values() {
            let offset = group
                .members
                .iter()
                .map(|t| *offset_per_lane.entry(t.src).or_insert(0))
                .max()
                .unwrap() as f64;
            let (pl, width) = group.src_center_and_width(map);
            let (block, arrow) = make_geom(offset, pl, width, group.angle);
            let mut seen_lanes = HashSet::new();
            for t in &group.members {
                if !seen_lanes.contains(&t.src) {
                    *offset_per_lane.get_mut(&t.src).unwrap() += 1;
                    seen_lanes.insert(t.src);
                }
            }

            draw.push(DrawTurnGroup {
                id: group.id,
                block,
                arrow,
            });
        }
        draw
    }
}

pub struct DrawUberTurnGroup {
    pub group: UberTurnGroup,
    pub block: Polygon,
    pub arrow: Polygon,
}

impl DrawUberTurnGroup {
    pub fn new(ic: &IntersectionCluster, map: &Map) -> Vec<DrawUberTurnGroup> {
        let mut offset_per_lane: HashMap<LaneID, usize> = HashMap::new();
        let mut draw = Vec::new();
        for group in ic.uber_turn_groups(map) {
            let offset = group
                .members
                .iter()
                .map(|ut| *offset_per_lane.entry(ut.entry()).or_insert(0))
                .max()
                .unwrap() as f64;
            let (pl, width) = group.src_center_and_width(map);
            let (block, arrow) = make_geom(offset, pl, width, group.angle());
            let mut seen_lanes = HashSet::new();
            for ut in &group.members {
                if !seen_lanes.contains(&ut.entry()) {
                    *offset_per_lane.get_mut(&ut.entry()).unwrap() += 1;
                    seen_lanes.insert(ut.entry());
                }
            }

            draw.push(DrawUberTurnGroup {
                group,
                block,
                arrow,
            });
        }
        draw
    }
}

// Produces (block, arrow)
fn make_geom(offset: f64, pl: PolyLine, width: Distance, angle: Angle) -> (Polygon, Polygon) {
    let height = TURN_ICON_ARROW_LENGTH;
    // Always extend the pl first to handle short entry lanes
    let extension = PolyLine::new(vec![
        pl.last_pt(),
        pl.last_pt()
            .project_away(Distance::meters(500.0), pl.last_line().angle()),
    ]);
    let pl = pl.extend(extension);
    let slice = pl.exact_slice(offset * height, (offset + 1.0) * height);
    let block = slice.make_polygons(width);

    let arrow = {
        let center = slice.middle();
        PolyLine::new(vec![
            center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle.opposite()),
            center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle),
        ])
        .make_arrow(Distance::meters(0.5), ArrowCap::Triangle)
        .unwrap()
    };

    (block, arrow)
}
