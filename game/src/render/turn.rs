use geom::{Angle, ArrowCap, Distance, PolyLine, Polygon};
use map_model::{IntersectionCluster, IntersectionID, LaneID, Map, MovementID, UberTurnGroup};
use std::collections::{HashMap, HashSet};

const TURN_ICON_ARROW_LENGTH: Distance = Distance::const_meters(1.5);

pub struct DrawMovement {
    pub id: MovementID,
    pub block: Polygon,
    pub arrow: Polygon,
}

impl DrawMovement {
    pub fn for_i(i: IntersectionID, map: &Map) -> Vec<DrawMovement> {
        // TODO Sort by angle here if we want some consistency
        let mut offset_per_lane: HashMap<LaneID, usize> = HashMap::new();
        let mut draw = Vec::new();
        for movement in map.get_traffic_signal(i).movements.values() {
            let offset = movement
                .members
                .iter()
                .map(|t| *offset_per_lane.entry(t.src).or_insert(0))
                .max()
                .unwrap();
            let (pl, width) = movement.src_center_and_width(map);
            let (block, arrow) = make_geom(offset as f64, pl, width, movement.angle);
            let mut seen_lanes = HashSet::new();
            for t in &movement.members {
                if !seen_lanes.contains(&t.src) {
                    *offset_per_lane.get_mut(&t.src).unwrap() = offset + 1;
                    seen_lanes.insert(t.src);
                }
            }

            draw.push(DrawMovement {
                id: movement.id,
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
                .unwrap();
            let (pl, width) = group.src_center_and_width(map);
            let (block, arrow) = make_geom(offset as f64, pl, width, group.angle());
            let mut seen_lanes = HashSet::new();
            for ut in &group.members {
                if !seen_lanes.contains(&ut.entry()) {
                    *offset_per_lane.get_mut(&ut.entry()).unwrap() = offset + 1;
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
    let extension = PolyLine::must_new(vec![
        pl.last_pt(),
        pl.last_pt()
            .project_away(Distance::meters(500.0), pl.last_line().angle()),
    ]);
    let pl = pl.must_extend(extension);
    let slice = pl.exact_slice(offset * height, (offset + 1.0) * height);
    let block = slice.make_polygons(width);

    let arrow = {
        let center = slice.middle();
        PolyLine::must_new(vec![
            center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle.opposite()),
            center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle),
        ])
        .make_arrow(Distance::meters(0.5), ArrowCap::Triangle)
    };

    (block, arrow)
}
