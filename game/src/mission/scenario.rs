use crate::common::{CommonState, ObjectColorer, ObjectColorerBuilder, Warping};
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::mission::pick_time_range;
use crate::sandbox::SandboxMode;
use crate::ui::UI;
use abstutil::{prettyprint_usize, MultiMap, WeightedUsizeChoice};
use ezgui::{
    hotkey, Choice, Color, EventCtx, EventLoopMode, GfxCtx, Key, Line, ModalMenu, Text, Wizard,
    WrappedWizard,
};
use geom::Duration;
use map_model::{BuildingID, IntersectionID, Map, Neighborhood};
use sim::{
    BorderSpawnOverTime, DrivingGoal, OriginDestination, Scenario, SeedParkedCars, SidewalkPOI,
    SidewalkSpot, SpawnOverTime, SpawnTrip,
};
use std::collections::{BTreeSet, HashMap};
use std::fmt;

pub struct ScenarioManager {
    menu: ModalMenu,
    common: CommonState,
    scenario: Scenario,

    // The usizes are indices into scenario.individ_trips
    trips_from_bldg: MultiMap<BuildingID, usize>,
    trips_to_bldg: MultiMap<BuildingID, usize>,
    trips_from_border: MultiMap<IntersectionID, usize>,
    trips_to_border: MultiMap<IntersectionID, usize>,
    cars_needed_per_bldg: HashMap<BuildingID, CarCount>,
    total_cars_needed: CarCount,
    total_parking_spots: usize,
    bldg_colors: ObjectColorer,
}

impl ScenarioManager {
    pub fn new(scenario: Scenario, ctx: &mut EventCtx, ui: &UI) -> ScenarioManager {
        let mut trips_from_bldg = MultiMap::new();
        let mut trips_to_bldg = MultiMap::new();
        let mut trips_from_border = MultiMap::new();
        let mut trips_to_border = MultiMap::new();
        let mut cars_needed_per_bldg = HashMap::new();
        for b in ui.primary.map.all_buildings() {
            cars_needed_per_bldg.insert(b.id, CarCount::new());
        }
        let mut total_cars_needed = CarCount::new();
        let color = Color::BLUE;
        let mut bldg_colors =
            ObjectColorerBuilder::new("trips", vec![("building with trips from/to it", color)]);
        for (idx, trip) in scenario.individ_trips.iter().enumerate() {
            // trips_from_bldg and trips_from_border
            match trip {
                // TODO CarAppearing might be from a border
                SpawnTrip::CarAppearing { .. } => {}
                SpawnTrip::MaybeUsingParkedCar(_, b, _) => {
                    trips_from_bldg.insert(*b, idx);
                    bldg_colors.add(ID::Building(*b), color);
                }
                SpawnTrip::UsingBike(_, ref spot, _)
                | SpawnTrip::JustWalking(_, ref spot, _)
                | SpawnTrip::UsingTransit(_, ref spot, _, _, _, _) => match spot.connection {
                    SidewalkPOI::Building(b) => {
                        trips_from_bldg.insert(b, idx);
                        bldg_colors.add(ID::Building(b), color);
                    }
                    SidewalkPOI::Border(i) => {
                        trips_from_border.insert(i, idx);
                    }
                    _ => {}
                },
            }

            // trips_to_bldg and trips_to_border
            match trip {
                SpawnTrip::CarAppearing { ref goal, .. }
                | SpawnTrip::MaybeUsingParkedCar(_, _, ref goal)
                | SpawnTrip::UsingBike(_, _, ref goal) => match goal {
                    DrivingGoal::ParkNear(b) => {
                        trips_to_bldg.insert(*b, idx);
                        bldg_colors.add(ID::Building(*b), color);
                    }
                    DrivingGoal::Border(i, _) => {
                        trips_to_border.insert(*i, idx);
                    }
                },
                SpawnTrip::JustWalking(_, _, ref spot)
                | SpawnTrip::UsingTransit(_, _, ref spot, _, _, _) => match spot.connection {
                    SidewalkPOI::Building(b) => {
                        trips_to_bldg.insert(b, idx);
                        bldg_colors.add(ID::Building(b), color);
                    }
                    SidewalkPOI::Border(i) => {
                        trips_to_border.insert(i, idx);
                    }
                    _ => {}
                },
            }

            // Parked cars
            if let SpawnTrip::MaybeUsingParkedCar(_, start_bldg, ref goal) = trip {
                let mut cnt = cars_needed_per_bldg.get_mut(start_bldg).unwrap();

                cnt.naive += 1;
                total_cars_needed.naive += 1;

                if cnt.available > 0 {
                    cnt.available -= 1;
                } else {
                    cnt.recycle += 1;
                    total_cars_needed.recycle += 1;
                }

                // Cars appearing at borders and driving in contribute parked cars.
                if let DrivingGoal::ParkNear(b) = goal {
                    cars_needed_per_bldg.get_mut(b).unwrap().available += 1;
                }
            }
        }

        let (filled_spots, free_parking_spots) = ui.primary.sim.get_all_parking_spots();
        assert!(filled_spots.is_empty());

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
            trips_from_border,
            trips_to_border,
            cars_needed_per_bldg,
            total_cars_needed,
            total_parking_spots: free_parking_spots.len(),
            bldg_colors: bldg_colors.build(ctx, &ui.primary.map),
        }
    }
}

