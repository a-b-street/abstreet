use std::collections::{HashMap, HashSet};

use geom::{Angle, ArrowCap, Circle, Distance, PolyLine, Polygon};
use map_model::{IntersectionID, LaneID, Map, MovementID, TurnPriority, SIDEWALK_THICKNESS};
use widgetry::{Color, GeomBatch, Prerender};

use crate::colors::ColorScheme;
use crate::render::{traffic_signal, BIG_ARROW_THICKNESS};
use crate::AppLike;

const TURN_ICON_ARROW_LENGTH: Distance = Distance::const_meters(1.5);

pub struct DrawMovement {
    pub id: MovementID,
    pub hitbox: Polygon,
}

impl DrawMovement {
    // Also returns the stuff to draw each movement
    pub fn for_i(
        prerender: &Prerender,
        map: &Map,
        cs: &ColorScheme,
        i: IntersectionID,
        idx: usize,
    ) -> Vec<(DrawMovement, GeomBatch)> {
        let signal = map.get_traffic_signal(i);
        let stage = &signal.stages[idx];

        // TODO Sort by angle here if we want some consistency
        let mut offset_per_lane: HashMap<LaneID, usize> = HashMap::new();
        let mut results = Vec::new();
        for movement in signal.movements.values() {
            let mut batch = GeomBatch::new();
            // TODO Refactor the slice_start/slice_end stuff from draw_signal_stage.
            let hitbox = if stage.protected_movements.contains(&movement.id) {
                if movement.id.crosswalk {
                    batch = traffic_signal::walk_icon(movement, prerender);
                    batch.unioned_polygon()
                } else {
                    let arrow = movement
                        .geom
                        .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle);
                    batch.push(cs.signal_protected_turn, arrow.clone());
                    if let Ok(p) = arrow.to_outline(Distance::meters(0.2)) {
                        batch.push(Color::BLACK, p);
                    }
                    arrow
                }
            } else if stage.yield_movements.contains(&movement.id) {
                let pl = &movement.geom;
                // We currently always assume the turn intersects a crosswalk at the beginning and
                // end, so draw without overlaps if the polyline is long enough.
                if pl.length() >= 2.0 * SIDEWALK_THICKNESS {
                    batch.extend(
                        Color::BLACK,
                        pl.exact_slice(
                            SIDEWALK_THICKNESS - Distance::meters(0.1),
                            pl.length() - SIDEWALK_THICKNESS + Distance::meters(0.1),
                        )
                        .dashed_arrow(
                            BIG_ARROW_THICKNESS,
                            Distance::meters(1.2),
                            Distance::meters(0.3),
                            ArrowCap::Triangle,
                        ),
                    );
                    let arrow = pl
                        .exact_slice(SIDEWALK_THICKNESS, pl.length() - SIDEWALK_THICKNESS)
                        .dashed_arrow(
                            BIG_ARROW_THICKNESS / 2.0,
                            Distance::meters(1.0),
                            Distance::meters(0.5),
                            ArrowCap::Triangle,
                        );
                    batch.extend(cs.signal_protected_turn, arrow.clone());
                } else {
                    // TODO These turns are often too small to even dash the arrow. So they'll just
                    // look like solid protected turns...
                    warn!(
                        "{:?} is too short to render as a yield movement",
                        movement.id
                    );
                    batch.extend(
                        cs.signal_protected_turn,
                        pl.dashed_arrow(
                            BIG_ARROW_THICKNESS / 2.0,
                            Distance::meters(1.0),
                            Distance::meters(0.5),
                            ArrowCap::Triangle,
                        ),
                    );
                }
                // Bit weird, but don't use the dashed arrow as the hitbox. The gaps in between
                // should still be clickable.
                movement
                    .geom
                    .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle)
            } else if movement.id.crosswalk {
                batch = traffic_signal::dont_walk_icon(movement, prerender);
                batch.unioned_polygon()
            } else {
                // Use circular icons for banned turns
                let offset = movement
                    .members
                    .iter()
                    .map(|t| *offset_per_lane.entry(t.src).or_insert(0))
                    .max()
                    .unwrap();
                let (pl, _) = movement.src_center_and_width(map);
                let (circle, arrow) = make_circle_geom(offset as f64, pl, movement.angle);
                let mut seen_lanes = HashSet::new();
                for t in &movement.members {
                    if !seen_lanes.contains(&t.src) {
                        *offset_per_lane.get_mut(&t.src).unwrap() = offset + 1;
                        seen_lanes.insert(t.src);
                    }
                }
                batch.push(cs.signal_banned_turn.alpha(0.5), circle.clone());
                batch.push(Color::WHITE, arrow);
                circle
            };
            results.push((
                DrawMovement {
                    id: movement.id,
                    hitbox,
                },
                batch,
            ));
        }
        results
    }

    pub fn draw_selected_movement(
        &self,
        app: &dyn AppLike,
        batch: &mut GeomBatch,
        next_priority: Option<TurnPriority>,
    ) {
        let movement = &app.map().get_traffic_signal(self.id.parent).movements[&self.id];
        let pl = &movement.geom;

        let green = Color::hex("#72CE36");
        match next_priority {
            Some(TurnPriority::Protected) => {
                let arrow = pl.make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle);
                batch.push(green.alpha(0.5), arrow.clone());
                if let Ok(p) = arrow.to_outline(Distance::meters(0.1)) {
                    batch.push(green, p);
                }
            }
            Some(TurnPriority::Yield) => {
                batch.extend(
                    // TODO Ideally the inner part would be the lower opacity green, but can't yet
                    // express that it should cover up the thicker solid blue beneath it
                    Color::BLACK.alpha(0.8),
                    pl.dashed_arrow(
                        BIG_ARROW_THICKNESS,
                        Distance::meters(1.2),
                        Distance::meters(0.3),
                        ArrowCap::Triangle,
                    ),
                );
                batch.extend(
                    green.alpha(0.8),
                    pl.exact_slice(Distance::meters(0.1), pl.length() - Distance::meters(0.1))
                        .dashed_arrow(
                            BIG_ARROW_THICKNESS / 2.0,
                            Distance::meters(1.0),
                            Distance::meters(0.5),
                            ArrowCap::Triangle,
                        ),
                );
            }
            Some(TurnPriority::Banned) => {
                batch.extend(
                    Color::BLACK.alpha(0.8),
                    pl.dashed_arrow(
                        BIG_ARROW_THICKNESS,
                        Distance::meters(1.2),
                        Distance::meters(0.3),
                        ArrowCap::Triangle,
                    ),
                );
                batch.extend(
                    app.cs().signal_banned_turn.alpha(0.8),
                    pl.exact_slice(Distance::meters(0.1), pl.length() - Distance::meters(0.1))
                        .dashed_arrow(
                            BIG_ARROW_THICKNESS / 2.0,
                            Distance::meters(1.0),
                            Distance::meters(0.5),
                            ArrowCap::Triangle,
                        ),
                );
            }
            None => {}
        }
    }
}

// Produces (circle, arrow)
fn make_circle_geom(offset: f64, pl: PolyLine, angle: Angle) -> (Polygon, Polygon) {
    let height = 2.0 * TURN_ICON_ARROW_LENGTH;
    // Always extend the pl first to handle short entry lanes
    let extension = PolyLine::must_new(vec![
        pl.last_pt(),
        pl.last_pt()
            .project_away(Distance::meters(500.0), pl.last_line().angle()),
    ]);
    let pl = pl.must_extend(extension);
    let slice = pl.exact_slice(offset * height, (offset + 1.0) * height);
    let center = slice.middle();
    let block = Circle::new(center, TURN_ICON_ARROW_LENGTH).to_polygon();

    let arrow = PolyLine::must_new(vec![
        center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle.opposite()),
        center.project_away(TURN_ICON_ARROW_LENGTH / 2.0, angle),
    ])
    .make_arrow(Distance::meters(0.5), ArrowCap::Triangle);

    (block, arrow)
}
