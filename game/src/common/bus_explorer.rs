use crate::common::{Colorer, ColorerBuilder, Overlays};
use crate::game::{State, Transition, WizardState};
use crate::ui::UI;
use ezgui::{Choice, Color, EventCtx, GeomBatch, GfxCtx, Line, Text};
use geom::{Circle, Distance, Pt2D};
use map_model::{BusRouteID, PathConstraints, PathRequest, PathStep};

pub struct ShowBusRoute {
    pub colorer: Colorer,
    labels: Vec<(Text, Pt2D)>,
    bus_locations: Vec<Pt2D>,
}

impl ShowBusRoute {
    pub fn new(id: BusRouteID, ctx: &mut EventCtx, ui: &UI) -> ShowBusRoute {
        let map = &ui.primary.map;
        let route = ui.primary.map.get_br(id);

        let mut bus_locations = Vec::new();
        for (_, pt) in ui.primary.sim.location_of_buses(id, map) {
            bus_locations.push(pt);
        }

        let mut txt = Text::from(Line(&route.name));
        txt.add(Line(format!("{} buses", bus_locations.len())));
        let mut colorer = ColorerBuilder::new(txt, vec![("route", Color::RED)]);
        for (stop1, stop2) in
            route
                .stops
                .iter()
                .zip(route.stops.iter().skip(1))
                .chain(std::iter::once((
                    route.stops.last().unwrap(),
                    &route.stops[0],
                )))
        {
            let bs1 = map.get_bs(*stop1);
            let bs2 = map.get_bs(*stop2);
            for step in map
                .pathfind(PathRequest {
                    start: bs1.driving_pos,
                    end: bs2.driving_pos,
                    constraints: PathConstraints::Bus,
                })
                .unwrap()
                .get_steps()
            {
                if let PathStep::Lane(l) = step {
                    colorer.add_l(*l, Color::RED, map);
                }
            }
        }

        let mut labels = Vec::new();
        for (idx, bs) in route.stops.iter().enumerate() {
            labels.push((
                Text::from(Line(format!("{}", idx + 1))).with_bg(),
                map.get_bs(*bs).sidewalk_pos.pt(map),
            ));
        }

        ShowBusRoute {
            colorer: colorer.build(ctx, ui),
            labels,
            bus_locations,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.colorer.draw(g);
        for (label, pt) in &self.labels {
            g.draw_text_at(label, *pt);
        }

        let mut batch = GeomBatch::new();
        let radius = Distance::meters(20.0) / g.canvas.cam_zoom;
        for pt in &self.bus_locations {
            batch.push(Color::BLUE, Circle::new(*pt, radius).to_polygon());
        }
        batch.draw(g);
    }

    pub fn make_route_picker(routes: Vec<BusRouteID>, from_sandbox_mode: bool) -> Box<dyn State> {
        let show_route = "show the route";
        let delays = "delays between stops";
        let passengers = "passengers waiting at each stop";

        WizardState::new(Box::new(move |wiz, ctx, ui| {
            let mut wizard = wiz.wrap(ctx);

            let id = if routes.len() == 1 {
                routes[0]
            } else {
                wizard
                    .choose("Explore which bus route?", || {
                        let mut choices: Vec<(&String, BusRouteID)> = routes
                            .iter()
                            .map(|id| (&ui.primary.map.get_br(*id).name, *id))
                            .collect();
                        // TODO Sort first by length, then lexicographically
                        choices.sort_by_key(|(name, _)| name.to_string());
                        choices
                            .into_iter()
                            .map(|(name, id)| Choice::new(name, id))
                            .collect()
                    })?
                    .1
            };
            let choice = wizard
                .choose_string("What do you want to see about this route?", || {
                    vec![show_route, delays, passengers]
                })?;
            ui.overlay = match choice {
                x if x == show_route => Overlays::show_bus_route(id, ctx, ui),
                x if x == delays => Overlays::delays_over_time(id, ctx, ui),
                x if x == passengers => Overlays::bus_passengers(id, ctx, ui),
                _ => unreachable!(),
            };
            if from_sandbox_mode {
                Some(Transition::Pop)
            } else {
                Some(Transition::PopTwice)
            }
        }))
    }
}
