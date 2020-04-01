use crate::app::App;
use crate::colors;
use crate::common::{
    make_heatmap, ColorLegend, Colorer, HeatmapColors, HeatmapOptions, ShowBusRoute, Warping,
};
use crate::game::Transition;
use crate::helpers::ID;
use crate::managed::{ManagedGUIState, WrappedComposite};
use crate::render::MIN_ZOOM_FOR_DETAIL;
use abstutil::{prettyprint_usize, Counter};
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, Histogram,
    HorizontalAlignment, Key, Line, Outcome, Slider, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Duration, PolyLine, Pt2D, Time};
use map_model::{BusRouteID, IntersectionID};
use sim::{GetDrawAgents, PandemicModel, ParkingSpot, PersonState};
use std::collections::HashSet;

pub enum Overlays {
    Inactive,
    ParkingAvailability(Time, Colorer),
    WorstDelay(Time, Colorer),
    TrafficJams(Time, Colorer),
    CumulativeThroughput(Time, Colorer),
    BikeNetwork(Colorer),
    BusNetwork(Colorer),
    Elevation(Colorer, Drawable),
    Edits(Colorer),
    TripsHistogram(Time, Composite),
    PopulationMap(Time, PopulationOptions, Drawable, Composite),

    // These aren't selectable from the main picker; they're particular to some object.
    // TODO They should become something else, like an info panel tab.
    IntersectionDemand(Time, IntersectionID, Drawable, Composite),
    BusRoute(Time, BusRouteID, ShowBusRoute),
}

impl Overlays {
    pub fn is_empty(&self) -> bool {
        match self {
            Overlays::Inactive => true,
            _ => false,
        }
    }

