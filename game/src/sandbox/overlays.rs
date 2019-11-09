use crate::common::{
    ObjectColorer, ObjectColorerBuilder, Plot, RoadColorer, RoadColorerBuilder, Series,
};
use crate::game::{Transition, WizardState};
use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::sandbox::bus_explorer::ShowBusRoute;
use crate::sandbox::SandboxMode;
use crate::ui::{ShowEverything, UI};
use abstutil::{prettyprint_usize, Counter};
use ezgui::{Choice, Color, EventCtx, GfxCtx, Key, Line, MenuUnderButton, Text};
use geom::Duration;
use map_model::{LaneType, PathStep};
use sim::{ParkingSpot, TripMode};
use std::collections::{BTreeMap, HashSet};

pub enum Overlays {
    Inactive,
    ParkingAvailability(Duration, RoadColorer),
    IntersectionDelay(Duration, ObjectColorer),
    Throughput(Duration, ObjectColorer),
    FinishedTrips(Duration, Plot<usize>),
    Chokepoints(Duration, ObjectColorer),
    BikeNetwork(RoadColorer),
    BusNetwork(RoadColorer),
    // Only set by certain gameplay modes
    BusRoute(ShowBusRoute),
    BusDelaysOverTime(Plot<Duration>),
}

impl Overlays {
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &UI,
        menu: &mut MenuUnderButton,
    ) -> Option<Transition> {
        if menu.action("change analytics overlay") {
            return Some(Transition::Push(WizardState::new(Box::new(
                |wiz, ctx, _| {
                    let (choice, _) =
                        wiz.wrap(ctx).choose("Show which analytics overlay?", || {
                            // TODO Filter out the current
                            vec![
                                Choice::new("none", ()).key(Key::N),
                                Choice::new("parking availability", ()).key(Key::P),
                                Choice::new("intersection delay", ()).key(Key::I),
                                Choice::new("cumulative throughput", ()).key(Key::T),
                                Choice::new("finished trips", ()).key(Key::F),
                                Choice::new("chokepoints", ()).key(Key::C),
                                Choice::new("bike network", ()).key(Key::B),
                                Choice::new("bus network", ()).key(Key::U),
                            ]
                        })?;
                    Some(Transition::PopWithData(Box::new(move |state, ui, ctx| {
                        let mut sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                        sandbox.overlay = Overlays::recalc(&choice, ui, ctx);
                    })))
                },
            ))));
        }

        let (choice, time) = match self {
            Overlays::Inactive => {
                return None;
            }
            Overlays::ParkingAvailability(t, _) => ("parking availability", *t),
            Overlays::IntersectionDelay(t, _) => ("intersection delay", *t),
            Overlays::Throughput(t, _) => ("cumulative throughput", *t),
            Overlays::FinishedTrips(t, _) => ("finished trips", *t),
            Overlays::Chokepoints(t, _) => ("chokepoints", *t),
            Overlays::BikeNetwork(_) => ("bike network", ui.primary.sim.time()),
            Overlays::BusNetwork(_) => ("bus network", ui.primary.sim.time()),
            Overlays::BusRoute(_) | Overlays::BusDelaysOverTime(_) => {
                // The gameplay mode will update it.
                return None;
            }
        };
        if time != ui.primary.sim.time() {
            *self = Overlays::recalc(choice, ui, ctx);
        }
        None
    }

    // True if active and should block normal drawing
    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) -> bool {
        match self {
            Overlays::Inactive => false,
            Overlays::ParkingAvailability(_, ref heatmap)
            | Overlays::BikeNetwork(ref heatmap)
            | Overlays::BusNetwork(ref heatmap) => {
                heatmap.draw(g, ui);
                true
            }
            Overlays::IntersectionDelay(_, ref heatmap)
            | Overlays::Throughput(_, ref heatmap)
            | Overlays::Chokepoints(_, ref heatmap) => {
                heatmap.draw(g, ui);
                true
            }
            Overlays::FinishedTrips(_, ref s) => {
                ui.draw(
                    g,
                    DrawOptions::new(),
                    &ui.primary.sim,
                    &ShowEverything::new(),
                );
                s.draw(g);
                true
            }
            Overlays::BusDelaysOverTime(ref s) => {
                ui.draw(
                    g,
                    DrawOptions::new(),
                    &ui.primary.sim,
                    &ShowEverything::new(),
                );
                s.draw(g);
                true
            }
            Overlays::BusRoute(ref s) => {
                s.draw(g, ui);
                true
            }
        }
    }

    fn recalc(choice: &str, ui: &UI, ctx: &mut EventCtx) -> Overlays {
        let time = ui.primary.sim.time();
        match choice {
            "none" => Overlays::Inactive,
            "parking availability" => {
                Overlays::ParkingAvailability(time, calculate_parking_heatmap(ctx, ui))
            }
            "intersection delay" => {
                Overlays::IntersectionDelay(time, calculate_intersection_delay(ctx, ui))
            }
            "cumulative throughput" => Overlays::Throughput(time, calculate_thruput(ctx, ui)),
            "finished trips" => Overlays::FinishedTrips(time, trip_stats(ui, ctx)),
            "chokepoints" => Overlays::Chokepoints(time, calculate_chokepoints(ctx, ui)),
            "bike network" => Overlays::BikeNetwork(calculate_bike_network(ctx, ui)),
            "bus network" => Overlays::BusNetwork(calculate_bus_network(ctx, ui)),
            _ => unreachable!(),
        }
    }
}

