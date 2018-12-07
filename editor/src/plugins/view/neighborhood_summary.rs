use crate::objects::{Ctx, DEBUG};
use crate::plugins::{Plugin, PluginCtx};
use crate::render::DrawMap;
use abstutil;
use ezgui::{Color, GfxCtx, Text};
use geom::{Polygon, Pt2D};
use map_model::{LaneID, Map};
use piston::input::Key;
use sim::{Neighborhood, Sim, Tick};
use std::collections::HashSet;

pub struct NeighborhoodSummary {
    regions: Vec<Region>,
    active: bool,
    last_summary: Option<Tick>,
    key: Key,
}

impl NeighborhoodSummary {
    pub fn new(
        key: Key,
        map: &Map,
        draw_map: &DrawMap,
        timer: &mut abstutil::Timer,
    ) -> NeighborhoodSummary {
        let neighborhoods = Neighborhood::load_all(map.get_name(), &map.get_gps_bounds());
        timer.start_iter("precompute neighborhood members", neighborhoods.len());
        NeighborhoodSummary {
            key,
            regions: neighborhoods
                .into_iter()
                .enumerate()
                .map(|(idx, (_, n))| {
                    timer.next();
                    Region::new(idx, n, map, draw_map)
                })
                .collect(),
            active: false,
            last_summary: None,
        }
    }
}

impl Plugin for NeighborhoodSummary {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        if self.active {
            if ctx
                .input
                .key_pressed(self.key, "stop showing neighborhood summaries")
            {
                self.active = false;
            }
        } else {
            self.active = ctx.primary.current_selection.is_none()
                && ctx.input.unimportant_key_pressed(
                    self.key,
                    DEBUG,
                    "show neighborhood summaries",
                );
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
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &mut Ctx) {
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
        let center = n.polygon.center();
        // TODO polygon overlap or complete containment would be more ideal
        let lanes = draw_map
            .get_matching_lanes(n.polygon.get_bounds())
            .into_iter()
            .filter_map(|id| {
                let l = map.get_l(id);
                if n.polygon.contains_pt(l.first_pt()) && n.polygon.contains_pt(l.last_pt()) {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();
        let mut summary = Text::new();
        summary.add_line(format!("{} - no summary yet", n.name));
        Region {
            name: n.name.clone(),
            polygon: n.polygon.clone(),
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
            let s1 = primary.summarize(&self.lanes);
            let s2 = secondary.summarize(&self.lanes);

            txt.add_line(format!(
                "{}|{} cars parked, {}|{} spots free",
                s1.cars_parked, s2.cars_parked, s1.open_parking_spots, s2.open_parking_spots
            ));
            txt.add_line(format!(
                "{}|{} moving cars, {}|{} stuck",
                s1.moving_cars, s2.moving_cars, s1.stuck_cars, s2.stuck_cars
            ));
            txt.add_line(format!(
                "{}|{} moving peds, {}|{} stuck",
                s1.moving_peds, s2.moving_peds, s1.stuck_peds, s2.stuck_peds
            ));
            txt.add_line(format!("{}|{} buses", s1.buses, s2.buses));
        // TODO diff all in a region and provide the count
        } else {
            let s1 = primary.summarize(&self.lanes);

            txt.add_line(format!(
                "{} cars parked, {} spots free",
                s1.cars_parked, s1.open_parking_spots
            ));
            txt.add_line(format!(
                "{} moving cars, {} stuck",
                s1.moving_cars, s1.stuck_cars
            ));
            txt.add_line(format!(
                "{} moving peds, {} stuck",
                s1.moving_peds, s1.stuck_peds
            ));
            txt.add_line(format!("{} buses", s1.buses));
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
