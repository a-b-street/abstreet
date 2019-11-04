use crate::game::{State, Transition, WizardState};
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{
    hotkey, Choice, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, ModalMenu, Text,
    VerticalAlignment,
};
use geom::{Duration, DurationHistogram, DurationStats};
use map_model::{BusRouteID, BusStopID};
use serde_derive::{Deserialize, Serialize};
use sim::{CarID, Sim, SimFlags, SimOptions, TripMode};
use std::collections::BTreeMap;

// TODO Also have some kind of screenshot to display for each challenge
#[derive(Clone)]
struct Challenge {
    title: String,
    description: String,
    map_name: String,
    gameplay: GameplayMode,
}
impl abstutil::Cloneable for Challenge {}

fn all_challenges() -> Vec<Challenge> {
    vec![
        Challenge {
            title: "Speed up route 48 (just Montlake area)".to_string(),
            description:
                "Decrease the average waiting time between all of route 48's stops by at least 30s"
                    .to_string(),
            map_name: "montlake".to_string(),
            gameplay: GameplayMode::OptimizeBus("48".to_string()),
        },
        Challenge {
            title: "Speed up route 48 (larger section)".to_string(),
            description:
                "Decrease the average waiting time between all of 48's stops by at least 30s"
                    .to_string(),
            map_name: "23rd".to_string(),
            gameplay: GameplayMode::OptimizeBus("48".to_string()),
        },
        Challenge {
            title: "Gridlock all of the everything".to_string(),
            description: "Make traffic as BAD as possible!".to_string(),
            map_name: "montlake".to_string(),
            gameplay: GameplayMode::CreateGridlock,
        },
        Challenge {
            title: "Speed up all bike trips".to_string(),
            description: "Reduce the 50%ile trip times of bikes by at least 1 minute".to_string(),
            map_name: "montlake".to_string(),
            gameplay: GameplayMode::FasterTrips(TripMode::Bike),
        },
        Challenge {
            title: "Speed up all car trips".to_string(),
            description: "Reduce the 50%ile trip times of drivers by at least 5 minutes"
                .to_string(),
            map_name: "montlake".to_string(),
            gameplay: GameplayMode::FasterTrips(TripMode::Drive),
        },
    ]
}

pub fn challenges_picker() -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, _| {
        let (_, challenge) = wiz.wrap(ctx).choose("Play which challenge?", || {
            all_challenges()
                .into_iter()
                .map(|c| Choice::new(c.title.clone(), c))
                .collect()
        })?;

        let mut summary = Text::from(Line(&challenge.description));
        summary.add(Line(""));
        summary.add(Line("Proposals:"));
        summary.add(Line(""));
        summary.add(Line("- example bus lane fix (untested)"));
        summary.add(Line("- example signal retiming (score 500)"));

        Some(Transition::Replace(Box::new(ChallengeSplash {
            summary,
            menu: ModalMenu::new(
                &challenge.title,
                vec![
                    (hotkey(Key::Escape), "back to challenges"),
                    (hotkey(Key::S), "start challenge"),
                    (hotkey(Key::L), "load existing proposal"),
                ],
                ctx,
            ),
            challenge,
        })))
    }))
}

struct ChallengeSplash {
    menu: ModalMenu,
    summary: Text,
    challenge: Challenge,
}

impl State for ChallengeSplash {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.event(ctx);
        if self.menu.action("back to challenges") {
            return Transition::Replace(challenges_picker());
        }
        if self.menu.action("start challenge") {
            if &self.challenge.map_name != ui.primary.map.get_name() {
                ctx.canvas.save_camera_state(ui.primary.map.get_name());
                let mut flags = ui.primary.current_flags.clone();
                flags.sim_flags.load = abstutil::path_map(&self.challenge.map_name);
                *ui = UI::new(flags, ctx, false);
            }
            return Transition::Replace(Box::new(SandboxMode::new(
                ctx,
                ui,
                self.challenge.gameplay.clone(),
            )));
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        g.draw_blocking_text(
            &self.summary,
            (HorizontalAlignment::Center, VerticalAlignment::Center),
        );
        self.menu.draw(g);
    }
}

// TODO Move all of this somewhere else, probably

