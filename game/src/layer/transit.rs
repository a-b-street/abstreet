use crate::app::App;
use crate::common::ColorDiscrete;
use crate::layer::{Layer, LayerOutcome};
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Pt2D, Time};
use map_model::{BusRouteID, PathConstraints, PathRequest, PathStep};

pub struct TransitNetwork {
    composite: Composite,
    unzoomed: Drawable,
    zoomed: Drawable,
    show_all_routes: bool,
    show_buses: bool,
    show_trains: bool,
}

impl Layer for TransitNetwork {
    fn name(&self) -> Option<&'static str> {
        Some("transit network")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome> {
        self.composite.align_above(ctx, minimap);
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            None => {
                let new_show_all_routes = self.composite.is_checked("show all routes");
                let new_show_buses = self.composite.is_checked("show buses");
                let new_show_trains = self.composite.is_checked("show trains");
                if (new_show_all_routes, new_show_buses, new_show_trains)
                    != (self.show_all_routes, self.show_buses, self.show_trains)
                {
                    *self = TransitNetwork::new(
                        ctx,
                        app,
                        new_show_all_routes,
                        new_show_buses,
                        new_show_trains,
                    );
                    self.composite.align_above(ctx, minimap);
                }
            }
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl TransitNetwork {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        show_all_routes: bool,
        show_buses: bool,
        show_trains: bool,
    ) -> TransitNetwork {
        let map = &app.primary.map;
        // TODO Same color for both?
        let mut categories = vec![
            ("bus lanes / rails", app.cs.bus_layer),
            ("transit stops", app.cs.bus_layer),
        ];
        if show_all_routes {
            categories.push(("routes", app.cs.bus_layer));
        }
        let mut colorer = ColorDiscrete::new(app, categories);
        for l in map.all_lanes() {
            if l.is_bus() && show_buses {
                colorer.add_l(l.id, "bus lanes / rails");
            }
            if l.is_light_rail() && show_trains {
                colorer.add_l(l.id, "bus lanes / rails");
            }
        }
        for bs in map.all_bus_stops().values() {
            if !bs.is_train_stop && show_buses {
                colorer.add_bs(bs.id, "transit stops");
            }
            if bs.is_train_stop && show_trains {
                colorer.add_bs(bs.id, "transit stops");
            }
        }
        if show_all_routes {
            for br in map.all_bus_routes() {
                if !show_buses && br.route_type == PathConstraints::Bus {
                    continue;
                }
                if !show_trains && br.route_type == PathConstraints::Train {
                    continue;
                }
                for (bs1, bs2) in loop_pairs(&br.stops) {
                    if let Some(path) = map.pathfind(PathRequest {
                        start: map.get_bs(bs1).driving_pos,
                        end: map.get_bs(bs2).driving_pos,
                        constraints: br.route_type,
                    }) {
                        for step in path.get_steps() {
                            if let PathStep::Lane(l) = step {
                                colorer.add_l(*l, "routes");
                            }
                        }
                    }
                }
            }
        }
        let (unzoomed, zoomed, legend) = colorer.build(ctx);

        let composite = Composite::new(Widget::col(vec![
            Widget::row(vec![
                Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                "Bus network".draw_text(ctx),
                Btn::plaintext("X")
                    .build(ctx, "close", hotkey(Key::Escape))
                    .align_right(),
            ]),
            Checkbox::text(ctx, "show all routes", None, show_all_routes),
            Checkbox::text(ctx, "show buses", None, show_buses),
            Checkbox::text(ctx, "show trains", None, show_trains),
            legend,
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        TransitNetwork {
            composite,
            unzoomed,
            zoomed,
            show_all_routes,
            show_buses,
            show_trains,
        }
    }
}

// TODO This maybe shouldn't be a layer
pub struct ShowTransitRoute {
    time: Time,
    route: BusRouteID,
    labels: Vec<(Text, Pt2D)>,
    bus_locations: Vec<Pt2D>,

    composite: Composite,
    unzoomed: Drawable,
    zoomed: Drawable,
}

impl Layer for ShowTransitRoute {
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
            *self = ShowTransitRoute::new(ctx, app, self.route);
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

impl ShowTransitRoute {
    pub fn new(ctx: &mut EventCtx, app: &App, id: BusRouteID) -> ShowTransitRoute {
        let map = &app.primary.map;
        let route = app.primary.map.get_br(id);

        let mut bus_locations = Vec::new();
        for (_, pt) in app.primary.sim.location_of_buses(id, map) {
            bus_locations.push(pt);
        }

        let mut categories = vec![("route", app.cs.unzoomed_bus)];
        if route.start_border.is_some() {
            categories.push(("start", Color::RED));
        }
        if route.end_border.is_some() {
            categories.push(("end", Color::GREEN));
        }
        let mut colorer = ColorDiscrete::new(app, categories);
        if let Some(l) = route.start_border {
            colorer.add_i(map.get_l(l).src_i, "start");
        }
        if let Some(l) = route.end_border {
            colorer.add_i(map.get_l(l).dst_i, "end");
        }
        for pair in route.stops.windows(2) {
            for step in map
                .pathfind(PathRequest {
                    start: map.get_bs(pair[0]).driving_pos,
                    end: map.get_bs(pair[1]).driving_pos,
                    constraints: route.route_type,
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
        for bs in &route.stops {
            let bs = map.get_bs(*bs);
            labels.push((
                Text::from(Line(&bs.name)).with_bg(),
                bs.sidewalk_pos.pt(map),
            ));
        }

        let (unzoomed, zoomed, legend) = colorer.build(ctx);
        ShowTransitRoute {
            time: app.primary.sim.time(),
            route: id,
            labels,
            unzoomed,
            zoomed,
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                    Line(&route.full_name).draw(ctx),
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

// TODO Use elsewhere
fn loop_pairs<T: Copy>(list: &Vec<T>) -> Vec<(T, T)> {
    let mut pairs = Vec::new();
    for pair in list.windows(2) {
        pairs.push((pair[0], pair[1]));
    }
    pairs.push((*list.last().unwrap(), list[0]));
    pairs
}
