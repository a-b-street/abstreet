use crate::common::{CommonState, Warping};
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::mission::pick_time_range;
use crate::sandbox::SandboxMode;
use crate::ui::UI;
use abstutil::{prettyprint_usize, MultiMap, WeightedUsizeChoice};
use ezgui::{hotkey, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Text, Wizard, WrappedWizard};
use geom::Duration;
use map_model::{BuildingID, IntersectionID, Map, Neighborhood};
use sim::{
    BorderSpawnOverTime, DrivingGoal, OriginDestination, Scenario, SeedParkedCars, SidewalkPOI,
    SidewalkSpot, SpawnOverTime, SpawnTrip,
};
use std::collections::{BTreeSet, HashMap};

pub struct ScenarioManager {
    menu: ModalMenu,
    common: CommonState,
    scenario: Scenario,

    // The usizes are indices into scenario.individ_trips
    trips_from_bldg: MultiMap<BuildingID, usize>,
    trips_to_bldg: MultiMap<BuildingID, usize>,
    cars_needed_per_bldg: HashMap<BuildingID, usize>,
    total_cars_needed: usize,
}

impl ScenarioManager {
    pub fn new(scenario: Scenario, ctx: &mut EventCtx) -> ScenarioManager {
        let mut trips_from_bldg = MultiMap::new();
        let mut trips_to_bldg = MultiMap::new();
        let mut cars_needed_per_bldg = HashMap::new();
        let mut total_cars_needed = 0;
        for (idx, trip) in scenario.individ_trips.iter().enumerate() {
            // trips_from_bldg
            match trip {
                SpawnTrip::CarAppearing { ref start_bldg, .. } => {
                    if let Some(b) = start_bldg {
                        trips_from_bldg.insert(*b, idx);
                    }
                }
                SpawnTrip::UsingBike(_, ref spot, _)
                | SpawnTrip::JustWalking(_, ref spot, _)
                | SpawnTrip::UsingTransit(_, ref spot, _, _, _, _) => {
                    if let SidewalkPOI::Building(b) = spot.connection {
                        trips_from_bldg.insert(b, idx);
                    }
                }
            }

            // trips_to_bldg
            match trip {
                SpawnTrip::CarAppearing { ref goal, .. } | SpawnTrip::UsingBike(_, _, ref goal) => {
                    if let DrivingGoal::ParkNear(b) = goal {
                        trips_to_bldg.insert(*b, idx);
                    }
                }
                SpawnTrip::JustWalking(_, _, ref spot)
                | SpawnTrip::UsingTransit(_, _, ref spot, _, _, _) => {
                    if let SidewalkPOI::Building(b) = spot.connection {
                        trips_to_bldg.insert(b, idx);
                    }
                }
            }

            // Parked cars
            if let SpawnTrip::CarAppearing {
                is_bike,
                ref start_bldg,
                ..
            } = trip
            {
                if !is_bike {
                    if let Some(b) = start_bldg {
                        *cars_needed_per_bldg.entry(*b).or_insert(0) += 1;
                        total_cars_needed += 1;
                    }
                }
            }
        }

        ScenarioManager {
            menu: ModalMenu::new(
                "Scenario Editor",
                vec![
                    vec![
                        (hotkey(Key::S), "save"),
                        (hotkey(Key::E), "edit"),
                        (hotkey(Key::I), "instantiate"),
                    ],
                    vec![
                        (hotkey(Key::Escape), "quit"),
                        (hotkey(Key::J), "warp"),
                        (hotkey(Key::K), "navigate"),
                        (hotkey(Key::SingleQuote), "shortcuts"),
                        (hotkey(Key::F1), "take a screenshot"),
                    ],
                ],
                ctx,
            ),
            common: CommonState::new(),
            scenario,
            trips_from_bldg,
            trips_to_bldg,
            cars_needed_per_bldg,
            total_cars_needed,
        }
    }
}

