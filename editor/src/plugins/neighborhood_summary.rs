use abstutil;
use ezgui::{Color, GfxCtx, Text};
use geom::{Polygon, Pt2D};
use map_model::{LaneID, Map};
use objects::{Ctx, DEBUG};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::Neighborhood;
use std::collections::HashSet;

pub struct NeighborhoodSummary {
    regions: Vec<Region>,
    active: bool,
}

impl NeighborhoodSummary {
    pub fn new(map: &Map) -> NeighborhoodSummary {
        NeighborhoodSummary {
            regions: abstutil::load_all_objects("neighborhoods", map.get_name())
                .into_iter()
                .enumerate()
                .map(|(idx, (_, n))| Region::new(idx, n, map))
                .collect(),
            active: false,
        }
    }
}

impl Plugin for NeighborhoodSummary {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        if self.active {
            if ctx
                .input
                .key_pressed(Key::Z, "stop showing neighborhood summaries")
            {
                self.active = false;
            }
        } else {
            self.active = ctx.primary.current_selection.is_none() && ctx
                .input
                .unimportant_key_pressed(Key::Z, DEBUG, "show neighborhood summaries");
        }

        self.active
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        if !self.active {
            return;
        }

        for r in &self.regions {
            // TODO some text
            g.draw_polygon(r.color, &r.polygon);

            let mut txt = Text::new();
            txt.add_line(format!("{} has {} lanes", r.name, r.lanes.len()));
            ctx.canvas.draw_text_at(g, txt, r.center);
        }
    }
}

// sim::Neighborhood is already taken, just call it something different here. :\
struct Region {
    name: String,
    polygon: Polygon,
    center: Pt2D,
    lanes: HashSet<LaneID>,
    color: Color,
}

impl Region {
    fn new(idx: usize, n: Neighborhood, map: &Map) -> Region {
        let center = Pt2D::center(&n.points);
        let polygon = Polygon::new(&n.points);
        // TODO polygon overlap or complete containment would be more ideal
        let lanes = map
            .all_lanes()
            .iter()
            .filter_map(|l| {
                if polygon.contains_pt(l.first_pt()) && polygon.contains_pt(l.last_pt()) {
                    Some(l.id)
                } else {
                    None
                }
            }).collect();
        Region {
            name: n.name.clone(),
            polygon,
            center,
            lanes,
            color: COLORS[idx % COLORS.len()],
        }
    }
}

const COLORS: [Color; 3] = [
    // TODO these are awful choices
    [1.0, 0.0, 0.0, 0.8],
    [0.0, 1.0, 0.0, 0.8],
    [0.0, 0.0, 1.0, 0.8],
];