impl State for ScenarioManager {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        // TODO Calculate this once? Except when we modify it, nice to automatically pick up
        // changes...
        {
            let mut txt = Text::prompt("Scenario Editor");
            txt.add(Line(&self.scenario.scenario_name));
            for line in self.scenario.describe() {
                txt.add(Line(line));
            }
            txt.add(Line(format!(
                "{} total parked cars needed, {} spots",
                self.total_cars_needed,
                prettyprint_usize(self.total_parking_spots),
            )));
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

                return Transition::Push(make_trip_picker(
                    self.scenario.clone(),
                    all_trips,
                    "building",
                    OD::Bldg(b),
                ));
            }
        } else if let Some(ID::Intersection(i)) = ui.primary.current_selection {
            let from = self.trips_from_border.get(i);
            let to = self.trips_to_border.get(i);
            if (!from.is_empty() || !to.is_empty())
                && ctx.input.contextual_action(Key::T, "browse trips")
            {
                // TODO Avoid the clone? Just happens once though.
                let mut all_trips = from.clone();
                all_trips.extend(to);

                return Transition::Push(make_trip_picker(
                    self.scenario.clone(),
                    all_trips,
                    "border",
                    OD::Border(i),
                ));
            }
        }

        Transition::Keep
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // TODO Let common contribute draw_options...
        self.bldg_colors.draw(g, ui);

        self.menu.draw(g);
        // TODO Weird to not draw common (turn cycler), but we want the custom OSD...

        if let Some(ID::Building(b)) = ui.primary.current_selection {
            let mut osd = CommonState::default_osd(ID::Building(b), ui);
            osd.append(Line(format!(
                ". {} trips from here, {} trips to here, {} parked cars needed",
                self.trips_from_bldg.get(b).len(),
                self.trips_to_bldg.get(b).len(),
                self.cars_needed_per_bldg[&b]
            )));
            CommonState::draw_custom_osd(g, osd);
        } else if let Some(ID::Intersection(i)) = ui.primary.current_selection {
            let mut osd = CommonState::default_osd(ID::Intersection(i), ui);
            osd.append(Line(format!(
                ". {} trips from here, {} trips to here",
                self.trips_from_border.get(i).len(),
                self.trips_to_border.get(i).len(),
            )));
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
        .choose_string("What kind of edit?", || {
            vec![seed_parked, spawn, spawn_border, randomize]
        })?
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
        .choose(query, || {
            Choice::from(Neighborhood::load_all(map.get_name(), map.get_gps_bounds()))
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
    if wizard.choose_string(query, || vec![neighborhood, border])? == neighborhood {
        choose_neighborhood(map, wizard, query).map(OriginDestination::Neighborhood)
    } else {
        choose_intersection(wizard, query).map(OriginDestination::Border)
    }
}

// TODO Yet another one of these... something needs to change.
#[derive(PartialEq, Debug, Clone, Copy)]
enum OD {
    Bldg(BuildingID),
    Border(IntersectionID),
}

fn make_trip_picker(
    scenario: Scenario,
    indices: BTreeSet<usize>,
    noun: &'static str,
    home: OD,
) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, ui| {
        let warp_to = wiz
            .wrap(ctx)
            .choose(&format!("Trips from/to this {}", noun), || {
                // TODO Panics if there are two duplicate trips (b1124 in montlake)
                indices
                    .iter()
                    .map(|idx| {
                        let trip = &scenario.individ_trips[*idx];
                        Choice::new(describe(trip, home), other_endpt(trip, home))
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

fn describe(trip: &SpawnTrip, home: OD) -> String {
    let driving_goal = |goal: &DrivingGoal| match goal {
        DrivingGoal::ParkNear(b) => {
            if OD::Bldg(*b) == home {
                "HERE".to_string()
            } else {
                b.to_string()
            }
        }
        DrivingGoal::Border(i, _) => {
            if OD::Border(*i) == home {
                "HERE".to_string()
            } else {
                i.to_string()
            }
        }
    };
    let sidewalk_spot = |spot: &SidewalkSpot| match &spot.connection {
        SidewalkPOI::Building(b) => {
            if OD::Bldg(*b) == home {
                "HERE".to_string()
            } else {
                b.to_string()
            }
        }
        SidewalkPOI::Border(i) => {
            if OD::Border(*i) == home {
                "HERE".to_string()
            } else {
                i.to_string()
            }
        }
        x => format!("{:?}", x),
    };

    match trip {
        SpawnTrip::CarAppearing {
            depart,
            start,
            goal,
            is_bike,
        } => format!(
            "{}: {} appears at {}, goes to {}",
            depart,
            if *is_bike { "bike" } else { "car" },
            start.lane(),
            driving_goal(goal)
        ),
        SpawnTrip::MaybeUsingParkedCar(depart, start_bldg, goal) => format!(
            "{}: try to drive from {} to {}",
            depart,
            if OD::Bldg(*start_bldg) == home {
                "HERE".to_string()
            } else {
                start_bldg.to_string()
            },
            driving_goal(goal),
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

fn other_endpt(trip: &SpawnTrip, home: OD) -> ID {
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
        SpawnTrip::CarAppearing { start, goal, .. } => (ID::Lane(start.lane()), driving_goal(goal)),
        SpawnTrip::MaybeUsingParkedCar(_, start_bldg, goal) => {
            (ID::Building(*start_bldg), driving_goal(goal))
        }
        SpawnTrip::UsingBike(_, start, goal) => (sidewalk_spot(start), driving_goal(goal)),
        SpawnTrip::JustWalking(_, start, goal) => (sidewalk_spot(start), sidewalk_spot(goal)),
        SpawnTrip::UsingTransit(_, start, goal, _, _, _) => {
            (sidewalk_spot(start), sidewalk_spot(goal))
        }
    };
    let home_id = match home {
        OD::Bldg(b) => ID::Building(b),
        OD::Border(i) => ID::Intersection(i),
    };
    if from == home_id {
        to
    } else if to == home_id {
        from
    } else {
        panic!("other_endpt broke when homed at {:?} for {:?}", home, trip)
    }
}

struct CarCount {
    naive: usize,
    recycle: usize,

    // Intermediate state
    available: usize,
}

impl CarCount {
    fn new() -> CarCount {
        CarCount {
            naive: 0,
            recycle: 0,
            available: 0,
        }
    }
}

impl fmt::Display for CarCount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} / {}",
            prettyprint_usize(self.naive),
            prettyprint_usize(self.recycle),
        )
    }
}