fn calculate_parking_heatmap(ctx: &mut EventCtx, ui: &UI) -> RoadColorer {
    let (filled_spots, avail_spots) = ui.primary.sim.get_all_parking_spots();
    let mut txt = Text::prompt("parking availability");
    txt.add(Line(format!(
        "{} spots filled",
        prettyprint_usize(filled_spots.len())
    )));
    txt.add(Line(format!(
        "{} spots available ",
        prettyprint_usize(avail_spots.len())
    )));

    let awful = Color::BLACK;
    let bad = Color::RED;
    let meh = Color::YELLOW;
    let good = Color::GREEN;
    let mut colorer = RoadColorerBuilder::new(
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
        ParkingSpot::Offstreet(b, _) => ui
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
        colorer.add(l, color, &ui.primary.map);
    }

    colorer.build(ctx, &ui.primary.map)
}

fn calculate_intersection_delay(ctx: &mut EventCtx, ui: &UI) -> ObjectColorer {
    let fast = Color::GREEN;
    let meh = Color::YELLOW;
    let slow = Color::RED;
    let mut colorer = ObjectColorerBuilder::new(
        Text::prompt("intersection delay (90%ile)"),
        vec![("< 10s", fast), ("<= 60s", meh), ("> 60s", slow)],
    );

    for i in ui.primary.map.all_intersections() {
        let delays = ui.primary.sim.get_intersection_delays(i.id);
        if let Some(d) = delays.percentile(90.0) {
            let color = if d < Duration::seconds(10.0) {
                fast
            } else if d <= Duration::seconds(60.0) {
                meh
            } else {
                slow
            };
            colorer.add(ID::Intersection(i.id), color);
        }
    }

    colorer.build(ctx, &ui.primary.map)
}

fn calculate_chokepoints(ctx: &mut EventCtx, ui: &UI) -> ObjectColorer {
    const TOP_N: usize = 10;

    let mut colorer = ObjectColorerBuilder::new(
        Text::prompt("chokepoints"),
        vec![("chokepoint", Color::RED)],
    );

    let mut per_road = Counter::new();
    let mut per_intersection = Counter::new();

    for a in ui.primary.sim.active_agents() {
        // Why would an active agent not have a path? Pedestrian riding a bus.
        if let Some(path) = ui.primary.sim.get_path(a) {
            for step in path.get_steps() {
                match step {
                    PathStep::Lane(l) | PathStep::ContraflowLane(l) => {
                        per_road.inc(ui.primary.map.get_l(*l).parent);
                    }
                    PathStep::Turn(t) => {
                        per_intersection.inc(t.parent);
                    }
                }
            }
        }
    }

    let mut roads = per_road.sorted_asc();
    roads.reverse();
    for r in roads.into_iter().take(TOP_N) {
        colorer.add(ID::Road(*r), Color::RED);
    }

    let mut intersections = per_intersection.sorted_asc();
    intersections.reverse();
    for i in intersections.into_iter().take(TOP_N) {
        colorer.add(ID::Intersection(*i), Color::RED);
    }

    colorer.build(ctx, &ui.primary.map)
}

