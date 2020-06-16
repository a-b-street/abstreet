use crate::app::App;
use crate::common::{ColorLegend, ColorNetwork, Colorer};
use crate::helpers::amenity_type;
use crate::layer::{Layer, LayerOutcome};
use abstutil::Counter;
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Distance, Time};
use map_model::LaneType;
use sim::TripMode;

pub struct BikeNetwork {
    composite: Composite,
    time: Time,
    unzoomed: Drawable,
    zoomed: Drawable,
}

impl Layer for BikeNetwork {
    fn name(&self) -> Option<&'static str> {
        Some("bike network")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = BikeNetwork::new(ctx, app);
        }

        self.composite.align_above(ctx, minimap);
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            None => {}
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

impl BikeNetwork {
    pub fn new(ctx: &mut EventCtx, app: &App) -> BikeNetwork {
        let mut num_lanes = 0;
        let mut total_dist = Distance::ZERO;
        let mut on_bike_lanes = Counter::new();
        let mut off_bike_lanes = Counter::new();
        let mut intersections_on = Counter::new();
        let mut intersections_off = Counter::new();
        // Make sure all bikes lanes show up no matter what
        for l in app.primary.map.all_lanes() {
            if l.is_biking() {
                on_bike_lanes.add(l.parent, 0);
                intersections_on.add(l.src_i, 0);
                intersections_on.add(l.src_i, 0);
                num_lanes += 1;
                total_dist += l.length();
            }
        }

        // Show throughput, broken down by bike lanes or not
        for ((r, mode, _), count) in &app.primary.sim.get_analytics().road_thruput.counts {
            if *mode == TripMode::Bike {
                let (fwd, back) = app.primary.map.get_r(*r).get_lane_types();
                if fwd.contains(&LaneType::Biking) || back.contains(&LaneType::Biking) {
                    on_bike_lanes.add(*r, *count);
                } else {
                    off_bike_lanes.add(*r, *count);
                }
            }
        }

        // Use intersection data too, but bin as on bike lanes or not based on connecting roads
        for ((i, mode, _), count) in &app.primary.sim.get_analytics().intersection_thruput.counts {
            if *mode == TripMode::Bike {
                if app
                    .primary
                    .map
                    .get_i(*i)
                    .roads
                    .iter()
                    .any(|r| on_bike_lanes.get(*r) > 0)
                {
                    intersections_on.add(*i, *count);
                } else {
                    intersections_off.add(*i, *count);
                }
            }
        }

        let composite = Composite::new(
            Widget::col(vec![
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg")
                        .margin_right(10),
                    "Bike network".draw_text(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Text::from_multiline(vec![
                    Line(format!("{} lanes", num_lanes)),
                    Line(format!("total distance of {}", total_dist)),
                ])
                .draw(ctx)
                .margin_below(10),
                Line("Throughput on bike lanes").draw(ctx),
                ColorLegend::gradient(
                    ctx,
                    vec![app.cs.good_green, app.cs.bad_green],
                    vec!["0%ile", "100%ile"],
                ),
                Line("Throughput on unprotected roads").draw(ctx),
                ColorLegend::gradient(
                    ctx,
                    vec![app.cs.good_red, app.cs.bad_red],
                    vec!["0%ile", "100%ile"],
                ),
            ])
            .padding(5)
            .bg(app.cs.panel_bg),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        let mut colorer = ColorNetwork::new(app);
        colorer.road_percentiles(on_bike_lanes, app.cs.good_green, app.cs.bad_green);
        colorer.road_percentiles(off_bike_lanes, app.cs.good_red, app.cs.bad_red);
        colorer.intersection_percentiles(intersections_on, app.cs.good_green, app.cs.bad_green);
        colorer.intersection_percentiles(intersections_off, app.cs.good_red, app.cs.bad_red);
        let (unzoomed, zoomed) = colorer.build(ctx);

        BikeNetwork {
            composite,
            time: app.primary.sim.time(),
            unzoomed,
            zoomed,
        }
    }
}

pub struct Static {
    pub colorer: Colorer,
    name: &'static str,
}

impl Layer for Static {
    fn name(&self) -> Option<&'static str> {
        Some(self.name)
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        _: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome> {
        self.colorer.legend.align_above(ctx, minimap);
        if self.colorer.event(ctx) {
            return Some(LayerOutcome::Close);
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.colorer.draw(g, app);
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.colorer.unzoomed);
    }
}

impl Static {
    pub fn bus_network(ctx: &mut EventCtx, app: &App) -> Static {
        // TODO Same color for both?
        let mut colorer = Colorer::discrete(
            ctx,
            "Bus network",
            Vec::new(),
            vec![
                ("bus lanes", app.cs.bus_layer),
                ("bus stops", app.cs.bus_layer),
            ],
        );
        for l in app.primary.map.all_lanes() {
            if l.is_bus() {
                colorer.add_l(l.id, app.cs.bus_layer, &app.primary.map);
            }
        }
        colorer.intersections_from_roads(&app.primary.map);
        for bs in app.primary.map.all_bus_stops().keys() {
            colorer.add_bs(*bs, app.cs.bus_layer);
        }

        Static {
            colorer: colorer.build(ctx, app),
            name: "bus network",
        }
    }

    pub fn edits(ctx: &mut EventCtx, app: &App) -> Static {
        let edits = app.primary.map.get_edits();

        let mut colorer = Colorer::discrete(
            ctx,
            format!("Map edits ({})", edits.edits_name),
            vec![
                format!("{} lane types changed", edits.original_lts.len()),
                format!("{} lanes reversed", edits.reversed_lanes.len()),
                format!("{} speed limits changed", edits.changed_speed_limits.len()),
                format!(
                    "{} intersections changed",
                    edits.original_intersections.len()
                ),
            ],
            vec![("modified lane/intersection", app.cs.edits_layer)],
        );

        for l in edits.original_lts.keys().chain(&edits.reversed_lanes) {
            colorer.add_l(*l, app.cs.edits_layer, &app.primary.map);
        }
        for i in edits.original_intersections.keys() {
            colorer.add_i(*i, app.cs.edits_layer);
        }
        for r in &edits.changed_speed_limits {
            colorer.add_r(*r, app.cs.edits_layer, &app.primary.map);
        }

        Static {
            colorer: colorer.build(ctx, app),
            name: "map edits",
        }
    }

    pub fn amenities(ctx: &mut EventCtx, app: &App) -> Static {
        let mut colorer = Colorer::discrete(
            ctx,
            "Amenities",
            Vec::new(),
            vec![
                ("groceries", Color::BLACK),
                ("food", Color::RED),
                ("bar", Color::BLUE),
                ("medical", Color::PURPLE),
                ("church / temple", Color::GREEN),
                ("education", Color::CYAN),
                ("bank / post office", Color::YELLOW),
                ("media", Color::PINK),
                ("childcare", Color::ORANGE),
                ("shopping", Color::WHITE),
                ("other", Color::hex("#96322F")),
            ],
        );

        for b in app.primary.map.all_buildings() {
            let mut other = false;
            for (_, a) in &b.amenities {
                if let Some(t) = amenity_type(a) {
                    colorer.add_b(
                        b.id,
                        match t {
                            "groceries" => Color::BLACK,
                            "food" => Color::RED,
                            "bar" => Color::BLUE,
                            "medical" => Color::PURPLE,
                            "church / temple" => Color::GREEN,
                            "education" => Color::CYAN,
                            "bank / post office" => Color::YELLOW,
                            "media" => Color::PINK,
                            "childcare" => Color::ORANGE,
                            "shopping" => Color::WHITE,
                            _ => unreachable!(),
                        },
                    );
                } else {
                    other = true;
                }
            }
            if other {
                colorer.add_b(b.id, Color::hex("#96322F"));
            }
        }

        Static {
            colorer: colorer.build(ctx, app),
            name: "amenities",
        }
    }
}
