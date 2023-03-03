use std::collections::BTreeMap;

use geom::{Distance, Pt2D, Ring};
use map_model::{CommonEndpoint, Direction, PathStepV2, PathV2, RoadID};
use widgetry::mapspace::{ToggleZoomed, ToggleZoomedBuilder};
use widgetry::Color;

use crate::AppLike;

pub fn draw_overlapping_paths(
    app: &dyn AppLike,
    paths: Vec<(PathV2, Color)>,
) -> ToggleZoomedBuilder {
    // Per road, just figure out what colors we need and whether to use the full road or start/end
    // mid-way through
    let mut colors_per_road: BTreeMap<RoadID, Vec<(Color, Option<DistanceInterval>)>> =
        BTreeMap::new();
    let mut colors_per_movement: Vec<(RoadID, RoadID, Color)> = Vec::new();
    for (path, color) in paths {
        for (idx, step) in path.get_steps().iter().enumerate() {
            match step {
                PathStepV2::Along(dr) | PathStepV2::Contraflow(dr) => {
                    let road_len = app.map().get_r(dr.road).length();
                    // TODO Handle Contraflow. Doesn't it just invert the direction we check?
                    let interval = if idx == 0 {
                        if dr.dir == Direction::Fwd {
                            Some(DistanceInterval {
                                start: path.get_req().start.dist_along(),
                                end: road_len,
                            })
                        } else {
                            Some(DistanceInterval {
                                start: Distance::ZERO,
                                // TODO I'm not sure why this is necessary, or if it's always
                                // correct. In one case where req.start comes from alt_start on the
                                // opposite side of the road, it's needed -- maybe meaning
                                // equiv_pos is broken.
                                end: road_len - path.get_req().start.dist_along(),
                            })
                        }
                    } else if idx == path.get_steps().len() - 1 {
                        if dr.dir == Direction::Fwd {
                            Some(DistanceInterval {
                                start: Distance::ZERO,
                                end: path.get_req().end.dist_along(),
                            })
                        } else {
                            Some(DistanceInterval {
                                // TODO Same as above -- this works, but I don't know why.
                                start: road_len - path.get_req().end.dist_along(),
                                end: road_len,
                            })
                        }
                    } else {
                        None
                    };
                    colors_per_road
                        .entry(dr.road)
                        .or_insert_with(Vec::new)
                        .push((color, interval));
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

    // Per road and color, mark the 4 corners of the thickened polyline.
    // (beginning left, beginning right, end left, end right)
    // TODO Make Color implement Ord; use hex in the meantime
    let mut pieces: BTreeMap<(RoadID, String), (Pt2D, Pt2D, Pt2D, Pt2D)> = BTreeMap::new();
    // Per road, divide the needed colors proportionally
    let mut draw = ToggleZoomed::builder();
    for (road, colors) in colors_per_road {
        let road = app.map().get_r(road);
        let width_per_piece = road.get_width() / (colors.len() as f64);
        for (idx, (color, interval)) in colors.into_iter().enumerate() {
            // Don't directly use road.shift_from_left_side, since we maybe need to clip
            let center_line = if let Some(interval) = interval {
                road.center_pts
                    .maybe_exact_slice(interval.start, interval.end)
            } else {
                Ok(road.center_pts.clone())
            };
            if let Ok(pl) = center_line.and_then(|pl| {
                pl.shift_from_center(road.get_width(), (0.5 + (idx as f64)) * width_per_piece)
            }) {
                let polygon = pl.make_polygons(width_per_piece);
                draw.unzoomed.push(color.alpha(0.8), polygon.clone());
                draw.zoomed.push(color.alpha(0.5), polygon);

                // Reproduce what make_polygons does to get the 4 corners
                if let Some(corners) = pl.get_four_corners_of_thickened(width_per_piece) {
                    pieces.insert((road.id, color.as_hex()), corners);
                }
            }
        }
    }

    // Fill in intersections
    for (from, to, color) in colors_per_movement {
        if let Some(from_corners) = pieces.get(&(from, color.as_hex())) {
            if let Some(to_corners) = pieces.get(&(to, color.as_hex())) {
                let from_road = app.map().get_r(from);
                let to_road = app.map().get_r(to);
                if let CommonEndpoint::One(i) = from_road.common_endpoint(to_road) {
                    let (from_left, from_right) = if from_road.src_i == i {
                        (from_corners.0, from_corners.1)
                    } else {
                        (from_corners.2, from_corners.3)
                    };
                    let (to_left, to_right) = if to_road.src_i == i {
                        (to_corners.0, to_corners.1)
                    } else {
                        (to_corners.2, to_corners.3)
                    };
                    // Glue the 4 corners together
                    if let Ok(ring) =
                        Ring::new(vec![from_left, from_right, to_right, to_left, from_left])
                    {
                        let polygon = ring.into_polygon();
                        draw.unzoomed.push(color.alpha(0.8), polygon.clone());
                        draw.zoomed.push(color.alpha(0.5), polygon);
                    }
                }
            }
        }
    }

    draw
}

struct DistanceInterval {
    start: Distance,
    end: Distance,
}
