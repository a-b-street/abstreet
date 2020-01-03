use crate::common::{CommonState, RoadColorer, RoadColorerBuilder};
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{Choice, Color, EventCtx, GeomBatch, GfxCtx, Key, Line, Text, WarpingItemSlider};
use geom::{Circle, Distance, Pt2D};
use map_model::{BusRoute, BusRouteID, BusStopID, PathConstraints, PathRequest, PathStep};

pub struct ShowBusRoute {
    colorer: RoadColorer,
    labels: Vec<(Text, Pt2D)>,
    bus_locations: Vec<Pt2D>,
}

pub struct BusRouteExplorer {
    slider: WarpingItemSlider<BusStopID>,
    show: ShowBusRoute,
}

impl ShowBusRoute {
    pub fn new(route: &BusRoute, ui: &UI, ctx: &mut EventCtx) -> ShowBusRoute {
        let map = &ui.primary.map;

        let mut bus_locations = Vec::new();
        for (_, pt) in ui.primary.sim.location_of_buses(route.id, map) {
            bus_locations.push(pt);
        }

        let mut txt = Text::from(Line(&route.name));
        txt.add(Line(format!("{} buses", bus_locations.len())));
        let mut colorer = RoadColorerBuilder::new(txt, vec![("route", Color::RED)]);
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
                    colorer.add(*l, Color::RED, map);
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

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.colorer.draw(g, ui);
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
}

impl BusRouteExplorer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<Box<dyn State>> {
        let map = &ui.primary.map;
        let (bs, routes) = match ui.primary.current_selection {
            Some(ID::BusStop(bs)) => (bs, map.get_routes_serving_stop(bs)),
            _ => {
                return None;
            }
        };
        if routes.is_empty() {
            return None;
        }
        if !ui.per_obj.action(ctx, Key::E, "explore bus route") {
            return None;
        }
        if routes.len() == 1 {
            Some(Box::new(BusRouteExplorer::for_route(
                routes[0],
                Some(bs),
                ui,
                ctx,
            )))
        } else {
            Some(make_bus_route_picker(
                routes.into_iter().map(|r| r.id).collect(),
                Some(bs),
            ))
        }
    }

    pub fn for_route(
        route: &BusRoute,
        start: Option<BusStopID>,
        ui: &UI,
        ctx: &mut EventCtx,
    ) -> BusRouteExplorer {
        let stops: Vec<(Pt2D, BusStopID, Text)> = route
            .stops
            .iter()
            .map(|bs| {
                let stop = ui.primary.map.get_bs(*bs);
                (stop.sidewalk_pos.pt(&ui.primary.map), stop.id, Text::new())
            })
            .collect();
        let mut slider = WarpingItemSlider::new(
            stops,
            &format!("Bus Route Explorer for {}", route.name),
            "stop",
            ctx,
        );
        if let Some(bs) = start {
            slider.override_initial_value(bs, ctx);
        }

        BusRouteExplorer {
            slider,
            show: ShowBusRoute::new(route, ui, ctx),
        }
    }
}

impl State for BusRouteExplorer {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.redo_mouseover() {
            // TODO Or use what debug mode is showing?
            ui.recalculate_current_selection(ctx);
        }
        ctx.canvas.handle_event(ctx.input);

        if let Some((evmode, done_warping)) = self.slider.event(ctx) {
            if done_warping {
                ui.primary.current_selection = Some(ID::BusStop(*self.slider.get().1));
            }
            Transition::KeepWithMode(evmode)
        } else {
            Transition::Pop
        }
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.show.draw(g, ui);
        self.slider.draw(g);
        CommonState::draw_osd(g, ui, &ui.primary.current_selection);
    }
}

fn make_bus_route_picker(routes: Vec<BusRouteID>, start: Option<BusStopID>) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, ui| {
        let (_, id) = wiz.wrap(ctx).choose("Explore which bus route?", || {
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
        })?;
        Some(Transition::Replace(Box::new(BusRouteExplorer::for_route(
            ui.primary.map.get_br(id),
            start,
            ui,
            ctx,
        ))))
    }))
}
