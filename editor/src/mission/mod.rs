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
use ezgui::{hotkey, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Wizard};
use geom::{Distance, Duration, Pt2D, PolyLine, Polygon};
use map_model::{BuildingID, Intersection, IntersectionID, LaneType, Map, PathRequest, Position};
use sim::{DrivingGoal, SidewalkSpot};
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
                    (hotkey(Key::Escape), "quit"),
                    (hotkey(Key::D), "visualize population data"),
                    (hotkey(Key::T), "visualize individual PSRC trips"),
                    (hotkey(Key::A), "visualize all PSRC trips"),
                    (hotkey(Key::S), "set up simulation with PSRC trips"),
                    (hotkey(Key::N), "manage neighborhoods"),
                    (hotkey(Key::W), "manage scenarios"),
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
                        if let Some(evmode) = viz.event(ctx, &mut state.ui) {
                            return evmode;
                        } else {
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

#[derive(Debug)]
pub enum TripEndpt {
    Building(BuildingID),
    // The Pt2D is the original point. It'll be outside the map and likely out-of-bounds entirely,
    // maybe even negative.
    Border(IntersectionID, Pt2D),
}

impl TripEndpt {
    fn new(
        endpt: &popdat::psrc::Endpoint,
        osm_id_to_bldg: &HashMap<i64, BuildingID>,
        map: &Map,
        borders: &Vec<&Intersection>,
    ) -> Option<TripEndpt> {
        if let Some(b) = endpt.osm_building.and_then(|id| osm_id_to_bldg.get(&id)) {
            return Some(TripEndpt::Building(*b));
        }
        let bounds = map.get_gps_bounds();
        borders
            .iter()
            .min_by_key(|i| {
                i.point
                    .to_gps(bounds)
                    .unwrap()
                    .gps_dist_meters(endpt.pos)
            })
            .map(|i| TripEndpt::Border(i.id, Pt2D::forcibly_from_gps(endpt.pos, bounds)))
    }

    fn start_pos_walking(&self, map: &Map) -> Position {
        match self {
            TripEndpt::Building(b) => Position::bldg_via_walking(*b, map),
            TripEndpt::Border(i, _) => {
                let lane = map.get_i(*i).get_outgoing_lanes(map, LaneType::Sidewalk)[0];
                Position::new(lane, Distance::ZERO)
            }
        }
    }

    fn end_pos_walking(&self, map: &Map) -> Position {
        match self {
            TripEndpt::Building(b) => Position::bldg_via_walking(*b, map),
            TripEndpt::Border(i, _) => {
                let lane = map.get_i(*i).get_incoming_lanes(map, LaneType::Sidewalk)[0];
                Position::new(lane, map.get_l(lane).length())
            }
        }
    }

    fn start_sidewalk_spot(&self, map: &Map) -> SidewalkSpot {
        match self {
            TripEndpt::Building(b) => SidewalkSpot::building(*b, map),
            TripEndpt::Border(i, _) => SidewalkSpot::start_at_border(*i, map).unwrap(),
        }
    }

    fn end_sidewalk_spot(&self, map: &Map) -> SidewalkSpot {
        match self {
            TripEndpt::Building(b) => SidewalkSpot::building(*b, map),
            TripEndpt::Border(i, _) => SidewalkSpot::end_at_border(*i, map).unwrap(),
        }
    }

    // TODO or biking
    // TODO bldg_via_driving needs to do find_driving_lane_near_building sometimes
    fn start_pos_driving(&self, map: &Map) -> Position {
        match self {
            TripEndpt::Building(b) => Position::bldg_via_driving(*b, map).unwrap(),
            TripEndpt::Border(i, _) => {
                let lane = map.get_i(*i).get_outgoing_lanes(map, LaneType::Driving)[0];
                Position::new(lane, Distance::ZERO)
            }
        }
    }

    fn end_pos_driving(&self, map: &Map) -> Position {
        match self {
            TripEndpt::Building(b) => Position::bldg_via_driving(*b, map).unwrap(),
            TripEndpt::Border(i, _) => {
                let lane = map.get_i(*i).get_outgoing_lanes(map, LaneType::Driving)[0];
                Position::new(lane, map.get_l(lane).length())
            }
        }
    }

    fn driving_goal(&self, lane_types: Vec<LaneType>, map: &Map) -> DrivingGoal {
        match self {
            TripEndpt::Building(b) => DrivingGoal::ParkNear(*b),
            TripEndpt::Border(i, _) => DrivingGoal::end_at_border(*i, lane_types, map).unwrap(),
        }
    }

    pub fn polygon<'a>(&self, map: &'a Map) -> &'a Polygon {
        match self {
            TripEndpt::Building(b) => &map.get_b(*b).polygon,
            TripEndpt::Border(i, _) => &map.get_i(*i).polygon,
        }
    }
}

