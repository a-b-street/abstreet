use abstutil;
use ezgui::{Color, GfxCtx, Text};
use geom::{Polygon, Pt2D};
use map_model::{LaneID, Map};
use objects::{Ctx, DEBUG};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use render::DrawMap;
use sim::{Neighborhood, Sim, Tick};
use std::collections::HashSet;

pub struct NeighborhoodSummary {
    regions: Vec<Region>,
    active: bool,
    last_summary: Option<Tick>,
}

impl NeighborhoodSummary {
    pub fn new(map: &Map, draw_map: &DrawMap, timer: &mut abstutil::Timer) -> NeighborhoodSummary {
        let neighborhoods = abstutil::load_all_objects("neighborhoods", map.get_name());
        timer.start_iter("precompute neighborhood members", neighborhoods.len());
        NeighborhoodSummary {
            regions: neighborhoods
                .into_iter()
                .enumerate()
                .map(|(idx, (_, n))| {
                    timer.next();
                    Region::new(idx, n, map, draw_map)
                }).collect(),
            active: false,
            last_summary: None,
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

        if self.active && Some(ctx.primary.sim.time) != self.last_summary {
            self.last_summary = Some(ctx.primary.sim.time);
            for r in self.regions.iter_mut() {
                r.update_summary(
                    &ctx.primary.sim,
                    ctx.secondary.as_ref().map(|(s, _)| &s.sim),
                );
            }
        }

        self.active
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        if !self.active {
            return;
        }

        for r in &self.regions {
            g.draw_polygon(r.color, &r.polygon);
            // TODO ezgui should take borrows
            ctx.canvas.draw_text_at(g, r.summary.clone(), r.center);
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
    summary: Text,
}

impl Region {
    fn new(idx: usize, n: Neighborhood, map: &Map, draw_map: &DrawMap) -> Region {
        let center = Pt2D::center(&n.points);
        let polygon = Polygon::new(&n.points);
        // TODO polygon overlap or complete containment would be more ideal
        let lanes = draw_map
            .get_matching_lanes(polygon.get_bounds().as_bbox())
            .into_iter()
            .filter_map(|id| {
                let l = map.get_l(id);
                if polygon.contains_pt(l.first_pt()) && polygon.contains_pt(l.last_pt()) {
                    Some(id)
                } else {
                    None
                }
            }).collect();
        let mut summary = Text::new();
        summary.add_line(format!("{} - no summary yet", n.name));
        Region {
            name: n.name.clone(),
            polygon,
            center,
            lanes,
            color: COLORS[idx % COLORS.len()],
            summary,
        }
    }

    fn update_summary(&mut self, primary: &Sim, maybe_secondary: Option<&Sim>) {
        let mut txt = Text::new();
        txt.add_line(format!("{} has {} lanes", self.name, self.lanes.len()));

        if let Some(secondary) = maybe_secondary {
            // TODO colors
        } else {
            let s = primary.summarize(&self.lanes);

            txt.add_line(format!(
                "{} cars parked, {} spots free",
                s.cars_parked, s.open_parking_spots
            ));
            txt.add_line(format!(
                "{} moving cars, {} stuck",
                s.moving_cars, s.stuck_cars
            ));
            txt.add_line(format!(
                "{} moving peds, {} stuck",
                s.moving_peds, s.stuck_peds
            ));
            txt.add_line(format!("{} buses", s.buses));
        }

        self.summary = txt;
    }
}

const COLORS: [Color; 3] = [
    // TODO these are awful choices
    Color([1.0, 0.0, 0.0, 0.8]),
    Color([0.0, 1.0, 0.0, 0.8]),
    Color([0.0, 0.0, 1.0, 0.8]),
];
