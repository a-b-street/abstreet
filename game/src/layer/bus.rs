use crate::app::App;
use crate::common::ColorDiscrete;
use crate::layer::{Layer, LayerOutcome};
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Pt2D, Time};
use map_model::{BusRouteID, PathConstraints, PathRequest, PathStep};

// TODO This maybe shouldn't be a layer
pub struct ShowBusRoute {
    time: Time,
    route: BusRouteID,
    labels: Vec<(Text, Pt2D)>,
    bus_locations: Vec<Pt2D>,

    composite: Composite,
    unzoomed: Drawable,
    zoomed: Drawable,
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

        Layer::simple_event(ctx, minimap, &mut self.composite)
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        self.composite.draw(g);

        // TODO Do this once
        let mut screen_batch = GeomBatch::new();
        for (label, pt) in &self.labels {
            screen_batch.append(
                label
                    .clone()
                    .render_g(g)
                    .centered_on(g.canvas.map_to_screen(*pt).to_pt()),
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
        g.redraw(&self.unzoomed);
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

        let mut colorer = ColorDiscrete::new(app, vec![("route", app.cs.unzoomed_bus)]);
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
                    colorer.add_l(*l, "route");
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

        let (unzoomed, zoomed, legend) = colorer.build(ctx);
        ShowBusRoute {
            time: app.primary.sim.time(),
            route: id,
            labels,
            unzoomed,
            zoomed,
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg"),
                    Line(&route.name).draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                format!("{} buses", bus_locations.len()).draw_text(ctx),
                legend,
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
            .build(ctx),
            bus_locations,
        }
    }
}
