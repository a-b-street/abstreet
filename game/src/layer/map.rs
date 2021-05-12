use maplit::btreeset;

use abstutil::{prettyprint_usize, Counter};
use geom::{Distance, Time};
use map_gui::tools::{ColorDiscrete, ColorLegend, ColorNetwork};
use map_gui::ID;
use map_model::{AmenityType, LaneType};
use sim::AgentType;
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Panel, Text, TextExt, Widget};

use crate::app::App;
use crate::layer::{header, Layer, LayerOutcome, PANEL_PLACEMENT};

pub struct BikeActivity {
    panel: Panel,
    time: Time,
    unzoomed: Drawable,
    zoomed: Drawable,
    tooltip: Option<Text>,
}

impl Layer for BikeActivity {
    fn name(&self) -> Option<&'static str> {
        Some("cycling activity")
    }
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        let mut recalc_tooltip = false;
        if app.primary.sim.time() != self.time {
            *self = BikeActivity::new(ctx, app);
            recalc_tooltip = true;
        }

        // Show a tooltip with count, only when unzoomed
        if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            if ctx.redo_mouseover() || recalc_tooltip {
                self.tooltip = None;
                if let Some(ID::Road(r)) = app.mouseover_unzoomed_roads_and_intersections(ctx) {
                    let cnt = app
                        .primary
                        .sim
                        .get_analytics()
                        .road_thruput
                        .total_for_with_agent_types(r, btreeset! { AgentType::Bike });
                    if cnt > 0 {
                        self.tooltip = Some(Text::from(prettyprint_usize(cnt)));
                    }
                }
            }
        } else {
            self.tooltip = None;
        }

        <dyn Layer>::simple_event(ctx, &mut self.panel)
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

impl BikeActivity {
    pub fn new(ctx: &mut EventCtx, app: &App) -> BikeActivity {
        let mut num_lanes = 0;
        let mut total_dist = Distance::ZERO;
        let mut on_bike_lanes = Counter::new();
        let mut off_bike_lanes = Counter::new();
        let mut intersections_on = Counter::new();
        let mut intersections_off = Counter::new();
        // Make sure all bikes lanes show up no matter what
        for l in app.primary.map.all_lanes().values() {
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

        let panel = Panel::new_builder(Widget::col(vec![
            header(ctx, "Cycling activity"),
            Text::from_multiline(vec![
                Line(format!("{} bike lanes", num_lanes)),
                Line(format!(
                    "total distance of {}",
                    total_dist.to_string(&app.opts.units)
                )),
            ])
            .into_widget(ctx),
            Line("Throughput on bike lanes").into_widget(ctx),
            ColorLegend::gradient(
                ctx,
                &app.cs.good_to_bad_green,
                vec!["lowest count", "highest"],
            ),
            Line("Throughput on unprotected roads").into_widget(ctx),
            ColorLegend::gradient(
                ctx,
                &app.cs.good_to_bad_red,
                vec!["lowest count", "highest"],
            ),
        ]))
        .aligned_pair(PANEL_PLACEMENT)
        .build(ctx);

        let mut colorer = ColorNetwork::new(app);
        colorer.ranked_roads(on_bike_lanes, &app.cs.good_to_bad_green);
        colorer.ranked_roads(off_bike_lanes, &app.cs.good_to_bad_red);
        colorer.ranked_intersections(intersections_on, &app.cs.good_to_bad_green);
        colorer.ranked_intersections(intersections_off, &app.cs.good_to_bad_red);
        let (unzoomed, zoomed) = colorer.build(ctx);

        BikeActivity {
            panel,
            time: app.primary.sim.time(),
            unzoomed,
            zoomed,
            tooltip: None,
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
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Option<LayerOutcome> {
        <dyn Layer>::simple_event(ctx, &mut self.panel)
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
        let panel = Panel::new_builder(Widget::col(vec![header(ctx, &title), extra, legend]))
            .aligned_pair(PANEL_PLACEMENT)
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
            .into_widget(ctx),
        )
    }

    pub fn amenities(ctx: &mut EventCtx, app: &App) -> Static {
        let food = Color::RED;
        let school = Color::CYAN;
        let shopping = Color::PURPLE;
        let other = Color::GREEN;

        let mut unzoomed = GeomBatch::new();
        let mut zoomed = GeomBatch::new();
        for b in app.primary.map.all_buildings() {
            if b.amenities.is_empty() {
                continue;
            }
            let mut color = None;
            for a in &b.amenities {
                if let Some(t) = AmenityType::categorize(&a.amenity_type) {
                    color = Some(match t {
                        AmenityType::Food => food,
                        AmenityType::School => school,
                        AmenityType::Shopping => shopping,
                        _ => other,
                    });
                    break;
                }
            }
            let color = color.unwrap_or(other);
            unzoomed.push(color, b.polygon.clone());
            zoomed.push(color.alpha(0.4), b.polygon.clone());
        }

        let panel = Panel::new_builder(Widget::col(vec![
            header(ctx, "Amenities"),
            ColorLegend::row(ctx, food, AmenityType::Food.to_string()),
            ColorLegend::row(ctx, school, AmenityType::School.to_string()),
            ColorLegend::row(ctx, shopping, AmenityType::Shopping.to_string()),
            ColorLegend::row(ctx, other, "other".to_string()),
        ]))
        .aligned_pair(PANEL_PLACEMENT)
        .build(ctx);

        Static {
            panel,
            unzoomed: ctx.upload(unzoomed),
            zoomed: ctx.upload(zoomed),
            name: "amenities",
        }
    }

    pub fn no_sidewalks(ctx: &mut EventCtx, app: &App) -> Static {
        let mut colorer = ColorDiscrete::new(app, vec![("no sidewalks", Color::RED)]);
        for l in app.primary.map.all_lanes().values() {
            if l.is_shoulder() && !app.primary.map.get_r(l.parent).is_cycleway() {
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
        for l in app.primary.map.all_lanes().values() {
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
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        let mut recalc_tooltip = false;
        if app.primary.sim.time() != self.time {
            *self = CongestionCaps::new(ctx, app);
            recalc_tooltip = true;
        }

        // Show a tooltip with count, only when unzoomed
        if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            if ctx.redo_mouseover() || recalc_tooltip {
                self.tooltip = None;
                if let Some(ID::Road(r)) = app.mouseover_unzoomed_roads_and_intersections(ctx) {
                    if let Some(cap) = app
                        .primary
                        .map
                        .get_r(r)
                        .access_restrictions
                        .cap_vehicles_per_hour
                    {
                        self.tooltip = Some(Text::from(format!(
                            "Cap of {} vehicles per hour",
                            prettyprint_usize(cap)
                        )));
                    }
                }
            }
        } else {
            self.tooltip = None;
        }

        <dyn Layer>::simple_event(ctx, &mut self.panel)
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
                let current = app.primary.sim.get_cap_counter(r.id);
                let pct = ((current as f64) / (cap as f64)).min(1.0);
                colorer.add_r(r.id, app.cs.good_to_bad_red.eval(pct));
            }
        }

        let panel = Panel::new_builder(Widget::col(vec![
            header(ctx, "Congestion caps"),
            format!("{} roads have caps", prettyprint_usize(num_roads)).text_widget(ctx),
            ColorLegend::gradient(ctx, &app.cs.good_to_bad_red, vec!["available", "full"]),
        ]))
        .aligned_pair(PANEL_PLACEMENT)
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
