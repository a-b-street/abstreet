use super::trip_stats::{ShowTripStats, TripStats};
use crate::common::{ObjectColorer, ObjectColorerBuilder, RoadColorer, RoadColorerBuilder};
use crate::game::{Transition, WizardState};
use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::sandbox::SandboxMode;
use crate::ui::{ShowEverything, UI};
use abstutil::{prettyprint_usize, Counter};
use ezgui::{Choice, Color, EventCtx, GfxCtx, Line, ModalMenu, Text};
use geom::Duration;
use map_model::PathStep;
use sim::ParkingSpot;
use std::collections::HashSet;

pub enum Analytics {
    Inactive,
    ParkingAvailability(Duration, RoadColorer),
    IntersectionDelay(Duration, ObjectColorer),
    Throughput(Duration, ObjectColorer),
    FinishedTrips(Duration, ShowTripStats),
    Chokepoints(Duration, ObjectColorer),
    BikeNetwork(RoadColorer),
}

impl Analytics {
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &UI,
        menu: &mut ModalMenu,
        trip_stats: &TripStats,
    ) -> Option<Transition> {
        if menu.action("change analytics overlay") {
            return Some(Transition::Push(WizardState::new(Box::new(
                |wiz, ctx, _| {
                    let (choice, _) =
                        wiz.wrap(ctx).choose("Show which analytics overlay?", || {
                            // TODO Filter out the current
                            vec![
                                Choice::new("none", ()),
                                Choice::new("parking availability", ()),
                                Choice::new("intersection delay", ()),
                                Choice::new("cumulative throughput", ()),
                                Choice::new("finished trips", ()),
                                Choice::new("chokepoints", ()),
                                Choice::new("bike network", ()),
                            ]
                        })?;
                    Some(Transition::PopWithData(Box::new(move |state, ui, ctx| {
                        let mut sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                        sandbox.analytics =
                            Analytics::recalc(&choice, &sandbox.trip_stats, ui, ctx);
                    })))
                },
            ))));
        }

        let (choice, time) = match self {
            Analytics::Inactive => {
                return None;
            }
            Analytics::ParkingAvailability(t, _) => ("parking availability", *t),
            Analytics::IntersectionDelay(t, _) => ("intersection delay", *t),
            Analytics::Throughput(t, _) => ("cumulative throughput", *t),
            Analytics::FinishedTrips(t, _) => ("finished trips", *t),
            Analytics::Chokepoints(t, _) => ("chokepoints", *t),
            Analytics::BikeNetwork(_) => ("bike network", ui.primary.sim.time()),
        };
        if time != ui.primary.sim.time() {
            *self = Analytics::recalc(choice, trip_stats, ui, ctx);
        }
        None
    }

    // True if active and should block normal drawing
    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) -> bool {
        match self {
            Analytics::Inactive => false,
            Analytics::ParkingAvailability(_, ref heatmap)
            | Analytics::BikeNetwork(ref heatmap) => {
                heatmap.draw(g, ui);
                true
            }
            Analytics::IntersectionDelay(_, ref heatmap)
            | Analytics::Throughput(_, ref heatmap)
            | Analytics::Chokepoints(_, ref heatmap) => {
                heatmap.draw(g, ui);
                true
            }
            Analytics::FinishedTrips(_, ref s) => {
                ui.draw(
                    g,
                    DrawOptions::new(),
                    &ui.primary.sim,
                    &ShowEverything::new(),
                );
                s.draw(g);
                true
            }
        }
    }

    fn recalc(choice: &str, trip_stats: &TripStats, ui: &UI, ctx: &mut EventCtx) -> Analytics {
        let time = ui.primary.sim.time();
        match choice {
            "none" => Analytics::Inactive,
            "parking availability" => {
                Analytics::ParkingAvailability(time, calculate_parking_heatmap(ctx, ui))
            }
            "intersection delay" => {
                Analytics::IntersectionDelay(time, calculate_intersection_delay(ctx, ui))
            }
            "cumulative throughput" => Analytics::Throughput(time, calculate_thruput(ctx, ui)),
            "finished trips" => {
                if let Some(s) = ShowTripStats::new(trip_stats, ui, ctx) {
                    Analytics::FinishedTrips(time, s)
                } else {
                    println!("No trip stats available. Pass --record_stats or make sure at least one trip is done.");
                    Analytics::Inactive
                }
            }
            "chokepoints" => Analytics::Chokepoints(time, calculate_chokepoints(ctx, ui)),
            "bike network" => Analytics::BikeNetwork(calculate_bike_network(ctx, ui)),
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