    // Since Overlays is embedded in UI, we have to do this slight trick
    pub fn update(ctx: &mut EventCtx, app: &mut App, minimap: &Composite) -> Option<Transition> {
        let now = app.primary.sim.time();
        match app.overlay {
            Overlays::ParkingAvailability(t, _) => {
                if now != t {
                    app.overlay = Overlays::parking_availability(ctx, app);
                }
            }
            Overlays::WorstDelay(t, _) => {
                if now != t {
                    app.overlay = Overlays::worst_delay(ctx, app);
                }
            }
            Overlays::TrafficJams(t, _) => {
                if now != t {
                    app.overlay = Overlays::traffic_jams(ctx, app);
                }
            }
            Overlays::CumulativeThroughput(t, _) => {
                if now != t {
                    app.overlay = Overlays::cumulative_throughput(ctx, app);
                }
            }
            Overlays::IntersectionDemand(t, i, _, _) => {
                if now != t {
                    app.overlay = Overlays::intersection_demand(i, ctx, app);
                }
            }
            Overlays::TripsHistogram(t, _) => {
                if now != t {
                    app.overlay = Overlays::trips_histogram(ctx, app);
                }
            }
            Overlays::BusRoute(t, id, _) => {
                if now != t {
                    app.overlay = Overlays::show_bus_route(id, ctx, app);
                }
            }
            Overlays::PopulationMap(t, ref opts, _, _) => {
                if now != t {
                    app.overlay = Overlays::population_map(ctx, app, opts.clone());
                }
            }
            // No updates needed
            Overlays::Inactive
            | Overlays::BikeNetwork(_)
            | Overlays::BusNetwork(_)
            | Overlays::Elevation(_, _)
            | Overlays::Edits(_) => {}
        };

        match app.overlay {
            Overlays::ParkingAvailability(_, ref mut heatmap)
            | Overlays::BikeNetwork(ref mut heatmap)
            | Overlays::BusNetwork(ref mut heatmap)
            | Overlays::Elevation(ref mut heatmap, _)
            | Overlays::WorstDelay(_, ref mut heatmap)
            | Overlays::TrafficJams(_, ref mut heatmap)
            | Overlays::CumulativeThroughput(_, ref mut heatmap)
            | Overlays::Edits(ref mut heatmap) => {
                heatmap.legend.align_above(ctx, minimap);
                if heatmap.event(ctx) {
                    app.overlay = Overlays::Inactive;
                }
            }
            Overlays::BusRoute(_, _, ref mut c) => {
                c.colorer.legend.align_above(ctx, minimap);
                if c.colorer.event(ctx) {
                    app.overlay = Overlays::Inactive;
                }
            }
            Overlays::IntersectionDemand(_, i, _, ref mut c) => {
                c.align_above(ctx, minimap);
                match c.event(ctx) {
                    Some(Outcome::Clicked(x)) => match x.as_ref() {
                        "intersection demand" => {
                            let id = ID::Intersection(i);
                            return Some(Transition::Push(Warping::new(
                                ctx,
                                id.canonical_point(&app.primary).unwrap(),
                                Some(10.0),
                                Some(id.clone()),
                                &mut app.primary,
                            )));
                        }
                        "X" => {
                            app.overlay = Overlays::Inactive;
                        }
                        _ => unreachable!(),
                    },
                    None => {}
                }
            }
            Overlays::TripsHistogram(_, ref mut c) => {
                c.align_above(ctx, minimap);
                match c.event(ctx) {
                    Some(Outcome::Clicked(x)) => match x.as_ref() {
                        "X" => {
                            app.overlay = Overlays::Inactive;
                        }
                        _ => unreachable!(),
                    },
                    None => {}
                }
            }
            Overlays::PopulationMap(_, ref mut opts, _, ref mut c) => {
                c.align_above(ctx, minimap);
                match c.event(ctx) {
                    Some(Outcome::Clicked(x)) => match x.as_ref() {
                        "close" => {
                            app.overlay = Overlays::Inactive;
                        }
                        _ => unreachable!(),
                    },
                    None => {
                        let new_opts = population_options(c);
                        if *opts != new_opts {
                            app.overlay = Overlays::population_map(ctx, app, new_opts);
                        }
                    }
                }
            }
            Overlays::Inactive => {}
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            Overlays::Inactive => {}
            Overlays::ParkingAvailability(_, ref heatmap)
            | Overlays::BikeNetwork(ref heatmap)
            | Overlays::BusNetwork(ref heatmap)
            | Overlays::WorstDelay(_, ref heatmap)
            | Overlays::TrafficJams(_, ref heatmap)
            | Overlays::CumulativeThroughput(_, ref heatmap)
            | Overlays::Edits(ref heatmap) => {
                if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
                    heatmap.draw(g);
                }
            }
            Overlays::Elevation(ref heatmap, ref draw) => {
                // TODO Maybe this is still useful when zoomed in
                if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
                    heatmap.draw(g);
                    g.redraw(draw);
                }
            }
            Overlays::PopulationMap(_, _, ref draw, ref composite) => {
                if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
                    g.redraw(draw);
                    composite.draw(g);
                }
            }
            // All of these shouldn't care about zoom
            Overlays::TripsHistogram(_, ref composite) => {
                composite.draw(g);
            }
            Overlays::IntersectionDemand(_, _, ref draw, ref legend) => {
                g.redraw(draw);
                legend.draw(g);
            }
            Overlays::BusRoute(_, _, ref s) => {
                s.draw(g);
            }
        }
    }

    pub fn maybe_colorer(&self) -> Option<&Colorer> {
        match self {
            Overlays::ParkingAvailability(_, ref heatmap)
            | Overlays::BikeNetwork(ref heatmap)
            | Overlays::BusNetwork(ref heatmap)
            | Overlays::Elevation(ref heatmap, _)
            | Overlays::WorstDelay(_, ref heatmap)
            | Overlays::TrafficJams(_, ref heatmap)
            | Overlays::CumulativeThroughput(_, ref heatmap)
            | Overlays::Edits(ref heatmap) => Some(heatmap),
            Overlays::BusRoute(_, _, ref s) => Some(&s.colorer),
            _ => None,
        }
    }

    pub fn change_overlays(ctx: &mut EventCtx, app: &App) -> Option<Transition> {
        // TODO Icons again, after some work
        let mut choices = vec![
            Btn::text_fg("None").build_def(ctx, hotkey(Key::N)),
            Btn::text_fg("map edits").build_def(ctx, hotkey(Key::E)),
            Btn::text_fg("worst traffic jams").build_def(ctx, hotkey(Key::J)),
            Btn::text_fg("elevation").build_def(ctx, hotkey(Key::S)),
            Btn::text_fg("parking availability").build_def(ctx, hotkey(Key::P)),
            Btn::text_fg("delay").build_def(ctx, hotkey(Key::D)),
            Btn::text_fg("throughput").build_def(ctx, hotkey(Key::T)),
            Btn::text_fg("bike network").build_def(ctx, hotkey(Key::B)),
            Btn::text_fg("bus network").build_def(ctx, hotkey(Key::U)),
            Btn::text_fg("population map").build_def(ctx, hotkey(Key::X)),
        ];
        if let Some(name) = match app.overlay {
            Overlays::Inactive => Some("None"),
            Overlays::ParkingAvailability(_, _) => Some("parking availability"),
            Overlays::WorstDelay(_, _) => Some("delay"),
            Overlays::TrafficJams(_, _) => Some("worst traffic jams"),
            Overlays::CumulativeThroughput(_, _) => Some("throughput"),
            Overlays::BikeNetwork(_) => Some("bike network"),
            Overlays::BusNetwork(_) => Some("bus network"),
            Overlays::Elevation(_, _) => Some("elevation"),
            Overlays::Edits(_) => Some("map edits"),
            Overlays::PopulationMap(_, _, _, _) => Some("population map"),
            _ => None,
        } {
            for btn in &mut choices {
                if btn.is_btn(name) {
                    *btn = Btn::text_fg(name).inactive(ctx).outline(2.0, Color::GREEN);
                    break;
                }
            }
        }

        let c = WrappedComposite::new(
            Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        "Heat Map Layers".draw_text(ctx),
                        Btn::text_fg("X")
                            .build(ctx, "close", hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Widget::row(choices.into_iter().map(|x| x.margin(5)).collect())
                        .flex_wrap(ctx, 30),
                ])
                .bg(colors::PANEL_BG)
                .outline(10.0, Color::WHITE)
                .padding(10),
            )
            .max_size_percent(35, 50)
            .build(ctx),
        )
        .cb("close", Box::new(|_, _| Some(Transition::Pop)))
        .maybe_cb(
            "None",
            Box::new(|_, app| {
                app.overlay = Overlays::Inactive;
                Some(Transition::Pop)
            }),
        )
        .maybe_cb(
            "parking availability",
            Box::new(|ctx, app| {
                app.overlay = Overlays::parking_availability(ctx, app);
                Some(maybe_unzoom(ctx, app))
            }),
        )
        .maybe_cb(
            "delay",
            Box::new(|ctx, app| {
                app.overlay = Overlays::worst_delay(ctx, app);
                Some(maybe_unzoom(ctx, app))
            }),
        )
        .maybe_cb(
            "worst traffic jams",
            Box::new(|ctx, app| {
                app.overlay = Overlays::traffic_jams(ctx, app);
                Some(maybe_unzoom(ctx, app))
            }),
        )
        .maybe_cb(
            "throughput",
            Box::new(|ctx, app| {
                app.overlay = Overlays::cumulative_throughput(ctx, app);
                Some(maybe_unzoom(ctx, app))
            }),
        )
        .maybe_cb(
            "bike network",
            Box::new(|ctx, app| {
                app.overlay = Overlays::bike_network(ctx, app);
                Some(maybe_unzoom(ctx, app))
            }),
        )
        .maybe_cb(
            "bus network",
            Box::new(|ctx, app| {
                app.overlay = Overlays::bus_network(ctx, app);
                Some(maybe_unzoom(ctx, app))
            }),
        )
        .maybe_cb(
            "elevation",
            Box::new(|ctx, app| {
                app.overlay = Overlays::elevation(ctx, app);
                Some(maybe_unzoom(ctx, app))
            }),
        )
        .maybe_cb(
            "map edits",
            Box::new(|ctx, app| {
                app.overlay = Overlays::map_edits(ctx, app);
                Some(maybe_unzoom(ctx, app))
            }),
        )
        .maybe_cb(
            "population map",
            Box::new(|ctx, app| {
                app.overlay = Overlays::population_map(
                    ctx,
                    app,
                    PopulationOptions {
                        pandemic: false,
                        heatmap: Some(HeatmapOptions::new()),
                    },
                );
                Some(maybe_unzoom(ctx, app))
            }),
        );
        Some(Transition::Push(ManagedGUIState::over_map(c)))
    }

    // Only for those hidden when zoomed in
    pub fn zoomed_name(&self) -> Option<&'static str> {
        match self {
            Overlays::Inactive => None,
            Overlays::ParkingAvailability(_, _) => Some("parking availability"),
            Overlays::WorstDelay(_, _) => Some("delay"),
            Overlays::TrafficJams(_, _) => Some("traffic jams"),
            Overlays::CumulativeThroughput(_, _) => Some("throughput"),
            Overlays::BikeNetwork(_) => Some("bike network"),
            Overlays::BusNetwork(_) => Some("bus network"),
            Overlays::Elevation(_, _) => Some("elevation"),
            Overlays::Edits(_) => Some("map edits"),
            Overlays::PopulationMap(_, _, _, _) => Some("population map"),
            Overlays::TripsHistogram(_, _) => None,
            Overlays::IntersectionDemand(_, _, _, _) => None,
            Overlays::BusRoute(_, _, _) => None,
        }
    }
}

