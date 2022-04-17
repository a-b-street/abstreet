use map_gui::tools::ColorDiscrete;
use map_model::{PathConstraints, PathStep};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{EventCtx, GfxCtx, Outcome, Panel, Toggle, Widget};

use crate::app::App;
use crate::layer::{header, Layer, LayerOutcome, PANEL_PLACEMENT};

pub struct TransitNetwork {
    panel: Panel,
    draw: ToggleZoomed,
}

impl Layer for TransitNetwork {
    fn name(&self) -> Option<&'static str> {
        Some("transit network")
    }
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                *self = TransitNetwork::new(
                    ctx,
                    app,
                    self.panel.is_checked("show all routes"),
                    self.panel.is_checked("show buses"),
                    self.panel.is_checked("show trains"),
                );
            }
            _ => {}
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.draw.draw(g);
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw.unzoomed);
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
        for ts in map.all_transit_stops().values() {
            if !ts.is_train_stop && show_buses {
                colorer.add_ts(ts.id, "transit stops");
            }
            if ts.is_train_stop && show_trains {
                colorer.add_ts(ts.id, "transit stops");
            }
        }
        if show_all_routes {
            for tr in map.all_transit_routes() {
                if !show_buses && tr.route_type == PathConstraints::Bus {
                    continue;
                }
                if !show_trains && tr.route_type == PathConstraints::Train {
                    continue;
                }
                for path in tr.all_paths(map).unwrap() {
                    for step in path.get_steps() {
                        if let PathStep::Lane(l) = step {
                            colorer.add_l(*l, "routes");
                        }
                    }
                }
            }
        }
        let (draw, legend) = colorer.build(ctx);

        let panel = Panel::new_builder(Widget::col(vec![
            header(ctx, "Transit network"),
            Toggle::switch(ctx, "show all routes", None, show_all_routes),
            Toggle::switch(ctx, "show buses", None, show_buses),
            Toggle::switch(ctx, "show trains", None, show_trains),
            legend,
        ]))
        .aligned_pair(PANEL_PLACEMENT)
        .build(ctx);

        TransitNetwork { panel, draw }
    }
}
