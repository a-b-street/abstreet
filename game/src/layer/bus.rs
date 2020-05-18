use crate::app::App;
use crate::common::Colorer;
use crate::layer::{Layer, LayerOutcome};
use ezgui::{Color, Composite, EventCtx, GeomBatch, GfxCtx, Line, Text};
use geom::{Circle, Distance, Pt2D, Time};
use map_model::{BusRouteID, PathConstraints, PathRequest, PathStep};

// TODO This maybe shouldn't be a layer
pub struct ShowBusRoute {
    time: Time,
    route: BusRouteID,

    colorer: Colorer,
    labels: Vec<(Text, Pt2D)>,
    bus_locations: Vec<Pt2D>,
}

impl Layer for ShowBusRoute {
    fn name(&self) -> Option<&'static str> {
        None
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = ShowBusRoute::new(ctx, app, self.route);
        }

        self.colorer.legend.align_above(ctx, minimap);
        if self.colorer.event(ctx) {
            return Some(LayerOutcome::Close);
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.colorer.draw(g, app);

        let mut screen_batch = GeomBatch::new();
        for (label, pt) in &self.labels {
            screen_batch.add_centered(
                label.clone().render_g(g),
                g.canvas.map_to_screen(*pt).to_pt(),
            );
        }
        let draw = g.upload(screen_batch);
        g.fork_screenspace();
        g.redraw(&draw);
        g.unfork();

        let mut batch = GeomBatch::new();
        let radius = Distance::meters(20.0) / g.canvas.cam_zoom;
        for pt in &self.bus_locations {
            batch.push(Color::BLUE, Circle::new(*pt, radius).to_polygon());
        }
        batch.draw(g);
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.colorer.unzoomed);
    }
}

impl ShowBusRoute {
    pub fn new(ctx: &mut EventCtx, app: &App, id: BusRouteID) -> ShowBusRoute {
        let map = &app.primary.map;
        let route = app.primary.map.get_br(id);

        let mut bus_locations = Vec::new();
        for (_, pt) in app.primary.sim.location_of_buses(id, map) {
            bus_locations.push(pt);
        }

        let color = app.cs.unzoomed_bus;
        let mut colorer = Colorer::discrete(
            ctx,
            &route.name,
            vec![format!("{} buses", bus_locations.len())],
            vec![("route", color)],
        );
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
                    colorer.add_l(*l, color, map);
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
            time: app.primary.sim.time(),
            route: id,
            colorer: colorer.build_both(ctx, app),
            labels,
            bus_locations,
        }
    }
}