impl Overlays {
    fn parking_availability(ctx: &mut EventCtx, app: &App) -> Overlays {
        let (filled_spots, avail_spots) = app.primary.sim.get_all_parking_spots();
        let mut txt = Text::from(Line("parking availability"));
        txt.add(Line(format!(
            "{} spots filled",
            prettyprint_usize(filled_spots.len())
        )));
        txt.add(Line(format!(
            "{} spots available ",
            prettyprint_usize(avail_spots.len())
        )));

        let awful = Color::hex("#801F1C");
        let bad = Color::hex("#EB5757");
        let meh = Color::hex("#F2C94C");
        let good = Color::hex("#7FFA4D");
        let mut colorer = Colorer::new(
            txt,
            vec![
                ("< 10%", awful),
                ("< 30%", bad),
                ("< 60%", meh),
                (">= 60%", good),
            ],
        );

        let lane = |spot| match spot {
            ParkingSpot::Onstreet(l, _) => l,
            ParkingSpot::Offstreet(b, _) => app
                .primary
                .map
                .get_b(b)
                .parking
                .as_ref()
                .unwrap()
                .driving_pos
                .lane(),
        };

        let mut filled = Counter::new();
        let mut avail = Counter::new();
        let mut keys = HashSet::new();
        for spot in filled_spots {
            let l = lane(spot);
            keys.insert(l);
            filled.inc(l);
        }
        for spot in avail_spots {
            let l = lane(spot);
            keys.insert(l);
            avail.inc(l);
        }

        for l in keys {
            let open = avail.get(l);
            let closed = filled.get(l);
            let percent = (open as f64) / ((open + closed) as f64);
            let color = if percent >= 0.6 {
                good
            } else if percent > 0.3 {
                meh
            } else if percent > 0.1 {
                bad
            } else {
                awful
            };
            colorer.add_l(l, color, &app.primary.map);
        }

        Overlays::ParkingAvailability(app.primary.sim.time(), colorer.build(ctx, app))
    }

