use crate::render::{BIG_ARROW_THICKNESS, TURN_ICON_ARROW_LENGTH};
use ezgui::{Color, GeomBatch, GfxCtx};
use geom::{Distance, Line, Polygon};
use map_model::{IntersectionID, Map, RoadID, Turn, TurnGroup, LANE_THICKNESS};
use std::collections::HashMap;

pub struct DrawTurn {}

impl DrawTurn {
    pub fn full_geom(t: &Turn, batch: &mut GeomBatch, color: Color) {
        batch.push(color, t.geom.make_arrow(BIG_ARROW_THICKNESS * 2.0).unwrap());
    }

    pub fn draw_full(t: &Turn, g: &mut GfxCtx, color: Color) {
        let mut batch = GeomBatch::new();
        DrawTurn::full_geom(t, &mut batch, color);
        batch.draw(g);
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

// TODO Don't store these in DrawMap; just generate when we hop into the traffic signal editor.
// Simplifies apply_map_edits!
pub struct DrawTurnGroup {
    pub group: TurnGroup,
    pub block: Polygon,
}

impl DrawTurnGroup {
    pub fn for_i(i: IntersectionID, map: &Map) -> Vec<DrawTurnGroup> {
        // TODO Sort by angle here if we want some consistency
        // TODO Handle short roads
        let mut offset_per_road: HashMap<RoadID, f64> = HashMap::new();
        let mut draw = Vec::new();
        for group in TurnGroup::for_i(i, map) {
            let offset = offset_per_road.entry(group.from).or_insert(0.5);

            // TODO center it properly
            let pl = {
                let r = map.get_r(group.from);
                if r.dst_i == i {
                    r.center_pts.reversed()
                } else {
                    r.center_pts.clone()
                }
            };
            // TODO Not number of turns, number of source lanes
            let block = pl
                .exact_slice(
                    *offset * TURN_ICON_ARROW_LENGTH,
                    (*offset + 1.0) * TURN_ICON_ARROW_LENGTH,
                )
                .make_polygons(LANE_THICKNESS * (group.members.len() as f64));
            draw.push(DrawTurnGroup { group, block });

            *offset += 1.0;
        }
        draw
    }
}
