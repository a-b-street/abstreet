use geom::{Angle, ArrowCap, Distance, PolyLine, Polygon};
use map_model::{IntersectionCluster, IntersectionID, LaneID, Map, TurnGroupID, UberTurnGroup};
use std::collections::{HashMap, HashSet};

const TURN_ICON_ARROW_LENGTH: Distance = Distance::const_meters(1.5);
const UBER_TURN_ICON_ARROW_LENGTH: Distance = Distance::const_meters(3.0);

pub struct DrawTurnGroup {
    pub id: TurnGroupID,
    pub block: Polygon,
    pub arrow: Polygon,
}

impl DrawTurnGroup {
    pub fn for_i(i: IntersectionID, map: &Map) -> Vec<DrawTurnGroup> {
        // TODO Sort by angle here if we want some consistency
        // TODO Handle short roads
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
            let slice = if pl.length() >= (offset + 1.0) * TURN_ICON_ARROW_LENGTH {
                pl.exact_slice(
                    offset * TURN_ICON_ARROW_LENGTH,
                    (offset + 1.0) * TURN_ICON_ARROW_LENGTH,
                )
            } else {
                pl
            };
            let block = slice.make_polygons(width);

            let arrow = {
                let center = slice.middle();
                PolyLine::new(vec![
                    center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, group.angle.opposite()),
                    center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, group.angle),
                ])
                .make_arrow(Distance::meters(0.5), ArrowCap::Triangle)
                .unwrap()
            };

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

// TODO Share some code with DrawTurnGroup?
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
            // TODO Right now they have one lane, but probably changing this soon
            let offset = group
                .members
                .iter()
                .map(|ut| *offset_per_lane.entry(ut.entry()).or_insert(0))
                .max()
                .unwrap() as f64;
            let (pl, width) = group.src_center_and_width(map);
            let height = UBER_TURN_ICON_ARROW_LENGTH;
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
                // Shrink this to fit inside block.
                // TODO This is not quite right yet
                let arrow = group
                    .geom
                    .make_arrow(Distance::meters(0.5), ArrowCap::Triangle)
                    .unwrap();
                // Autocrop
                let full_bounds = arrow.get_bounds();
                let arrow = arrow.translate(-full_bounds.min_x, -full_bounds.min_y);
                // Rotate it to account for the rotated block
                let rot = slice
                    .last_line()
                    .angle()
                    .shortest_rotation_towards(Angle::ZERO);
                let arrow = arrow.rotate(rot);
                let full_bounds = arrow.get_bounds();
                // Scale it
                let arrow = arrow.scale(
                    (width.inner_meters() / full_bounds.width())
                        .min(height.inner_meters() / full_bounds.height()),
                );
                // Un-rotate it
                let arrow = arrow.rotate(-rot);
                // And translate back into the block
                arrow.translate(
                    block.center().x() - width.inner_meters() / 2.0,
                    block.center().y() - height.inner_meters() / 2.0,
                )
            };

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