    fn worst_delay(ctx: &mut EventCtx, app: &App) -> Overlays {
        let slow = Color::hex("#EB5757");
        let moderate = Color::hex("#F4DA22");
        let fast = Color::hex("#7FFA4D");
        // TODO explain more
        let mut colorer = Colorer::new(
            Text::from(Line("delay")),
            vec![
                ("> 5 minutes", slow),
                ("1 - 5 minutes", moderate),
                ("< 60s", fast),
            ],
        );

        let (per_road, per_intersection) = app.primary.sim.worst_delay(&app.primary.map);
        for (r, d) in per_road {
            let color = if d > Duration::minutes(5) {
                slow
            } else if d > Duration::minutes(1) {
                moderate
            } else {
                fast
            };
            colorer.add_r(r, color, &app.primary.map);
        }
        for (i, d) in per_intersection {
            let color = if d > Duration::minutes(5) {
                slow
            } else if d > Duration::minutes(1) {
                moderate
            } else {
                fast
            };
            colorer.add_i(i, color);
        }

        Overlays::WorstDelay(app.primary.sim.time(), colorer.build(ctx, app))
    }

    pub fn traffic_jams(ctx: &mut EventCtx, app: &App) -> Overlays {
        let jams = app.primary.sim.delayed_intersections(Duration::minutes(5));

        // TODO Silly colors
        let others = Color::hex("#7FFA4D");
        let early = Color::hex("#F4DA22");
        let earliest = Color::hex("#EB5757");
        let mut colorer = Colorer::new(
            Text::from(Line(format!("{} traffic jams", jams.len()))),
            vec![
                ("longest lasting", earliest),
                ("recent problems", early),
                ("others", others),
            ],
        );

        for (idx, (i, _)) in jams.into_iter().enumerate() {
            if idx == 0 {
                colorer.add_i(i, earliest);
            } else if idx <= 5 {
                colorer.add_i(i, early);
            } else {
                colorer.add_i(i, others);
            }
        }

        Overlays::TrafficJams(app.primary.sim.time(), colorer.build(ctx, app))
    }

