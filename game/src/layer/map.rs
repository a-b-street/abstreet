use abstutil::{prettyprint_usize, Counter};
use geom::{Distance, Time};
use map_gui::tools::{ColorDiscrete, ColorLegend, ColorNetwork};
use map_gui::ID;
use map_model::{AmenityType, LaneType, PathConstraints};
use sim::AgentType;
use widgetry::{
    Btn, Color, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Line, Panel, Text, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::App;
use crate::layer::{Layer, LayerOutcome};

pub struct BikeNetwork {
    panel: Panel,
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
        minimap: &Panel,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = BikeNetwork::new(ctx, app);
        }

        Layer::simple_event(ctx, minimap, &mut self.panel)
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
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
        for ((r, agent_type, _), count) in &app.primary.sim.get_analytics().road_thruput.counts {
            if *agent_type == AgentType::Bike {
                if app
                    .primary
                    .map
                    .get_r(*r)
                    .lanes_ltr()
                    .into_iter()
                    .any(|(_, _, lt)| lt == LaneType::Biking)
                {
                    on_bike_lanes.add(*r, *count);
                } else {
                    off_bike_lanes.add(*r, *count);
                }
            }
        }

        // Use intersection data too, but bin as on bike lanes or not based on connecting roads
        for ((i, agent_type, _), count) in
            &app.primary.sim.get_analytics().intersection_thruput.counts
        {
            if *agent_type == AgentType::Bike {
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

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                "Bike network".draw_text(ctx),
                Btn::close(ctx),
            ]),
            Text::from_multiline(vec![
                Line(format!("{} lanes", num_lanes)),
                Line(format!(
                    "total distance of {}",
                    total_dist.to_string(&app.opts.units)
                )),
            ])
            .draw(ctx),
            Line("Throughput on bike lanes").draw(ctx),
            ColorLegend::gradient(
                ctx,
                &app.cs.good_to_bad_green,
                vec!["lowest count", "highest"],
            ),
            Line("Throughput on unprotected roads").draw(ctx),
            ColorLegend::gradient(
                ctx,
                &app.cs.good_to_bad_red,
                vec!["lowest count", "highest"],
            ),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        let mut colorer = ColorNetwork::new(app);
        colorer.ranked_roads(on_bike_lanes, &app.cs.good_to_bad_green);
        colorer.ranked_roads(off_bike_lanes, &app.cs.good_to_bad_red);
        colorer.ranked_intersections(intersections_on, &app.cs.good_to_bad_green);
        colorer.ranked_intersections(intersections_off, &app.cs.good_to_bad_red);
        let (unzoomed, zoomed) = colorer.build(ctx);

        BikeNetwork {
            panel,
            time: app.primary.sim.time(),
            unzoomed,
            zoomed,
        }
    }
}

pub struct Static {
    panel: Panel,
    pub unzoomed: Drawable,
    pub zoomed: Drawable,
    name: &'static str,
}