pub fn prebake() {
    let mut timer = Timer::new("prebake all challenge results");

    timer.start("run normal sim");
    let (map, mut sim, _) = SimFlags {
        load: abstutil::path1_bin(
            "montlake",
            abstutil::SCENARIOS,
            "weekday_typical_traffic_from_psrc",
        ),
        use_map_fixes: true,
        rng_seed: Some(42),
        opts: SimOptions::new("prebaked"),
    }
    .load(&mut timer);
    sim.timed_step(&map, Duration::END_OF_DAY, &mut timer);
    timer.stop("run normal sim");

    let results = PrebakedResults {
        faster_trips: FasterTrips::from(&sim),
        gridlock_delays: GridlockDelays::from(&sim),
        bus_arrivals: BusArrivals::from(&sim),
    };
    abstutil::write_json("../data/prebaked_results.json", &results).unwrap();
}

// TODO Something more general?
// - key by GameplayMode (which needs map name too maybe)
// - different baselines/benchmarks
// TODO Actually, can we just store sim Analytics, and move all of this derived stuff there?
#[derive(Serialize, Deserialize)]
pub struct PrebakedResults {
    pub faster_trips: FasterTrips,
    pub gridlock_delays: GridlockDelays,
    pub bus_arrivals: BusArrivals,
}

#[derive(Serialize, Deserialize)]
pub struct FasterTrips(pub Vec<(Duration, Option<TripMode>, Duration)>);
impl FasterTrips {
    pub fn from(sim: &Sim) -> FasterTrips {
        FasterTrips(sim.get_analytics().finished_trips.clone())
    }

    pub fn to_stats(&self, now: Duration) -> BTreeMap<TripMode, DurationStats> {
        let mut distribs: BTreeMap<TripMode, DurationHistogram> = BTreeMap::new();
        for m in TripMode::all() {
            distribs.insert(m, DurationHistogram::new());
        }
        for (t, mode, dt) in &self.0 {
            if *t > now {
                break;
            }
            // Skip aborted trips
            if let Some(m) = mode {
                distribs.get_mut(&m).unwrap().add(*dt);
            }
        }
        let mut results = BTreeMap::new();
        for (m, distrib) in distribs {
            results.insert(m, distrib.to_stats());
        }
        results
    }
}

#[derive(Serialize, Deserialize)]
pub struct BusArrivals(pub Vec<(Duration, CarID, BusRouteID, BusStopID)>);
impl BusArrivals {
    pub fn from(sim: &Sim) -> BusArrivals {
        BusArrivals(sim.get_analytics().bus_arrivals.clone())
    }

    pub fn to_stats(&self, r: BusRouteID, now: Duration) -> BTreeMap<BusStopID, DurationStats> {
        let mut per_bus: BTreeMap<CarID, Vec<(Duration, BusStopID)>> = BTreeMap::new();
        for (t, car, route, stop) in &self.0 {
            if *t > now {
                break;
            }
            if *route == r {
                per_bus
                    .entry(*car)
                    .or_insert_with(Vec::new)
                    .push((*t, *stop));
            }
        }
        let mut delay_to_stop: BTreeMap<BusStopID, DurationHistogram> = BTreeMap::new();
        for events in per_bus.values() {
            for pair in events.windows(2) {
                delay_to_stop
                    .entry(pair[1].1)
                    .or_insert_with(DurationHistogram::new)
                    .add(pair[1].0 - pair[0].0);
            }
        }
        delay_to_stop
            .into_iter()
            .map(|(k, v)| (k, v.to_stats()))
            .collect()
    }
}

#[derive(Serialize, Deserialize)]
pub struct GridlockDelays {
    pub lt_1m: usize,
    pub lt_5m: usize,
    pub stuck: usize,
}
impl GridlockDelays {
    pub fn from(sim: &Sim) -> GridlockDelays {
        let mut delays = GridlockDelays {
            lt_1m: 0,
            lt_5m: 0,
            stuck: 0,
        };
        for a in sim.get_agent_metadata() {
            if a.time_spent_blocked < Duration::minutes(1) {
                delays.lt_1m += 1;
            } else if a.time_spent_blocked < Duration::minutes(5) {
                delays.lt_5m += 1;
            } else {
                delays.stuck += 1;
            }
        }
        delays
    }
}