    fn cumulative_throughput(ctx: &mut EventCtx, app: &App) -> Overlays {
        let light = Color::hex("#7FFA4D");
        let medium = Color::hex("#F4DA22");
        let heavy = Color::hex("#EB5757");
        let mut colorer = Colorer::new(
            Text::from(Line("Throughput")),
            vec![
                ("< 50%ile", light),
                ("< 90%ile", medium),
                (">= 90%ile", heavy),
            ],
        );

        let stats = &app.primary.sim.get_analytics().thruput_stats;

        // TODO If there are many duplicate counts, arbitrarily some will look heavier! Find the
        // disribution of counts instead.
        // TODO Actually display the counts at these percentiles
        // TODO Dump the data in debug mode
        {
            let roads = stats.count_per_road.sorted_asc();
            let p50_idx = ((roads.len() as f64) * 0.5) as usize;
            let p90_idx = ((roads.len() as f64) * 0.9) as usize;
            for (idx, r) in roads.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    light
                } else if idx < p90_idx {
                    medium
                } else {
                    heavy
                };
                colorer.add_r(*r, color, &app.primary.map);
            }
        }
        // TODO dedupe
        {
            let intersections = stats.count_per_intersection.sorted_asc();
            let p50_idx = ((intersections.len() as f64) * 0.5) as usize;
            let p90_idx = ((intersections.len() as f64) * 0.9) as usize;
            for (idx, i) in intersections.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    light
                } else if idx < p90_idx {
                    medium
                } else {
                    heavy
                };
                colorer.add_i(*i, color);
            }
        }

        Overlays::CumulativeThroughput(app.primary.sim.time(), colorer.build(ctx, app))
    }

    fn bike_network(ctx: &mut EventCtx, app: &App) -> Overlays {
        let color = Color::hex("#7FFA4D");
        let mut colorer = Colorer::new(
            Text::from(Line("bike networks")),
            vec![("bike lanes", color)],
        );
        for l in app.primary.map.all_lanes() {
            if l.is_biking() {
                colorer.add_l(l.id, color, &app.primary.map);
            }
        }
        Overlays::BikeNetwork(colorer.build(ctx, app))
    }

    fn bus_network(ctx: &mut EventCtx, app: &App) -> Overlays {
        let lane = Color::hex("#4CA7E9");
        let stop = Color::hex("#4CA7E9");
        let mut colorer = Colorer::new(
            Text::from(Line("bus networks")),
            vec![("bus lanes", lane), ("bus stops", stop)],
        );
        for l in app.primary.map.all_lanes() {
            if l.is_bus() {
                colorer.add_l(l.id, lane, &app.primary.map);
            }
        }
        for bs in app.primary.map.all_bus_stops().keys() {
            colorer.add_bs(*bs, stop);
        }

        Overlays::BusNetwork(colorer.build(ctx, app))
    }

    fn elevation(ctx: &mut EventCtx, app: &App) -> Overlays {
        // TODO Two passes because we have to construct the text first :(
        let mut max = 0.0_f64;
        for l in app.primary.map.all_lanes() {
            let pct = l.percent_grade(&app.primary.map).abs();
            max = max.max(pct);
        }
        let mut txt = Text::from(Line("elevation change"));
        txt.add(Line(format!("Steepest road: {:.0}%", max * 100.0)));

        let awful = Color::hex("#801F1C");
        let bad = Color::hex("#EB5757");
        let meh = Color::hex("#F2C94C");
        let good = Color::hex("#7FFA4D");
        let mut colorer = Colorer::new(
            txt,
            vec![
                (">= 15% (steep)", awful),
                ("< 15%", bad),
                ("< 5%", meh),
                ("< 1% (flat)", good),
            ],
        );

        let mut max = 0.0_f64;
        for l in app.primary.map.all_lanes() {
            let pct = l.percent_grade(&app.primary.map).abs();
            max = max.max(pct);

            let color = if pct < 0.01 {
                good
            } else if pct < 0.05 {
                meh
            } else if pct < 0.15 {
                bad
            } else {
                awful
            };
            colorer.add_l(l.id, color, &app.primary.map);
        }

        let arrow_color = Color::BLACK;
        let mut batch = GeomBatch::new();
        // Time for uphill arrows!
        // TODO Draw V's, not arrows.
        // TODO Or try gradient colors.
        for r in app.primary.map.all_roads() {
            let (mut pl, _) = r.get_thick_polyline(&app.primary.map).unwrap();
            let e1 = app.primary.map.get_i(r.src_i).elevation;
            let e2 = app.primary.map.get_i(r.dst_i).elevation;
            if (e1 - e2).abs() / pl.length() < 0.01 {
                // Don't bother with ~flat roads
                continue;
            }
            if e1 > e2 {
                pl = pl.reversed();
            }

            let arrow_len = Distance::meters(5.0);
            let btwn = Distance::meters(10.0);
            let thickness = Distance::meters(1.0);
            let len = pl.length();

            let mut dist = arrow_len;
            while dist + arrow_len <= len {
                let (pt, angle) = pl.dist_along(dist);
                batch.push(
                    arrow_color,
                    PolyLine::new(vec![
                        pt.project_away(arrow_len / 2.0, angle.opposite()),
                        pt.project_away(arrow_len / 2.0, angle),
                    ])
                    .make_arrow(thickness)
                    .unwrap(),
                );
                dist += btwn;
            }
        }

        Overlays::Elevation(colorer.build(ctx, app), batch.upload(ctx))
    }

    pub fn trips_histogram(ctx: &mut EventCtx, app: &App) -> Overlays {
        if app.has_prebaked().is_none() {
            return Overlays::Inactive;
        }

        let now = app.primary.sim.time();
        Overlays::TripsHistogram(
            now,
            Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        {
                            let mut txt = Text::from(Line("Are trips "));
                            txt.append(Line("faster").fg(Color::GREEN));
                            txt.append(Line(", "));
                            txt.append(Line("slower").fg(Color::RED));
                            txt.append(Line(", or "));
                            txt.append(Line("the same").fg(Color::YELLOW));
                            txt.append(Line("?"));
                            txt.draw(ctx)
                        }
                        .margin(10),
                        Btn::text_fg("X").build_def(ctx, None).align_right(),
                    ]),
                    Histogram::new(
                        app.primary
                            .sim
                            .get_analytics()
                            .trip_time_deltas(now, app.prebaked()),
                        ctx,
                    ),
                ])
                .bg(colors::PANEL_BG),
            )
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
            .build(ctx),
        )
    }

    pub fn intersection_demand(i: IntersectionID, ctx: &mut EventCtx, app: &App) -> Overlays {
        let mut batch = GeomBatch::new();

        let mut total_demand = 0;
        let mut demand_per_group: Vec<(&PolyLine, usize)> = Vec::new();
        for g in app.primary.map.get_traffic_signal(i).turn_groups.values() {
            let demand = app
                .primary
                .sim
                .get_analytics()
                .thruput_stats
                .demand
                .get(&g.id)
                .cloned()
                .unwrap_or(0);
            if demand > 0 {
                total_demand += demand;
                demand_per_group.push((&g.geom, demand));
            }
        }

        for (pl, demand) in demand_per_group {
            let percent = (demand as f64) / (total_demand as f64);
            batch.push(
                Color::RED,
                pl.make_arrow(percent * Distance::meters(5.0)).unwrap(),
            );
        }

        let col = vec![
            Widget::row(vec![
                "intersection demand".draw_text(ctx),
                Btn::svg_def("../data/system/assets/tools/location.svg")
                    .build(ctx, "intersection demand", None)
                    .margin(5),
                Btn::text_fg("X").build_def(ctx, None).align_right(),
            ]),
            ColorLegend::row(ctx, Color::RED, "current demand"),
        ];

        Overlays::IntersectionDemand(
            app.primary.sim.time(),
            i,
            batch.upload(ctx),
            Composite::new(Widget::col(col).bg(colors::PANEL_BG))
                .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
                .build(ctx),
        )
    }

    pub fn show_bus_route(id: BusRouteID, ctx: &mut EventCtx, app: &App) -> Overlays {
        Overlays::BusRoute(app.primary.sim.time(), id, ShowBusRoute::new(id, ctx, app))
    }

    pub fn map_edits(ctx: &mut EventCtx, app: &App) -> Overlays {
        let edits = app.primary.map.get_edits();

        let mut txt = Text::from(Line(format!("map edits ({})", edits.edits_name)));
        txt.add(Line(format!(
            "{} lane types changed",
            edits.original_lts.len()
        )));
        txt.add(Line(format!(
            "{} lanes reversed",
            edits.reversed_lanes.len()
        )));
        txt.add(Line(format!(
            "{} intersections changed",
            edits.original_intersections.len()
        )));

        let changed = Color::CYAN;
        let mut colorer = Colorer::new(txt, vec![("modified lane/intersection", changed)]);

        for l in edits.original_lts.keys().chain(&edits.reversed_lanes) {
            colorer.add_l(*l, changed, &app.primary.map);
        }
        for i in edits.original_intersections.keys() {
            colorer.add_i(*i, changed);
        }

        Overlays::Edits(colorer.build(ctx, app))
    }

    // TODO Disable drawing unzoomed agents... or alternatively, implement this by asking Sim to
    // return this kind of data instead!
    fn population_map(ctx: &mut EventCtx, app: &App, opts: PopulationOptions) -> Overlays {
        // Only display infected people if this is enabled.
        let maybe_pandemic = if opts.pandemic {
            // TODO Why not app.primary.current_flags.sim_flags.make_rng()? Because that'll only be
            // the same every time this code runs (frequently, as the simulation is run)
            // if --rng_seed is specified in the flags. If you forget it, quite
            // confusing to see the model jump around.
            use rand::SeedableRng;
            use rand_xorshift::XorShiftRng;

            Some(PandemicModel::calculate(
                app.primary.sim.get_analytics(),
                app.primary.sim.time(),
                &mut XorShiftRng::from_seed([42; 16]),
            ))
        } else {
            None
        };

        let mut pts = Vec::new();
        // Faster to grab all agent positions than individually map trips to agent positions.
        if let Some(ref model) = maybe_pandemic {
            for a in app.primary.sim.get_unzoomed_agents(&app.primary.map) {
                if let Some(p) = a.person {
                    if model.infected.contains(&p) {
                        pts.push(a.pos);
                    }
                }
            }
        } else {
            for a in app.primary.sim.get_unzoomed_agents(&app.primary.map) {
                pts.push(a.pos);
            }
        }

        // Many people are probably in the same building. If we're building a heatmap, we
        // absolutely care about these repeats! If we're just drawing the simple dot map, avoid
        // drawing repeat circles.
        let mut seen_bldgs = HashSet::new();
        let mut repeat_pts = Vec::new();
        for person in app.primary.sim.get_all_people() {
            match person.state {
                // Already covered above
                PersonState::Trip(_) => {}
                PersonState::Inside(b) => {
                    if maybe_pandemic
                        .as_ref()
                        .map(|m| !m.infected.contains(&person.id))
                        .unwrap_or(false)
                    {
                        continue;
                    }

                    let pt = app.primary.map.get_b(b).polygon.center();
                    if seen_bldgs.contains(&b) {
                        repeat_pts.push(pt);
                    } else {
                        seen_bldgs.insert(b);
                        pts.push(pt);
                    }
                }
                PersonState::OffMap | PersonState::Limbo => {}
            }
        }

        let mut batch = GeomBatch::new();
        let color_scale = if let Some(ref o) = opts.heatmap {
            pts.extend(repeat_pts);
            make_heatmap(&mut batch, app.primary.map.get_bounds(), pts, o)
        } else {
            // It's quite silly to produce triangles for the same circle over and over again. ;)
            let circle = Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(10.0)).to_polygon();
            for pt in pts {
                batch.push(Color::RED.alpha(0.8), circle.translate(pt.x(), pt.y()));
            }
            Vec::new()
        };
        let controls = population_controls(ctx, app, &opts, maybe_pandemic, color_scale);
        Overlays::PopulationMap(app.primary.sim.time(), opts, ctx.upload(batch), controls)
    }
}

