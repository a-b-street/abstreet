use super::trip_stats::{ShowTripStats, TripStats};
use crate::common::{ObjectColorer, ObjectColorerBuilder, RoadColorer, RoadColorerBuilder};
use crate::game::{Transition, WizardState};
use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::sandbox::SandboxMode;
use crate::ui::{ShowEverything, UI};
use abstutil::Counter;
use ezgui::{Choice, Color, EventCtx, GfxCtx, ModalMenu};
use geom::Duration;
use map_model::{IntersectionID, RoadID, Traversable};
use sim::{Event, ParkingSpot};
use std::collections::HashSet;

pub enum Analytics {
    Inactive,
    ParkingAvailability(Duration, RoadColorer),
    IntersectionDelay(Duration, ObjectColorer),
    Throughput(Duration, ObjectColorer),
    FinishedTrips(Duration, ShowTripStats),
}

impl Analytics {
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &UI,
        menu: &mut ModalMenu,
        thruput_stats: &ThruputStats,
        trip_stats: &TripStats,
    ) -> Option<Transition> {
        if menu.action("change analytics overlay") {
            return Some(Transition::Push(WizardState::new(Box::new(
                |wiz, ctx, _| {
                    let (choice, _) =
                        wiz.wrap(ctx).choose("Show which analytics overlay?", || {
                            vec![
                                Choice::new("none", ()),
                                Choice::new("parking availability", ()),
                                Choice::new("intersection delay", ()),
                                Choice::new("cumulative throughput", ()),
                                Choice::new("finished trips", ()),
                            ]
                        })?;
                    Some(Transition::PopWithData(Box::new(move |state, ui, ctx| {
                        let mut sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                        sandbox.analytics = Analytics::recalc(
                            &choice,
                            &sandbox.thruput_stats,
                            &sandbox.trip_stats,
                            ui,
                            ctx,
                        );
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
        };
        if time != ui.primary.sim.time() {
            *self = Analytics::recalc(choice, thruput_stats, trip_stats, ui, ctx);
        }
        None
    }

    // True if active and should block normal drawing
    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) -> bool {
        match self {
            Analytics::Inactive => false,
            Analytics::ParkingAvailability(_, ref heatmap) => {
                heatmap.draw(g, ui);
                true
            }
            Analytics::IntersectionDelay(_, ref heatmap)
            | Analytics::Throughput(_, ref heatmap) => {
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

    fn recalc(
        choice: &str,
        thruput_stats: &ThruputStats,
        trip_stats: &TripStats,
        ui: &UI,
        ctx: &mut EventCtx,
    ) -> Analytics {
        let time = ui.primary.sim.time();
        match choice {
            "none" => Analytics::Inactive,
            "parking availability" => {
                Analytics::ParkingAvailability(time, calculate_parking_heatmap(ctx, ui))
            }
            "intersection delay" => {
                Analytics::IntersectionDelay(time, calculate_intersection_delay(ctx, ui))
            }
            "cumulative throughput" => {
                Analytics::Throughput(time, calculate_thruput(thruput_stats, ctx, ui))
            }
            "finished trips" => {
                if let Some(s) = ShowTripStats::new(trip_stats, ui, ctx) {
                    Analytics::FinishedTrips(time, s)
                } else {
                    println!("No trip stats available. Pass --record_stats or make sure at least one trip is done.");
                    Analytics::Inactive
                }
            }
            _ => unreachable!(),
        }
    }
}

fn calculate_parking_heatmap(ctx: &mut EventCtx, ui: &UI) -> RoadColorer {
    let awful = Color::BLACK;
    let bad = Color::RED;
    let meh = Color::YELLOW;
    let good = Color::GREEN;
    let mut colorer = RoadColorerBuilder::new(
        "parking availability",
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
    let (filled_spots, avail_spots) = ui.primary.sim.get_all_parking_spots();
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
        "intersection delay (90%ile)",
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

fn calculate_thruput(stats: &ThruputStats, ctx: &mut EventCtx, ui: &UI) -> ObjectColorer {
    let light = Color::GREEN;
    let medium = Color::YELLOW;
    let heavy = Color::RED;
    let mut colorer = ObjectColorerBuilder::new(
        "Throughput",
        vec![
            ("< 50%ile", light),
            ("< 90%ile", medium),
            (">= 90%ile", heavy),
        ],
    );

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

pub struct ThruputStats {
    count_per_road: Counter<RoadID>,
    count_per_intersection: Counter<IntersectionID>,
}

impl ThruputStats {
    pub fn new() -> ThruputStats {
        ThruputStats {
            count_per_road: Counter::new(),
            count_per_intersection: Counter::new(),
        }
    }

    pub fn record(&mut self, ui: &mut UI) {
        for ev in ui.primary.sim.collect_events() {
            if let Event::AgentEntersTraversable(_, to) = ev {
                match to {
                    Traversable::Lane(l) => self.count_per_road.inc(ui.primary.map.get_l(l).parent),
                    Traversable::Turn(t) => self.count_per_intersection.inc(t.parent),
                };
            }
        }
    }
}
