use crate::app::App;
use crate::common::ColorDiscrete;
use crate::layer::{Layer, LayerOutcome};
use ezgui::{
    hotkey, Btn, Checkbox, Composite, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key,
    Outcome, TextExt, VerticalAlignment, Widget,
};
use map_model::{PathConstraints, PathStep};

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
                for req in br.all_steps(map) {
                    if let Some(path) = map.pathfind(req) {
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
                "Transit network".draw_text(ctx),
                Btn::plaintext("X")
                    .build(ctx, "close", hotkey(Key::Escape))
                    .align_right(),
            ]),
            Checkbox::switch(ctx, "show all routes", None, show_all_routes),
            Checkbox::switch(ctx, "show buses", None, show_buses),
            Checkbox::switch(ctx, "show trains", None, show_trains),
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