fn maybe_unzoom(ctx: &EventCtx, app: &mut App) -> Transition {
    if ctx.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
        return Transition::Pop;
    }
    Transition::Replace(Warping::new(
        ctx,
        ctx.canvas.center_to_map_pt(),
        Some(0.99 * MIN_ZOOM_FOR_DETAIL),
        None,
        &mut app.primary,
    ))
}

#[derive(Clone, PartialEq)]
pub struct PopulationOptions {
    pandemic: bool,
    // If None, just a dot map
    heatmap: Option<HeatmapOptions>,
}

// This function sounds more ominous than it should.
fn population_controls(
    ctx: &mut EventCtx,
    app: &App,
    opts: &PopulationOptions,
    pandemic: Option<PandemicModel>,
    max_per_color: Vec<(f64, Color)>,
) -> Composite {
    let (total_ppl, ppl_in_bldg, ppl_off_map) = app.primary.sim.num_ppl();

    let mut col = vec![
        Widget::row(vec![
            // TODO Only bold the first part
            Line(format!("Population: {}", prettyprint_usize(total_ppl)))
                .small_heading()
                .draw(ctx),
            Btn::text_fg("X")
                .build(ctx, "close", hotkey(Key::Escape))
                .align_right(),
        ]),
        Widget::row(vec![
            Widget::draw_svg(ctx, "../data/system/assets/tools/home.svg"),
            prettyprint_usize(ppl_in_bldg).draw_text(ctx),
            format!("Off-map: {}", prettyprint_usize(ppl_off_map)).draw_text(ctx),
        ])
        .centered(),
        Widget::checkbox(ctx, "Run pandemic model", None, opts.pandemic),
    ];

    if let Some(model) = pandemic {
        col.push(
            format!(
                "Pandemic model: {} infected ({:.1}%)",
                prettyprint_usize(model.infected.len()),
                (model.infected.len() as f64) / (total_ppl as f64) * 100.0
            )
            .draw_text(ctx),
        );
    }

    col.push(Widget::checkbox(
        ctx,
        "Show heatmap",
        None,
        opts.heatmap.is_some(),
    ));
    if let Some(ref o) = opts.heatmap {
        // TODO Display the value...
        col.push(Widget::row(vec![
            "Resolution (meters)".draw_text(ctx).margin(5),
            // 1 to 100m
            Slider::horizontal(ctx, 100.0, 25.0, (o.resolution - 1.0) / 99.0)
                .named("resolution")
                .align_right()
                .centered_vert(),
        ]));
        col.push(Widget::row(vec![
            "Radius (0 to 10 * resolution)".draw_text(ctx).margin(5),
            // 0 to 10
            Slider::horizontal(ctx, 100.0, 25.0, (o.radius as f64) / 10.0)
                .named("radius")
                .align_right()
                .centered_vert(),
        ]));

        col.push(Widget::row(vec![
            "Color scheme".draw_text(ctx).margin(5),
            Widget::dropdown(ctx, "Colors", o.colors, HeatmapColors::choices()),
        ]));

        // Legend for the heatmap colors
        for (max, color) in max_per_color {
            col.push(ColorLegend::row(ctx, color, format!("<= {}", max)));
        }
    }

    Composite::new(Widget::col(col).padding(5).bg(colors::PANEL_BG))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx)
}

fn population_options(c: &mut Composite) -> PopulationOptions {
    let heatmap = if c.is_checked("Show heatmap") {
        // Did we just change?
        if c.has_widget("resolution") {
            Some(HeatmapOptions {
                resolution: 1.0 + c.slider("resolution").get_percent() * 99.0,
                radius: (c.slider("radius").get_percent() * 10.0) as usize,
                colors: c.dropdown_value("Colors"),
            })
        } else {
            Some(HeatmapOptions::new())
        }
    } else {
        None
    };
    PopulationOptions {
        pandemic: c.is_checked("Run pandemic model"),
        heatmap,
    }
}