impl State for ScenarioManager {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        // TODO Calculate this once? Except when we modify it, nice to automatically pick up
        // changes...
        {
            let mut txt = Text::prompt("Scenario Editor");
            txt.add_line(self.scenario.scenario_name.clone());
            for line in self.scenario.describe() {
                txt.add_line(line);
            }
            txt.add_line(format!(
                "{} total parked cars needed",
                prettyprint_usize(self.total_cars_needed)
            ));
            self.menu.handle_event(ctx, Some(txt));
        }
        ctx.canvas.handle_event(ctx.input);
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        if let Some(t) = self.common.event(ctx, ui, &mut self.menu) {
            return t;
        }

        if self.menu.action("quit") {
            return Transition::Pop;
        } else if self.menu.action("save") {
            self.scenario.save();
        } else if self.menu.action("edit") {
            return Transition::Push(Box::new(ScenarioEditor {
                scenario: self.scenario.clone(),
                wizard: Wizard::new(),
            }));
        } else if self.menu.action("instantiate") {
            ctx.loading_screen("instantiate scenario", |_, timer| {
                self.scenario.instantiate(
                    &mut ui.primary.sim,
                    &ui.primary.map,
                    &mut ui.primary.current_flags.sim_flags.make_rng(),
                    timer,
                );
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
            });
            return Transition::Replace(Box::new(SandboxMode::new(ctx)));
        }

        if let Some(ID::Building(b)) = ui.primary.current_selection {
            let from = self.trips_from_bldg.get(b);
            let to = self.trips_to_bldg.get(b);
            if (!from.is_empty() || !to.is_empty())
                && ctx.input.contextual_action(Key::T, "browse trips")
            {
                // TODO Avoid the clone? Just happens once though.
                let mut all_trips = from.clone();
                all_trips.extend(to);

                return Transition::Push(make_trip_picker(self.scenario.clone(), all_trips, b));
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.menu.draw(g);
        // TODO Weird to not draw common (turn cycler), but we want the custom OSD...

        if let Some(ID::Building(b)) = ui.primary.current_selection {
            let mut osd = Text::new();
            osd.append(format!("{}", b), Some(ui.cs.get("OSD ID color")));
            osd.append(" is ".to_string(), None);
            osd.append(
                ui.primary.map.get_b(b).get_name(),
                Some(ui.cs.get("OSD name color")),
            );
            let from = self.trips_from_bldg.get(b);
            let to = self.trips_to_bldg.get(b);
            osd.append(
                format!(
                    ". {} trips from here, {} trips to here, {} parked cars needed",
                    from.len(),
                    to.len(),
                    self.cars_needed_per_bldg.get(&b).unwrap_or(&0)
                ),
                None,
            );
            CommonState::draw_custom_osd(g, osd);
        } else {
            CommonState::draw_osd(g, ui, &ui.primary.current_selection);
        }
    }
}

struct ScenarioEditor {
    scenario: Scenario,
    wizard: Wizard,
}

impl State for ScenarioEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if let Some(()) = edit_scenario(&ui.primary.map, &mut self.scenario, self.wizard.wrap(ctx))
        {
            // TODO autosave, or at least make it clear there are unsaved edits
            let scenario = self.scenario.clone();
            return Transition::PopWithData(Box::new(|state, _, _| {
                let mut manager = state.downcast_mut::<ScenarioManager>().unwrap();
                manager.scenario = scenario;
                // Don't need to update trips_from_bldg or trips_to_bldg, since edit_scenario
                // doesn't touch individ_trips.
            }));
        } else if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if let Some(neighborhood) = self.wizard.current_menu_choice::<Neighborhood>() {
            g.draw_polygon(ui.cs.get("neighborhood polygon"), &neighborhood.polygon);
        }
        self.wizard.draw(g);
    }
}

