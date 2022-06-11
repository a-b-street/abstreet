use std::collections::BTreeMap;

use geom::{Distance, PolyLine, Pt2D};
use map_model::{CommonEndpoint, PathStepV2, PathV2, RoadID};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{Color, EventCtx};

use crate::AppLike;

pub fn draw_overlapping_paths(
    ctx: &mut EventCtx,
    app: &dyn AppLike,
    paths: Vec<(PathV2, Color)>,
) -> ToggleZoomed {
    // Per road, just figure out what colors we need
    let mut colors_per_road: BTreeMap<RoadID, Vec<Color>> = BTreeMap::new();
    let mut colors_per_movement: Vec<(RoadID, RoadID, Color)> = Vec::new();
    for (path, color) in paths {
        for step in path.get_steps() {
            match step {
                PathStepV2::Along(dr) | PathStepV2::Contraflow(dr) => {
                    colors_per_road
                        .entry(dr.road)
                        .or_insert_with(Vec::new)
                        .push(color);
                }
                PathStepV2::Movement(m) => {
                    colors_per_movement.push((m.from.road, m.to.road, color));
                }
                PathStepV2::ContraflowMovement(m) => {
                    colors_per_movement.push((m.to.road, m.from.road, color));
                }
            }
        }
    }

    // Per road and color, mark where the adjusted polyline begins and ends, and its width
    // TODO Make Color implement Ord; use hex in the meantime
    let mut pieces: BTreeMap<(RoadID, String), (Pt2D, Pt2D, Distance)> = BTreeMap::new();
    // Per road, divide the needed colors proportionally
    let mut draw = ToggleZoomed::builder();
    for (road, colors) in colors_per_road {
        let road = app.map().get_r(road);
        let width_per_piece = road.get_width() / (colors.len() as f64);
        for (idx, color) in colors.into_iter().enumerate() {
            if let Ok(pl) = road.shift_from_left_side((0.5 + (idx as f64)) * width_per_piece) {
                let polygon = pl.make_polygons(width_per_piece);
                draw.unzoomed.push(color.alpha(0.8), polygon.clone());
                draw.zoomed.push(color.alpha(0.5), polygon);

                pieces.insert(
                    (road.id, color.as_hex()),
                    (pl.first_pt(), pl.last_pt(), width_per_piece),
                );
            }
        }
    }

    // Fill in intersections
    for (from, to, color) in colors_per_movement {
        if let Some((from_pt1, from_pt2, from_width)) = pieces.get(&(from, color.as_hex())).cloned()
        {
            if let Some((to_pt1, to_pt2, to_width)) = pieces.get(&(to, color.as_hex())).cloned() {
                let from_road = app.map().get_r(from);
                let to_road = app.map().get_r(to);
                if let CommonEndpoint::One(i) = from_road.common_endpoint(to_road) {
                    let pt1 = if from_road.src_i == i {
                        from_pt1
                    } else {
                        from_pt2
                    };
                    let pt2 = if to_road.src_i == i { to_pt1 } else { to_pt2 };
                    if let Ok(pl) = PolyLine::new(vec![pt1, pt2]) {
                        let polygon = pl.make_polygons(from_width.min(to_width));
                        draw.unzoomed.push(color.alpha(0.8), polygon.clone());
                        draw.zoomed.push(color.alpha(0.5), polygon);
                    }
                }
            }
        }
    }
    draw.build(ctx)
}
