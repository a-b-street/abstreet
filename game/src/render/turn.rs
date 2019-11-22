use crate::render::BIG_ARROW_THICKNESS;
use ezgui::{Color, GeomBatch, GfxCtx};
use geom::{Distance, Line, PolyLine, Polygon};
use map_model::{IntersectionID, LaneID, Map, Turn, TurnGroup};
use std::collections::{HashMap, HashSet};

const TURN_ICON_ARROW_LENGTH: Distance = Distance::const_meters(2.0);

pub struct DrawTurn {}

impl DrawTurn {
    pub fn draw_full(t: &Turn, g: &mut GfxCtx, color: Color) {
        g.draw_polygon(
            color,
            &t.geom.make_arrow(BIG_ARROW_THICKNESS * 2.0).unwrap(),
        );
    }

    // TODO make a polyline.dashed or something
    // TODO get rid of all these weird DrawTurn things generally
    pub fn draw_dashed(turn: &Turn, batch: &mut GeomBatch, color: Color) {
        let dash_len = Distance::meters(1.0);
        batch.extend(
            color,
            turn.geom
                .dashed_polygons(BIG_ARROW_THICKNESS, dash_len, Distance::meters(0.5)),
        );
        // And a cap on the arrow. In case the last line is long, trim it to be the dash
        // length.
        let last_line = turn.geom.last_line();
        let last_len = last_line.length();
        let arrow_line = if last_len <= dash_len {
            last_line
        } else {
            Line::new(last_line.dist_along(last_len - dash_len), last_line.pt2())
        };
        batch.push(
            color,
            arrow_line
                .to_polyline()
                .make_arrow(BIG_ARROW_THICKNESS)
                .unwrap(),
        );
    }
}

pub struct DrawTurnGroup {
    pub group: TurnGroup,
    pub block: Polygon,
    pub arrow: Polygon,
}

impl DrawTurnGroup {
    pub fn for_i(i: IntersectionID, map: &Map) -> Vec<DrawTurnGroup> {
        // TODO Sort by angle here if we want some consistency
        // TODO Handle short roads
        let mut offset_per_lane: HashMap<LaneID, usize> = HashMap::new();
        let mut draw = Vec::new();
        for group in TurnGroup::for_i(i, map) {
            let offset = group
                .members
                .iter()
                .map(|t| *offset_per_lane.entry(t.src).or_insert(0))
                .max()
                .unwrap() as f64;
            let (pl, width) = group.src_center_and_width(map);
            let slice = pl.exact_slice(
                offset * TURN_ICON_ARROW_LENGTH,
                (offset + 1.0) * TURN_ICON_ARROW_LENGTH,
            );
            let block = slice.make_polygons(width);

            let arrow = {
                let angle = group.angle(map);
                let center = slice.middle();
                PolyLine::new(vec![
                    center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle.opposite()),
                    center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle),
                ])
                .make_arrow(Distance::meters(0.5))
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
                group,
                block,
                arrow,
            });
        }
        draw
    }
}