impl Layer for Static {
    fn name(&self) -> Option<&'static str> {
        Some(self.name)
    }
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App, minimap: &Panel) -> Option<LayerOutcome> {
        Layer::simple_event(ctx, minimap, &mut self.panel)
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
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

impl Static {
    fn new(
        ctx: &mut EventCtx,
        colorer: ColorDiscrete,
        name: &'static str,
        title: String,
        extra: Widget,
    ) -> Static {
        let (unzoomed, zoomed, legend) = colorer.build(ctx);
        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                title.draw_text(ctx),
                Btn::close(ctx),
            ]),
            extra,
            legend,
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        Static {
            panel,
            unzoomed,
            zoomed,
            name,
        }
    }

    pub fn edits(ctx: &mut EventCtx, app: &App) -> Static {
        let mut colorer = ColorDiscrete::new(
            app,
            vec![("modified road/intersection", app.cs.edits_layer)],
        );

        let edits = app.primary.map.get_edits();
        let (lanes, roads) = edits.changed_lanes(&app.primary.map);
        for l in lanes {
            colorer.add_l(l, "modified road/intersection");
        }
        for r in roads {
            colorer.add_r(r, "modified road/intersection");
        }
        for i in edits.original_intersections.keys() {
            colorer.add_i(*i, "modified road/intersection");
        }

        Static::new(
            ctx,
            colorer,
            "map edits",
            format!("Map edits ({})", edits.edits_name),
            Text::from_multiline(vec![
                Line(format!("{} roads changed", edits.changed_roads.len())),
                Line(format!(
                    "{} intersections changed",
                    edits.original_intersections.len()
                )),
            ])
            .draw(ctx),
        )
    }

    pub fn amenities(ctx: &mut EventCtx, app: &App) -> Static {
        let mut colorer = ColorDiscrete::new(
            app,
            vec![
                (AmenityType::Groceries.to_string(), Color::BLACK),
                (AmenityType::Food.to_string(), Color::RED),
                (AmenityType::Bar.to_string(), Color::BLUE),
                (AmenityType::Medical.to_string(), Color::PURPLE),
                (AmenityType::Religious.to_string(), Color::GREEN),
                (AmenityType::Education.to_string(), Color::CYAN),
                (AmenityType::Financial.to_string(), Color::YELLOW),
                (AmenityType::PostOffice.to_string(), Color::YELLOW),
                (AmenityType::Culture.to_string(), Color::PINK),
                (AmenityType::Childcare.to_string(), Color::ORANGE),
                (AmenityType::Shopping.to_string(), Color::WHITE),
                ("other".to_string(), Color::hex("#96322F")),
            ],
        );

        for b in app.primary.map.all_buildings() {
            let mut other = false;
            for a in &b.amenities {
                if let Some(t) = AmenityType::categorize(&a.amenity_type) {
                    colorer.add_b(b.id, t.to_string());
                } else {
                    other = true;
                }
            }
            if other {
                colorer.add_b(b.id, "other");
            }
        }

        Static::new(
            ctx,
            colorer,
            "amenities",
            "Amenities".to_string(),
            Widget::nothing(),
        )
    }

    pub fn no_sidewalks(ctx: &mut EventCtx, app: &App) -> Static {
        let mut colorer = ColorDiscrete::new(app, vec![("no sidewalks", Color::RED)]);
        for l in app.primary.map.all_lanes() {
            if l.is_shoulder() {
                colorer.add_r(l.parent, "no sidewalks");
            }
        }
        Static::new(
            ctx,
            colorer,
            "no sidewalks",
            "No sidewalks".to_string(),
            Widget::nothing(),
        )
    }

    pub fn blackholes(ctx: &mut EventCtx, app: &App) -> Static {
        let mut colorer = ColorDiscrete::new(
            app,
            vec![
                ("driving blackhole", Color::RED),
                ("biking blackhole", Color::GREEN),
                ("driving + biking blackhole", Color::BLUE),
            ],
        );
        for l in app.primary.map.all_lanes() {
            if l.driving_blackhole && l.biking_blackhole {
                colorer.add_l(l.id, "driving + biking blackhole");
            } else if l.driving_blackhole {
                colorer.add_l(l.id, "driving blackhole");
            } else if l.biking_blackhole {
                colorer.add_l(l.id, "biking blackhole");
            }
        }
        Static::new(
            ctx,
            colorer,
            "blackholes",
            "blackholes".to_string(),
            Widget::nothing(),
        )
    }
}

pub struct CongestionCaps {
    panel: Panel,
    time: Time,
    unzoomed: Drawable,
    zoomed: Drawable,
    tooltip: Option<Text>,
}

impl Layer for CongestionCaps {
    fn name(&self) -> Option<&'static str> {
        Some("congestion caps")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Panel,
    ) -> Option<LayerOutcome> {
        let mut recalc_tooltip = false;
        if app.primary.sim.time() != self.time {
            *self = CongestionCaps::new(ctx, app);
            recalc_tooltip = true;
        }

        // Show a tooltip with count, only when unzoomed
        if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            if ctx.redo_mouseover() || recalc_tooltip {
                self.tooltip = None;
                match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                    Some(ID::Road(r)) => {
                        if let Some(cap) = app
                            .primary
                            .map
                            .get_r(r)
                            .access_restrictions
                            .cap_vehicles_per_hour
                        {
                            self.tooltip = Some(Text::from(Line(format!(
                                "Cap of {} vehicles per hour",
                                prettyprint_usize(cap)
                            ))));
                        }
                    }
                    _ => {}
                }
            }
        } else {
            self.tooltip = None;
        }

        Layer::simple_event(ctx, minimap, &mut self.panel)
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        if let Some(ref txt) = self.tooltip {
            g.draw_mouse_tooltip(txt.clone());
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl CongestionCaps {
    pub fn new(ctx: &mut EventCtx, app: &App) -> CongestionCaps {
        let mut colorer = ColorNetwork::new(app);
        let map = &app.primary.map;

        let mut num_roads = 0;
        for r in map.all_roads() {
            if let Some(cap) = r.access_restrictions.cap_vehicles_per_hour {
                num_roads += 1;
                if let Some(l) = r
                    .all_lanes()
                    .into_iter()
                    .find(|l| PathConstraints::Car.can_use(map.get_l(*l), map))
                {
                    let current = app.primary.sim.get_cap_counter(l);
                    let pct = ((current as f64) / (cap as f64)).min(1.0);
                    colorer.add_r(r.id, app.cs.good_to_bad_red.eval(pct));
                }
            }
        }

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                "Congestion caps".draw_text(ctx),
                Btn::close(ctx),
            ]),
            format!("{} roads have caps", prettyprint_usize(num_roads)).draw_text(ctx),
            ColorLegend::gradient(ctx, &app.cs.good_to_bad_red, vec!["available", "full"]),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        let (unzoomed, zoomed) = colorer.build(ctx);

        CongestionCaps {
            panel,
            time: app.primary.sim.time(),
            unzoomed,
            zoomed,
            tooltip: None,
        }
    }
}