#[derive(Debug)]
pub struct Trip {
    pub from: TripEndpt,
    pub to: TripEndpt,
    pub depart_at: Duration,
    pub purpose: (popdat::psrc::Purpose, popdat::psrc::Purpose),
    pub mode: popdat::psrc::Mode,
    // These are an upper bound when TripEndpt::Border is involved.
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
                start: self.from.start_pos_walking(map),
                end: self.from.end_pos_walking(map),
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
            Mode::Bike => PathRequest {
                start: self.from.start_pos_driving(map),
                end: self.to.end_pos_driving(map),
                can_use_bike_lanes: true,
                can_use_bus_lanes: false,
            },
            Mode::Drive => PathRequest {
                start: self.from.start_pos_driving(map),
                end: self.to.end_pos_driving(map),
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            },
            Mode::Transit => {
                let start = self.from.start_pos_walking(map);
                let end = self.to.end_pos_walking(map);
                if let Some((stop1, _, _)) = map.should_use_transit(start, end) {
                    PathRequest {
                        start,
                        end: SidewalkSpot::bus_stop(stop1, map).sidewalk_pos,
                        can_use_bike_lanes: false,
                        can_use_bus_lanes: false,
                    }
                } else {
                    // Just fall back to walking. :\
                    PathRequest {
                        start,
                        end,
                        can_use_bike_lanes: false,
                        can_use_bus_lanes: false,
                    }
                }
            }
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
    let map = &ui.primary.map;

    let mut osm_id_to_bldg = HashMap::new();
    for b in map.all_buildings() {
        osm_id_to_bldg.insert(b.osm_way_id, b.id);
    }
    let incoming_borders = map.all_incoming_borders();
    let outgoing_borders = map.all_outgoing_borders();

    let mut results = Vec::new();
    timer.start_iter("clip trips", popdat.trips.len());
    for trip in &popdat.trips {
        timer.next();
        if results.len() == max_results {
            continue;
        }

        let from = skip_fail!(TripEndpt::new(
            &trip.from,
            &osm_id_to_bldg,
            map,
            &incoming_borders,
        ));
        let to = skip_fail!(TripEndpt::new(
            &trip.to,
            &osm_id_to_bldg,
            map,
            &outgoing_borders,
        ));
        // TODO Detect pass-through trips
        match (&from, &to) {
            (TripEndpt::Border(_, _), TripEndpt::Border(_, _)) => {
                continue;
            }
            _ => {}
        }

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
    use sim::{Scenario, TripSpec};

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
                            trip.from.start_pos_driving(map),
                            map,
                        ) {
                            TripSpec::CarAppearing {
                                start_pos,
                                goal: trip.to.driving_goal(vec![LaneType::Driving], map),
                                ped_speed: Scenario::rand_ped_speed(&mut rng),
                                vehicle_spec: Scenario::rand_car(&mut rng),
                            }
                        } else {
                            timer.warn(format!("Can't make car appear at {:?}", trip.from));
                            continue;
                        }
                    }
                    Mode::Bike => match trip.from {
                        TripEndpt::Building(b) => TripSpec::UsingBike {
                            start: SidewalkSpot::building(b, map),
                            goal: trip.to.driving_goal(vec![LaneType::Biking, LaneType::Driving], map),
                            ped_speed: Scenario::rand_ped_speed(&mut rng),
                            vehicle: Scenario::rand_bike(&mut rng),
                        },
                        TripEndpt::Border(i, _) => {
                            let vehicle = Scenario::rand_bike(&mut rng);
                            let l = map.get_i(i).get_outgoing_lanes(
                                map,
                                // TODO Or Biking
                                LaneType::Driving)[0];
                            TripSpec::CarAppearing {
                                start_pos: Position::new(l, vehicle.length),
                                goal: trip.to.driving_goal(vec![LaneType::Biking, LaneType::Driving], map),
                                ped_speed: Scenario::rand_ped_speed(&mut rng),
                                vehicle_spec: vehicle,
                            }
                        },
                    },
                    Mode::Walk => TripSpec::JustWalking {
                        start: trip.from.start_sidewalk_spot(map),
                        goal: trip.to.end_sidewalk_spot(map),
                        ped_speed: Scenario::rand_ped_speed(&mut rng),
                    },
                    Mode::Transit => {
                        let start = trip.from.start_sidewalk_spot(map);
                        let goal = trip.to.end_sidewalk_spot(map);
                        let ped_speed = Scenario::rand_ped_speed(&mut rng);
                        if let Some((stop1, stop2, route)) = map.should_use_transit(start.sidewalk_pos, goal.sidewalk_pos) {
                            TripSpec::UsingTransit {
                                start, goal, route, stop1, stop2, ped_speed,
                            }
                        } else {
                            timer.warn(format!("{:?} not actually using transit, because pathfinding didn't find any useful route", trip));
                            TripSpec::JustWalking {
                                start, goal, ped_speed }
                        }
                    }
                },
                map,
            );
            min_time = min_time.min(trip.depart_at);
        }
        timer.note(format!("Expect the first trip to start at {}", min_time));

        for route in map.get_all_bus_routes() {
            ui.primary.sim.seed_bus_route(route, map, &mut timer);
        }

        ui.primary.sim.spawn_all_trips(map, &mut timer, true);
        ui.primary.sim.step(map, Duration::const_seconds(0.1));
    });
}
