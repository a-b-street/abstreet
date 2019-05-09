use crate::helpers::rotating_color;
use crate::render::DrawMap;
use crate::ui::UI;
use abstutil;
use ezgui::{Color, Drawable, GfxCtx, ModalMenu, Prerender, Text};
use geom::{Duration, Polygon, Pt2D};
use map_model::{LaneID, Map, Neighborhood};
use sim::Sim;
use std::collections::HashSet;

pub struct NeighborhoodSummary {
    regions: Vec<Region>,
    draw_all_regions: Drawable,
    pub active: bool,
    last_summary: Option<Duration>,
}

impl NeighborhoodSummary {
    pub fn new(
        map: &Map,
        draw_map: &DrawMap,
        prerender: &Prerender,
        timer: &mut abstutil::Timer,
    ) -> NeighborhoodSummary {
        let neighborhoods = Neighborhood::load_all(map.get_name(), &map.get_gps_bounds());
        timer.start_iter("precompute neighborhood members", neighborhoods.len());
        let regions: Vec<Region> = neighborhoods
            .into_iter()
            .enumerate()
            .map(|(idx, (_, n))| {
                timer.next();
                Region::new(idx, n, map, draw_map)
            })
            .collect();
        let draw_all_regions = prerender.upload_borrowed(
            regions
                .iter()
                .map(|r| (r.color, &r.polygon))
                .collect::<Vec<_>>(),
        );

        NeighborhoodSummary {
            regions,
            draw_all_regions,
            active: false,
            last_summary: None,
        }
    }

    pub fn event(&mut self, ui: &UI, menu: &mut ModalMenu) {
        if menu.action("show/hide neighborhood summaries") {
            self.active = !self.active;
        }

        if self.active && Some(ui.primary.sim.time()) != self.last_summary {
            self.last_summary = Some(ui.primary.sim.time());
            for r in self.regions.iter_mut() {
                r.update_summary(&ui.primary.sim);
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if !self.active {
            return;
        }

        g.redraw(&self.draw_all_regions);
        for r in &self.regions {
            g.draw_text_at_mapspace(&r.summary, r.center);
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
    fn new(idx: usize, n: Neighborhood, _map: &Map, _draw_map: &DrawMap) -> Region {
        let center = n.polygon.center();
        // TODO polygon overlap or complete containment would be more ideal
        // TODO Re-enable when this is useful; just causes slow start all the time
        /*let lanes = draw_map
        .get_matching_lanes(n.polygon.get_bounds())
        .into_iter()
        .filter(|id| {
            let l = map.get_l(*id);
            n.polygon.contains_pt(l.first_pt()) && n.polygon.contains_pt(l.last_pt())
        })
        .collect();*/
        Region {
            name: n.name.clone(),
            polygon: n.polygon.clone(),
            center,
            lanes: HashSet::new(),
            color: rotating_color(idx),
            summary: Text::from_line(format!("{} - no summary yet", n.name)),
        }
    }

    fn update_summary(&mut self, primary: &Sim) {
        let mut txt = Text::new();
        txt.add_styled_line(self.name.clone(), None, Some(Color::GREEN), Some(50));
        txt.add_line(format!("contains {} lanes", self.lanes.len()));

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

        self.summary = txt;
    }
}
