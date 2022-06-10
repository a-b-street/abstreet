use std::collections::BTreeMap;

use map_model::{PathStepV2, PathV2, RoadID};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{Color, EventCtx};

use crate::App;

// TODO Move to map_gui
pub fn draw_overlapping_paths(
    ctx: &mut EventCtx,
    app: &App,
    paths: Vec<(PathV2, Color)>,
) -> ToggleZoomed {
    // Per road, just figure out what colors we need
    let mut colors_per_road: BTreeMap<RoadID, Vec<Color>> = BTreeMap::new();
    for (path, color) in paths {
        for step in path.get_steps() {
            match step {
                PathStepV2::Along(dr) | PathStepV2::Contraflow(dr) => {
                    colors_per_road
                        .entry(dr.road)
                        .or_insert_with(Vec::new)
                        .push(color);
                }
                PathStepV2::Movement(_) | PathStepV2::ContraflowMovement(_) => {
                    // TODO Intersections?
                }
            }
        }
    }

    // Per road, divide the needed colors proportionally
    let mut draw = ToggleZoomed::builder();
    for (road, colors) in colors_per_road {
        let road = app.map.get_r(road);
        let width_per_piece = road.get_width() / (colors.len() as f64);
        for (idx, color) in colors.into_iter().enumerate() {
            if let Ok(pl) = road.shift_from_left_side((0.5 + (idx as f64)) * width_per_piece) {
                let polygon = pl.make_polygons(width_per_piece);
                draw.unzoomed.push(color.alpha(0.8), polygon.clone());
                draw.unzoomed.push(color.alpha(0.5), polygon);
            }
        }
    }

    draw.build(ctx)
}
