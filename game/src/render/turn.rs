use crate::app::App;
use crate::render::BIG_ARROW_THICKNESS;
use geom::{Angle, ArrowCap, Distance, PolyLine, Polygon};
use map_model::{
    IntersectionCluster, IntersectionID, LaneID, Map, Movement, MovementID, TurnPriority,
    UberTurnGroup,
};
use std::collections::{HashMap, HashSet};
use widgetry::{Color, GeomBatch};

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

    pub fn draw_selected_movement(
        &self,
        app: &App,
        batch: &mut GeomBatch,
        m: &Movement,
        next_priority: Option<TurnPriority>,
    ) {
        // TODO Refactor this mess. Maybe after things like "dashed with outline" can be expressed
        // more composably like SVG, using lyon.
        let block_color = match next_priority {
            Some(TurnPriority::Protected) => {
                let green = Color::hex("#72CE36");
                let arrow = m.geom.make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle);
                batch.push(green.alpha(0.5), arrow.clone());
                if let Ok(p) = arrow.to_outline(Distance::meters(0.1)) {
                    batch.push(green, p);
                }
                green
            }
            Some(TurnPriority::Yield) => {
                batch.extend(
                    // TODO Ideally the inner part would be the lower opacity blue, but can't yet
                    // express that it should cover up the thicker solid blue beneath it
                    Color::BLACK.alpha(0.8),
                    m.geom.dashed_arrow(
                        BIG_ARROW_THICKNESS,
                        Distance::meters(1.2),
                        Distance::meters(0.3),
                        ArrowCap::Triangle,
                    ),
                );
                batch.extend(
                    app.cs.signal_permitted_turn.alpha(0.8),
                    m.geom
                        .exact_slice(
                            Distance::meters(0.1),
                            m.geom.length() - Distance::meters(0.1),
                        )
                        .dashed_arrow(
                            BIG_ARROW_THICKNESS / 2.0,
                            Distance::meters(1.0),
                            Distance::meters(0.5),
                            ArrowCap::Triangle,
                        ),
                );
                app.cs.signal_permitted_turn
            }
            Some(TurnPriority::Banned) => {
                let red = Color::hex("#EB3223");
                let arrow = m.geom.make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle);
                batch.push(red.alpha(0.5), arrow.clone());
                if let Ok(p) = arrow.to_outline(Distance::meters(0.1)) {
                    batch.push(red, p);
                }
                red
            }
            None => app.cs.signal_turn_block_bg,
        };
        batch.push(block_color, self.block.clone());
        batch.push(Color::WHITE, self.arrow.clone());
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