fn edit_scenario(map: &Map, scenario: &mut Scenario, mut wizard: WrappedWizard) -> Option<()> {
    let seed_parked = "Seed parked cars";
    let spawn = "Spawn agents";
    let spawn_border = "Spawn agents from a border";
    let randomize = "Randomly spawn stuff from/to every neighborhood";
    match wizard
        .choose_str(
            "What kind of edit?",
            vec![seed_parked, spawn, spawn_border, randomize],
        )?
        .as_str()
    {
        x if x == seed_parked => {
            scenario.seed_parked_cars.push(SeedParkedCars {
                neighborhood: choose_neighborhood(
                    map,
                    &mut wizard,
                    "Seed parked cars in what area?",
                )?,
                cars_per_building: input_weighted_usize(
                    &mut wizard,
                    "How many cars per building? (ex: 4,4,2)",
                )?,
            });
        }
        x if x == spawn => {
            let (start_time, stop_time) =
                pick_time_range(&mut wizard, "Start spawning when?", "Stop spawning when?")?;
            scenario.spawn_over_time.push(SpawnOverTime {
                num_agents: wizard.input_usize("Spawn how many agents?")?,
                start_time,
                stop_time,
                start_from_neighborhood: choose_neighborhood(
                    map,
                    &mut wizard,
                    "Where should the agents start?",
                )?,
                goal: choose_origin_destination(map, &mut wizard, "Where should the agents go?")?,
                percent_biking: wizard
                    .input_percent("What percent of the walking trips will bike instead?")?,
                percent_use_transit: wizard.input_percent(
                    "What percent of the walking trips will consider taking transit?",
                )?,
            });
        }
        x if x == spawn_border => {
            let (start_time, stop_time) =
                pick_time_range(&mut wizard, "Start spawning when?", "Stop spawning when?")?;
            scenario.border_spawn_over_time.push(BorderSpawnOverTime {
                num_peds: wizard.input_usize("Spawn how many pedestrians?")?,
                num_cars: wizard.input_usize("Spawn how many cars?")?,
                num_bikes: wizard.input_usize("Spawn how many bikes?")?,
                start_time,
                stop_time,
                // TODO validate it's a border!
                start_from_border: choose_intersection(
                    &mut wizard,
                    "Which border should the agents spawn at?",
                )?,
                goal: choose_origin_destination(map, &mut wizard, "Where should the agents go?")?,
                percent_use_transit: wizard.input_percent(
                    "What percent of the walking trips will consider taking transit?",
                )?,
            });
        }
        x if x == randomize => {
            let neighborhoods = Neighborhood::load_all(map.get_name(), &map.get_gps_bounds());
            for (src, _) in &neighborhoods {
                for (dst, _) in &neighborhoods {
                    scenario.spawn_over_time.push(SpawnOverTime {
                        num_agents: 100,
                        start_time: Duration::ZERO,
                        stop_time: Duration::minutes(10),
                        start_from_neighborhood: src.to_string(),
                        goal: OriginDestination::Neighborhood(dst.to_string()),
                        percent_biking: 0.1,
                        percent_use_transit: 0.2,
                    });
                }
            }
        }
        _ => unreachable!(),
    };
    Some(())
}

fn choose_neighborhood(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    // Load the full object, since we usually visualize the neighborhood when menuing over it
    wizard
        .choose_something(query, || {
            Neighborhood::load_all(map.get_name(), map.get_gps_bounds())
        })
        .map(|(n, _)| n)
}

fn input_weighted_usize(wizard: &mut WrappedWizard, query: &str) -> Option<WeightedUsizeChoice> {
    wizard.input_something(
        query,
        None,
        Box::new(|line| WeightedUsizeChoice::parse(&line)),
    )
}

// TODO Validate the intersection exists? Let them pick it with the cursor?
fn choose_intersection(wizard: &mut WrappedWizard, query: &str) -> Option<IntersectionID> {
    wizard.input_something(
        query,
        None,
        Box::new(|line| usize::from_str_radix(&line, 10).ok().map(IntersectionID)),
    )
}

fn choose_origin_destination(
    map: &Map,
    wizard: &mut WrappedWizard,
    query: &str,
) -> Option<OriginDestination> {
    let neighborhood = "Neighborhood";
    let border = "Border intersection";
    if wizard.choose_str(query, vec![neighborhood, border])? == neighborhood {
        choose_neighborhood(map, wizard, query).map(OriginDestination::Neighborhood)
    } else {
        choose_intersection(wizard, query).map(OriginDestination::Border)
    }
}

