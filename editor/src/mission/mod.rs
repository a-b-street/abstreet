mod all_trips;
mod dataviz;
mod individ_trips;
mod neighborhood;
mod scenario;

use crate::game::{GameState, Mode};
use crate::render::DrawOptions;
use crate::sandbox::SandboxMode;
use crate::ui::ShowEverything;
use crate::ui::UI;
use abstutil::{skip_fail, Timer};
use ezgui::{EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Wizard};
use geom::{Distance, Duration, PolyLine};
use map_model::{BuildingID, Map, PathRequest, Position};
use std::collections::HashMap;

pub struct MissionEditMode {
    state: State,
}

enum State {
    Exploring(ModalMenu),
    Neighborhood(neighborhood::NeighborhoodEditor),
    Scenario(scenario::ScenarioEditor),
    DataViz(dataviz::DataVisualizer),
    IndividualTrips(individ_trips::TripsVisualizer),
    AllTrips(all_trips::TripsVisualizer),
}

impl MissionEditMode {
    pub fn new(ctx: &EventCtx, ui: &mut UI) -> MissionEditMode {
        // TODO Warn first?
        ui.primary.reset_sim();

        MissionEditMode {
            state: State::Exploring(ModalMenu::new(
                "Mission Edit Mode",
                vec![
                    (Some(Key::Escape), "quit"),
                    (Some(Key::D), "visualize population data"),
                    (Some(Key::T), "visualize individual PSRC trips"),
                    (Some(Key::A), "visualize all PSRC trips"),
                    (Some(Key::S), "set up simulation with PSRC trips"),
                    (Some(Key::N), "manage neighborhoods"),
                    (Some(Key::W), "manage scenarios"),
                ],
                ctx,
            )),
        }
    }

    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        match state.mode {
            Mode::Mission(ref mut mode) => {
                match mode.state {
                    State::Exploring(ref mut menu) => {
                        menu.handle_event(ctx, None);
                        ctx.canvas.handle_event(ctx.input);

                        if menu.action("quit") {
                            state.mode = Mode::SplashScreen(Wizard::new(), None);
                        } else if menu.action("visualize population data") {
                            mode.state =
                                State::DataViz(dataviz::DataVisualizer::new(ctx, &state.ui));
                        } else if menu.action("visualize individual PSRC trips") {
                            mode.state = State::IndividualTrips(
                                individ_trips::TripsVisualizer::new(ctx, &state.ui),
                            );
                        } else if menu.action("visualize all PSRC trips") {
                            mode.state =
                                State::AllTrips(all_trips::TripsVisualizer::new(ctx, &state.ui));
                        } else if menu.action("set up simulation with PSRC trips") {
                            instantiate_trips(ctx, &mut state.ui);
                            state.mode = Mode::Sandbox(SandboxMode::new(ctx));
                        } else if menu.action("manage neighborhoods") {
                            mode.state = State::Neighborhood(
                                neighborhood::NeighborhoodEditor::PickNeighborhood(Wizard::new()),
                            );
                        } else if menu.action("manage scenarios") {
                            mode.state = State::Scenario(scenario::ScenarioEditor::PickScenario(
                                Wizard::new(),
                            ));
                        }
                    }
                    State::DataViz(ref mut viz) => {
                        if viz.event(ctx, &state.ui) {
                            mode.state = MissionEditMode::new(ctx, &mut state.ui).state;
                        }
                    }
                    State::IndividualTrips(ref mut viz) => {
                        if viz.event(ctx, &mut state.ui) {
                            mode.state = MissionEditMode::new(ctx, &mut state.ui).state;
                        }
                    }
                    State::AllTrips(ref mut viz) => {
                        if viz.event(ctx, &mut state.ui) {
                            mode.state = MissionEditMode::new(ctx, &mut state.ui).state;
                        }
                    }
                    State::Neighborhood(ref mut editor) => {
                        if editor.event(ctx, &state.ui) {
                            mode.state = MissionEditMode::new(ctx, &mut state.ui).state;
                        }
                    }
                    State::Scenario(ref mut editor) => {
                        if let Some(new_mode) = editor.event(ctx, &mut state.ui) {
                            state.mode = new_mode;
                        }
                    }
                }
                EventLoopMode::InputOnly
            }
            _ => unreachable!(),
        }
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        state.ui.draw(
            g,
            DrawOptions::new(),
            &state.ui.primary.sim,
            &ShowEverything::new(),
        );

        match state.mode {
            Mode::Mission(ref mode) => match mode.state {
                State::Exploring(ref menu) => {
                    menu.draw(g);
                }
                State::DataViz(ref viz) => {
                    viz.draw(g, &state.ui);
                }
                State::IndividualTrips(ref viz) => {
                    viz.draw(g, &state.ui);
                }
                State::AllTrips(ref viz) => {
                    viz.draw(g, &state.ui);
                }
                State::Neighborhood(ref editor) => {
                    editor.draw(g, &state.ui);
                }
                State::Scenario(ref editor) => {
                    editor.draw(g, &state.ui);
                }
            },
            _ => unreachable!(),
        }
    }
}