fn calculate_thruput(ctx: &mut EventCtx, ui: &UI) -> ObjectColorer {
    let light = Color::GREEN;
    let medium = Color::YELLOW;
    let heavy = Color::RED;
    let mut colorer = ObjectColorerBuilder::new(
        Text::prompt("Throughput"),
        vec![
            ("< 50%ile", light),
            ("< 90%ile", medium),
            (">= 90%ile", heavy),
        ],
    );

    let stats = &ui.primary.sim.get_analytics().thruput_stats;

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
            colorer.add(ID::Road(*r), color);
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
            colorer.add(ID::Intersection(*i), color);
        }
    }

    colorer.build(ctx, &ui.primary.map)
}

fn calculate_bike_network(ctx: &mut EventCtx, ui: &UI) -> RoadColorer {
    let mut colorer = RoadColorerBuilder::new(
        Text::prompt("bike networks"),
        vec![("bike lanes", Color::GREEN)],
    );
    for l in ui.primary.map.all_lanes() {
        if l.is_biking() {
            colorer.add(l.id, Color::GREEN, &ui.primary.map);
        }
    }
    colorer.build(ctx, &ui.primary.map)
}

fn calculate_bus_network(ctx: &mut EventCtx, ui: &UI) -> RoadColorer {
    let mut colorer = RoadColorerBuilder::new(
        Text::prompt("bus networks"),
        vec![("bike lanes", Color::GREEN)],
    );
    for l in ui.primary.map.all_lanes() {
        if l.lane_type == LaneType::Bus {
            colorer.add(l.id, Color::GREEN, &ui.primary.map);
        }
    }
    colorer.build(ctx, &ui.primary.map)
}

fn trip_stats(ui: &UI, ctx: &mut EventCtx) -> Plot<usize> {
    let lines: Vec<(&str, Color, Option<TripMode>)> = vec![
        (
            "walking",
            ui.cs.get("unzoomed pedestrian"),
            Some(TripMode::Walk),
        ),
        ("biking", ui.cs.get("unzoomed bike"), Some(TripMode::Bike)),
        (
            "transit",
            ui.cs.get("unzoomed bus"),
            Some(TripMode::Transit),
        ),
        ("driving", ui.cs.get("unzoomed car"), Some(TripMode::Drive)),
        ("aborted", Color::PURPLE.alpha(0.5), None),
    ];

    // What times do we use for interpolation?
    let num_x_pts = 100;
    let mut times = Vec::new();
    for i in 0..num_x_pts {
        let percent_x = (i as f64) / ((num_x_pts - 1) as f64);
        let t = ui.primary.sim.time() * percent_x;
        times.push(t);
    }

    // Gather the data
    let mut counts = Counter::new();
    let mut pts_per_mode: BTreeMap<Option<TripMode>, Vec<(Duration, usize)>> =
        lines.iter().map(|(_, _, m)| (*m, Vec::new())).collect();
    for (t, m, _) in &ui.primary.sim.get_analytics().finished_trips {
        counts.inc(*m);
        if *t > times[0] {
            times.remove(0);
            for (_, _, mode) in &lines {
                pts_per_mode
                    .get_mut(mode)
                    .unwrap()
                    .push((*t, counts.get(*mode)));
            }
        }
    }
    // Don't forget the last batch
    for (_, _, mode) in &lines {
        pts_per_mode
            .get_mut(mode)
            .unwrap()
            .push((ui.primary.sim.time(), counts.get(*mode)));
    }

    Plot::new(
        "finished trips",
        lines
            .into_iter()
            .map(|(name, color, m)| Series {
                label: name.to_string(),
                color,
                pts: pts_per_mode.remove(&m).unwrap(),
            })
            .collect(),
        0,
        ctx,
    )
}