fn make_trip_picker(
    scenario: Scenario,
    indices: BTreeSet<usize>,
    home: BuildingID,
) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, ui| {
        let warp_to = wiz
            .wrap(ctx)
            .choose_something("Trips from/to this building", || {
                // TODO Panics if there are two duplicate trips (b1124 in montlake)
                indices
                    .iter()
                    .map(|idx| {
                        let trip = &scenario.individ_trips[*idx];
                        (describe(trip, home), other_endpt(trip, home))
                    })
                    .collect()
            })?
            .1;
        Some(Transition::ReplaceWithMode(
            Warping::new(
                ctx,
                warp_to.canonical_point(&ui.primary).unwrap(),
                None,
                Some(warp_to),
                &mut ui.primary,
            ),
            EventLoopMode::Animation,
        ))
    }))
}

fn describe(trip: &SpawnTrip, home: BuildingID) -> String {
    let driving_goal = |goal: &DrivingGoal| match goal {
        DrivingGoal::ParkNear(b) => {
            if *b == home {
                "HERE".to_string()
            } else {
                b.to_string()
            }
        }
        DrivingGoal::Border(i, _) => i.to_string(),
    };
    let sidewalk_spot = |spot: &SidewalkSpot| match &spot.connection {
        SidewalkPOI::Building(b) => {
            if *b == home {
                "HERE".to_string()
            } else {
                b.to_string()
            }
        }
        SidewalkPOI::Border(i) => i.to_string(),
        x => format!("{:?}", x),
    };

    match trip {
        SpawnTrip::CarAppearing {
            depart,
            start,
            goal,
            is_bike,
            start_bldg,
        } => format!(
            "{}: {} appears {}, goes to {}",
            depart,
            if *is_bike { "bike" } else { "car" },
            if start_bldg == &Some(home) {
                "HERE".to_string()
            } else {
                format!("at {}", start.lane())
            },
            driving_goal(goal)
        ),
        SpawnTrip::UsingBike(depart, start, goal) => format!(
            "{}: bike from {} to {}",
            depart,
            sidewalk_spot(start),
            driving_goal(goal)
        ),
        SpawnTrip::JustWalking(depart, start, goal) => format!(
            "{}: walk from {} to {}",
            depart,
            sidewalk_spot(start),
            sidewalk_spot(goal)
        ),
        SpawnTrip::UsingTransit(depart, start, goal, route, _, _) => format!(
            "{}: bus from {} to {} using {}",
            depart,
            sidewalk_spot(start),
            sidewalk_spot(goal),
            route
        ),
    }
}

fn other_endpt(trip: &SpawnTrip, home: BuildingID) -> ID {
    let driving_goal = |goal: &DrivingGoal| match goal {
        DrivingGoal::ParkNear(b) => ID::Building(*b),
        DrivingGoal::Border(i, _) => ID::Intersection(*i),
    };
    let sidewalk_spot = |spot: &SidewalkSpot| match &spot.connection {
        SidewalkPOI::Building(b) => ID::Building(*b),
        SidewalkPOI::Border(i) => ID::Intersection(*i),
        x => panic!("other_endpt for {:?}?", x),
    };

    let (from, to) = match trip {
        SpawnTrip::CarAppearing {
            start,
            goal,
            start_bldg,
            ..
        } => (
            start_bldg
                .map(|b| ID::Building(b))
                .unwrap_or(ID::Lane(start.lane())),
            driving_goal(goal),
        ),
        SpawnTrip::UsingBike(_, start, goal) => (sidewalk_spot(start), driving_goal(goal)),
        SpawnTrip::JustWalking(_, start, goal) => (sidewalk_spot(start), sidewalk_spot(goal)),
        SpawnTrip::UsingTransit(_, start, goal, _, _, _) => {
            (sidewalk_spot(start), sidewalk_spot(goal))
        }
    };
    if from == ID::Building(home) {
        to
    } else if to == ID::Building(home) {
        from
    } else {
        panic!("other_endpt broke when homed at {} for {:?}", home, trip)
    }
}