pub struct Trip {
    pub from: BuildingID,
    pub to: BuildingID,
    pub depart_at: Duration,
    pub purpose: (popdat::psrc::Purpose, popdat::psrc::Purpose),
    pub mode: popdat::psrc::Mode,
    pub trip_time: Duration,
    pub trip_dist: Distance,
    // clip_trips doesn't populate this.
    pub route: Option<PolyLine>,
}

impl Trip {
    pub fn end_time(&self) -> Duration {
        self.depart_at + self.trip_time
    }

    pub fn path_req(&self, map: &Map) -> PathRequest {
        use popdat::psrc::Mode;

        match self.mode {
            Mode::Walk => PathRequest {
                start: Position::bldg_via_walking(self.from, map),
                end: Position::bldg_via_walking(self.to, map),
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
            // TODO bldg_via_driving needs to do find_driving_lane_near_building sometimes,
            // refactor that
            Mode::Bike => PathRequest {
                // TODO Allow bike lane start/stops too
                start: Position::bldg_via_driving(self.from, map).unwrap(),
                end: Position::bldg_via_driving(self.to, map).unwrap(),
                can_use_bike_lanes: true,
                can_use_bus_lanes: false,
            },
            Mode::Drive => PathRequest {
                start: Position::bldg_via_driving(self.from, map).unwrap(),
                end: Position::bldg_via_driving(self.to, map).unwrap(),
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
        }
    }
}

// TODO max_results just temporary for development.
pub fn clip_trips(
    popdat: &popdat::PopDat,
    ui: &UI,
    max_results: usize,
    timer: &mut Timer,
) -> Vec<Trip> {
    let mut osm_id_to_bldg = HashMap::new();
    for b in ui.primary.map.all_buildings() {
        osm_id_to_bldg.insert(b.osm_way_id, b.id);
    }

    let mut results = Vec::new();
    timer.start_iter("clip trips", popdat.trips.len());
    for trip in &popdat.trips {
        timer.next();
        if results.len() == max_results {
            continue;
        }

        let from = *skip_fail!(osm_id_to_bldg.get(&trip.from));
        let to = *skip_fail!(osm_id_to_bldg.get(&trip.to));
        results.push(Trip {
            from,
            to,
            depart_at: trip.depart_at,
            purpose: trip.purpose,
            mode: trip.mode,
            trip_time: trip.trip_time,
            trip_dist: trip.trip_dist,
            route: None,
        });
    }
    results
}

fn instantiate_trips(ctx: &mut EventCtx, ui: &mut UI) {
    use popdat::psrc::Mode;
    use sim::{DrivingGoal, Scenario, SidewalkSpot, TripSpec};

    ctx.loading_screen("set up sim with PSRC trips", |_, mut timer| {
        let popdat: popdat::PopDat = abstutil::read_binary("../data/shapes/popdat", &mut timer)
            .expect("Couldn't load popdat");
        let map = &ui.primary.map;
        let mut rng = ui.primary.current_flags.sim_flags.make_rng();

        let mut min_time = Duration::parse("23:59:59.9").unwrap();

        for trip in clip_trips(&popdat, ui, 10_000, &mut timer) {
            ui.primary.sim.schedule_trip(
                trip.depart_at,
                match trip.mode {
                    // TODO Use a parked car, but first have to figure out what cars to seed.
                    Mode::Drive => {
                        if let Some(start_pos) = TripSpec::spawn_car_at(
                            Position::bldg_via_driving(trip.from, map).unwrap(),
                            map,
                        ) {
                            TripSpec::CarAppearing {
                                start_pos,
                                goal: DrivingGoal::ParkNear(trip.to),
                                ped_speed: Scenario::rand_ped_speed(&mut rng),
                                vehicle_spec: Scenario::rand_car(&mut rng),
                            }
                        } else {
                            timer.warn(format!("Can't make car appear at {}", trip.from));
                            continue;
                        }
                    }
                    Mode::Bike => TripSpec::UsingBike {
                        start: SidewalkSpot::building(trip.from, map),
                        goal: DrivingGoal::ParkNear(trip.to),
                        ped_speed: Scenario::rand_ped_speed(&mut rng),
                        vehicle: Scenario::rand_bike(&mut rng),
                    },
                    Mode::Walk => TripSpec::JustWalking {
                        start: SidewalkSpot::building(trip.from, map),
                        goal: SidewalkSpot::building(trip.to, map),
                        ped_speed: Scenario::rand_ped_speed(&mut rng),
                    },
                },
                map,
            );
            min_time = min_time.min(trip.depart_at);
        }
        timer.note(format!("Expect the first trip to start at {}", min_time));

        ui.primary.sim.spawn_all_trips(map, &mut timer, true);
    });
}
