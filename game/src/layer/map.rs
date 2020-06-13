use crate::app::App;
use crate::common::{ColorLegend, Colorer};
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
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl BikeNetwork {
    pub fn new(ctx: &mut EventCtx, app: &App) -> BikeNetwork {
        let mut on_bike_lanes = Counter::new();
        let mut off_bike_lanes = Counter::new();
        // Make sure all bikes lanes show up no matter what
        for l in app.primary.map.all_lanes() {
            if l.is_biking() {
                on_bike_lanes.add(l.parent, 0);
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

        // TODO Weird combo colorer with two scales?!
        let mut colors = app.cs.good_to_bad_monochrome_green.to_vec();
        colors.extend(app.cs.good_to_bad_monochrome_red.to_vec());
        let mut colorer = Colorer::scaled(
            ctx,
            "Bike throughput",
            Vec::new(),
            colors,
            // Dummy
            vec!["0", "50", "90", "99", "100", "50", "90", "99", "100"],
        );

        for (counter, scale) in vec![
            (on_bike_lanes, &app.cs.good_to_bad_monochrome_green),
            (off_bike_lanes, &app.cs.good_to_bad_monochrome_red),
        ] {
            let roads = counter.sorted_asc();
            let p50_idx = ((roads.len() as f64) * 0.5) as usize;
            let p90_idx = ((roads.len() as f64) * 0.9) as usize;
            let p99_idx = ((roads.len() as f64) * 0.99) as usize;
            for (idx, list) in roads.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    scale[0]
                } else if idx < p90_idx {
                    scale[1]
                } else if idx < p99_idx {
                    scale[2]
                } else {
                    scale[3]
                };
                for r in list {
                    colorer.add_r(r, color, &app.primary.map);
                }
            }
        }
        colorer.intersections_from_roads(&app.primary.map);

        let mut num_lanes = 0;
        let mut total_dist = Distance::ZERO;
        for l in app.primary.map.all_lanes() {
            if l.is_biking() {
                num_lanes += 1;
                total_dist += l.length();
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
                Line("Throughput on bike lanes (percentiles)").draw(ctx),
                ColorLegend::scale(
                    ctx,
                    app.cs.good_to_bad_monochrome_green.to_vec(),
                    vec!["0%", "40%", "70%", "90%", "100%"],
                )
                .margin_below(10),
                Line("Throughput on unprotected roads (percentiles)").draw(ctx),
                ColorLegend::scale(
                    ctx,
                    app.cs.good_to_bad_monochrome_red.to_vec(),
                    vec!["0%", "40%", "70%", "90%", "100%"],
                ),
            ])
            .padding(5)
            .bg(app.cs.panel_bg),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        BikeNetwork {
            composite,
            time: app.primary.sim.time(),
            unzoomed: colorer.build_both(ctx, app).unzoomed,
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
            colorer: colorer.build_unzoomed(ctx, app),
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
            colorer: colorer.build_both(ctx, app),
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
            colorer: colorer.build_both(ctx, app),
            name: "amenities",
        }
    }
}
