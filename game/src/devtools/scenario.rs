use crate::app::App;
use crate::common::{tool_panel, Colorer, CommonState, ContextualActions, Warping};
use crate::devtools::blocks::BlockMap;
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::managed::{WrappedComposite, WrappedOutcome};
use abstutil::{prettyprint_usize, Counter, MultiMap};
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Slider, VerticalAlignment, Widget,
};
use geom::{Distance, Line, PolyLine, Polygon};
use map_model::{BuildingID, IntersectionID, Map};
use sim::{
    DrivingGoal, IndividTrip, PersonSpec, Scenario, SidewalkPOI, SidewalkSpot, SpawnTrip,
    TripEndpoint,
};
use std::collections::BTreeSet;

pub struct ScenarioManager {
    composite: Composite,
    common: CommonState,
    tool_panel: WrappedComposite,
    scenario: Scenario,

    // The (person, trip) usizes are indices into scenario.people[x].trips[y]
    trips_from_bldg: MultiMap<BuildingID, (usize, usize)>,
    trips_to_bldg: MultiMap<BuildingID, (usize, usize)>,
    trips_from_border: MultiMap<IntersectionID, (usize, usize)>,
    trips_to_border: MultiMap<IntersectionID, (usize, usize)>,
    bldg_colors: Colorer,

    demand: Option<Drawable>,
}

impl ScenarioManager {
    pub fn new(scenario: Scenario, ctx: &mut EventCtx, app: &App) -> ScenarioManager {
        let mut trips_from_bldg = MultiMap::new();
        let mut trips_to_bldg = MultiMap::new();
        let mut trips_from_border = MultiMap::new();
        let mut trips_to_border = MultiMap::new();
        let mut num_trips = 0;
        for (idx1, person) in scenario.people.iter().enumerate() {
            for (idx2, trip) in person.trips.iter().enumerate() {
                num_trips += 1;
                let idx = (idx1, idx2);
                match trip.trip.start(&app.primary.map) {
                    TripEndpoint::Bldg(b) => {
                        trips_from_bldg.insert(b, idx);
                    }
                    TripEndpoint::Border(i) => {
                        trips_from_border.insert(i, idx);
                    }
                }
                match trip.trip.end() {
                    TripEndpoint::Bldg(b) => {
                        trips_to_bldg.insert(b, idx);
                    }
                    TripEndpoint::Border(i) => {
                        trips_to_border.insert(i, idx);
                    }
                }
            }
        }

        let mut bldg_colors = Colorer::scaled(
            ctx,
            "Parked cars per building",
            Vec::new(),
            vec![Color::BLUE, Color::RED, Color::BLACK],
            vec!["0", "1-2", "3-4", "..."],
        );
        let mut total_cars_needed = 0;
        for (b, count) in scenario.count_parked_cars_per_bldg().consume() {
            total_cars_needed += count;
            let color = if count == 0 {
                continue;
            } else if count == 1 || count == 2 {
                Color::BLUE
            } else if count == 3 || count == 4 {
                Color::RED
            } else {
                Color::BLACK
            };
            bldg_colors.add_b(b, color);
        }

        let (filled_spots, free_parking_spots) = app.primary.sim.get_all_parking_spots();
        assert!(filled_spots.is_empty());

        ScenarioManager {
            composite: WrappedComposite::quick_menu(
                ctx,
                app,
                format!("Scenario {}", scenario.scenario_name),
                vec![
                    format!("{} total trips", prettyprint_usize(num_trips),),
                    format!("{} people", prettyprint_usize(scenario.people.len())),
                    format!("seed {} parked cars", prettyprint_usize(total_cars_needed)),
                    format!(
                        "{} parking spots",
                        prettyprint_usize(free_parking_spots.len()),
                    ),
                ],
                vec![
                    (hotkey(Key::B), "block map"),
                    (hotkey(Key::D), "dot map"),
                    (lctrl(Key::P), "stop showing paths"),
                ],
            ),
            common: CommonState::new(),
            tool_panel: tool_panel(ctx, app),
            scenario,
            trips_from_bldg,
            trips_to_bldg,
            trips_from_border,
            trips_to_border,
            bldg_colors: bldg_colors.build_both(ctx, app),
            demand: None,
        }
    }
}

impl State for ScenarioManager {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "dot map" => {
                    return Transition::Push(Box::new(DotMap::new(ctx, app, &self.scenario)));
                }
                "block map" => {
                    return Transition::Push(Box::new(BlockMap::new(
                        ctx,
                        app,
                        self.scenario.clone(),
                    )));
                }
                // TODO Inactivate this sometimes
                "stop showing paths" => {
                    self.demand = None;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        if let Some(t) = self.common.event(
            ctx,
            app,
            &mut Actions {
                demand: &mut self.demand,
                scenario: &self.scenario,
                trips_from_bldg: &self.trips_from_bldg,
                trips_to_bldg: &self.trips_to_bldg,
                trips_from_border: &self.trips_from_border,
                trips_to_border: &self.trips_to_border,
            },
        ) {
            return t;
        }
        match self.tool_panel.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => t,
            Some(WrappedOutcome::Clicked(x)) => match x.as_ref() {
                "back" => Transition::Pop,
                _ => unreachable!(),
            },
            None => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // TODO Let common contribute draw_options...
        self.bldg_colors.draw(g);
        if let Some(ref p) = self.demand {
            g.redraw(p);
        }

        self.composite.draw(g);
        self.common.draw(g, app);
        self.tool_panel.draw(g);
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
    indices: BTreeSet<(usize, usize)>,
    noun: &'static str,
    home: OD,
) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, app| {
        let mut people = BTreeSet::new();
        for (idx1, _) in &indices {
            people.insert(scenario.people[*idx1].id);
        }

        let warp_to = wiz
            .wrap(ctx)
            .choose(
                &format!("Trips from/to this {}, by {} people", noun, people.len()),
                || {
                    // TODO Panics if there are two duplicate trips (b1124 in montlake)
                    indices
                        .iter()
                        .map(|(idx1, idx2)| {
                            let person = &scenario.people[*idx1];
                            let trip = &person.trips[*idx2];
                            Choice::new(
                                describe(person, trip, home),
                                other_endpt(trip, home, &app.primary.map),
                            )
                        })
                        .collect()
                },
            )?
            .1;
        Some(Transition::Replace(Warping::new(
            ctx,
            warp_to.canonical_point(&app.primary).unwrap(),
            None,
            Some(warp_to),
            &mut app.primary,
        )))
    }))
}

fn describe(person: &PersonSpec, trip: &IndividTrip, home: OD) -> String {
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

    match &trip.trip {
        SpawnTrip::VehicleAppearing {
            start,
            goal,
            is_bike,
        } => format!(
            "{} at {}: {} appears at {}, goes to {}",
            person.id,
            trip.depart,
            if *is_bike { "bike" } else { "car" },
            start.lane(),
            driving_goal(goal)
        ),
        SpawnTrip::FromBorder { i, goal, is_bike } => format!(
            "{} at {}: {} appears at {}, goes to {}",
            person.id,
            trip.depart,
            if *is_bike { "bike" } else { "car" },
            i,
            driving_goal(goal)
        ),
        SpawnTrip::UsingParkedCar(start_bldg, goal) => format!(
            "{} at {}: drive from {} to {}",
            person.id,
            trip.depart,
            if OD::Bldg(*start_bldg) == home {
                "HERE".to_string()
            } else {
                start_bldg.to_string()
            },
            driving_goal(goal),
        ),
        SpawnTrip::UsingBike(start, goal) => format!(
            "{} at {}: bike from {} to {}",
            person.id,
            trip.depart,
            sidewalk_spot(start),
            driving_goal(goal)
        ),
        SpawnTrip::JustWalking(start, goal) => format!(
            "{} at {}: walk from {} to {}",
            person.id,
            trip.depart,
            sidewalk_spot(start),
            sidewalk_spot(goal)
        ),
        SpawnTrip::UsingTransit(start, goal, route, _, _) => format!(
            "{} at {}: bus from {} to {} using {}",
            person.id,
            trip.depart,
            sidewalk_spot(start),
            sidewalk_spot(goal),
            route
        ),
    }
}

fn other_endpt(trip: &IndividTrip, home: OD, map: &Map) -> ID {
    let driving_goal = |goal: &DrivingGoal| match goal {
        DrivingGoal::ParkNear(b) => ID::Building(*b),
        DrivingGoal::Border(i, _) => ID::Intersection(*i),
    };
    let sidewalk_spot = |spot: &SidewalkSpot| match &spot.connection {
        SidewalkPOI::Building(b) => ID::Building(*b),
        SidewalkPOI::Border(i) => ID::Intersection(*i),
        x => panic!("other_endpt for {:?}?", x),
    };

    let (from, to) = match &trip.trip {
        SpawnTrip::VehicleAppearing { start, goal, .. } => (
            ID::Intersection(map.get_l(start.lane()).src_i),
            driving_goal(goal),
        ),
        SpawnTrip::FromBorder { i, goal, .. } => (ID::Intersection(*i), driving_goal(goal)),
        SpawnTrip::UsingParkedCar(start_bldg, goal) => {
            (ID::Building(*start_bldg), driving_goal(goal))
        }
        SpawnTrip::UsingBike(start, goal) => (sidewalk_spot(start), driving_goal(goal)),
        SpawnTrip::JustWalking(start, goal) => (sidewalk_spot(start), sidewalk_spot(goal)),
        SpawnTrip::UsingTransit(start, goal, _, _, _) => {
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

// TODO Understand demand better.
// - Be able to select an area, see trips to/from it
// - Weight the arrow size by how many trips go there
// - Legend, counting the number of trips
fn show_demand(
    scenario: &Scenario,
    from: &BTreeSet<(usize, usize)>,
    to: &BTreeSet<(usize, usize)>,
    home: OD,
    app: &App,
    ctx: &EventCtx,
) -> Drawable {
    let mut from_ids = Counter::new();
    for (idx1, idx2) in from {
        from_ids.inc(other_endpt(
            &scenario.people[*idx1].trips[*idx2],
            home,
            &app.primary.map,
        ));
    }
    let mut to_ids = Counter::new();
    for (idx1, idx2) in to {
        to_ids.inc(other_endpt(
            &scenario.people[*idx1].trips[*idx2],
            home,
            &app.primary.map,
        ));
    }
    let from_count = from_ids.consume();
    let mut to_count = to_ids.consume();
    let max_count =
        (*from_count.values().max().unwrap()).max(*to_count.values().max().unwrap()) as f64;

    let mut batch = GeomBatch::new();
    let home_pt = match home {
        OD::Bldg(b) => app.primary.map.get_b(b).polygon.center(),
        OD::Border(i) => app.primary.map.get_i(i).polygon.center(),
    };

    for (id, cnt) in from_count {
        // Bidirectional?
        if let Some(other_cnt) = to_count.remove(&id) {
            let width = Distance::meters(1.0)
                + ((cnt.max(other_cnt) as f64) / max_count) * Distance::meters(2.0);
            batch.push(
                Color::PURPLE.alpha(0.8),
                PolyLine::new(vec![home_pt, id.canonical_point(&app.primary).unwrap()])
                    .make_polygons(width),
            );
        } else {
            let width = Distance::meters(1.0) + ((cnt as f64) / max_count) * Distance::meters(2.0);
            batch.push(
                Color::RED.alpha(0.8),
                PolyLine::new(vec![home_pt, id.canonical_point(&app.primary).unwrap()])
                    .make_arrow(width)
                    .unwrap(),
            );
        }
    }
    for (id, cnt) in to_count {
        let width = Distance::meters(1.0) + ((cnt as f64) / max_count) * Distance::meters(2.0);
        batch.push(
            Color::BLUE.alpha(0.8),
            PolyLine::new(vec![id.canonical_point(&app.primary).unwrap(), home_pt])
                .make_arrow(width)
                .unwrap(),
        );
    }

    batch.upload(ctx)
}

struct DotMap {
    composite: Composite,

    lines: Vec<Line>,
    draw: Option<(f64, Drawable)>,
}

impl DotMap {
    fn new(ctx: &mut EventCtx, app: &App, scenario: &Scenario) -> DotMap {
        let map = &app.primary.map;
        let lines = scenario
            .people
            .iter()
            .flat_map(|p| {
                p.trips.iter().filter_map(|trip| {
                    let (start, end) = match &trip.trip {
                        SpawnTrip::VehicleAppearing { start, goal, .. } => {
                            (start.pt(map), goal.pt(map))
                        }
                        SpawnTrip::FromBorder { i, goal, .. } => {
                            (map.get_i(*i).polygon.center(), goal.pt(map))
                        }
                        SpawnTrip::UsingParkedCar(b, goal) => {
                            (map.get_b(*b).polygon.center(), goal.pt(map))
                        }
                        SpawnTrip::UsingBike(start, goal) => {
                            (start.sidewalk_pos.pt(map), goal.pt(map))
                        }
                        SpawnTrip::JustWalking(start, goal) => {
                            (start.sidewalk_pos.pt(map), goal.sidewalk_pos.pt(map))
                        }
                        SpawnTrip::UsingTransit(start, goal, _, _, _) => {
                            (start.sidewalk_pos.pt(map), goal.sidewalk_pos.pt(map))
                        }
                    };
                    Line::maybe_new(start, end)
                })
            })
            .collect();
        DotMap {
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Dot map of all trips").small_heading().draw(ctx),
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Slider::horizontal(ctx, 150.0, 25.0, 0.0).named("time slider"),
                ])
                .padding(10)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),

            lines,
            draw: None,
        }
    }
}

impl State for DotMap {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        let pct = self.composite.slider("time slider").get_percent();

        if self.draw.as_ref().map(|(p, _)| pct != *p).unwrap_or(true) {
            let mut batch = GeomBatch::new();
            let radius = Distance::meters(5.0);
            for l in &self.lines {
                // Circles are too expensive. :P
                batch.push(
                    Color::RED,
                    Polygon::rectangle_centered(l.percent_along(pct), radius, radius),
                );
            }
            self.draw = Some((pct, batch.upload(ctx)));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        if let Some((_, ref d)) = self.draw {
            g.redraw(d);
        }
        self.composite.draw(g);
    }
}

struct Actions<'a> {
    demand: &'a mut Option<Drawable>,
    scenario: &'a Scenario,
    trips_from_bldg: &'a MultiMap<BuildingID, (usize, usize)>,
    trips_to_bldg: &'a MultiMap<BuildingID, (usize, usize)>,
    trips_from_border: &'a MultiMap<IntersectionID, (usize, usize)>,
    trips_to_border: &'a MultiMap<IntersectionID, (usize, usize)>,
}
impl<'a> ContextualActions for Actions<'a> {
    fn actions(&self, _: &App, id: ID) -> Vec<(Key, String)> {
        let mut actions = Vec::new();

        if let ID::Building(b) = id {
            let from = self.trips_from_bldg.get(b);
            let to = self.trips_to_bldg.get(b);
            if !from.is_empty() || !to.is_empty() {
                actions.push((Key::T, "browse trips".to_string()));
                if self.demand.is_none() {
                    actions.push((Key::P, "show trips to and from".to_string()));
                }
            }
        } else if let ID::Intersection(i) = id {
            let from = self.trips_from_border.get(i);
            let to = self.trips_to_border.get(i);
            if !from.is_empty() || !to.is_empty() {
                actions.push((Key::T, "browse trips".to_string()));
                if self.demand.is_none() {
                    actions.push((Key::P, "show trips to and from".to_string()));
                }
            }
        }

        actions
    }
    fn execute(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        id: ID,
        action: String,
        _: &mut bool,
    ) -> Transition {
        match (id, action.as_ref()) {
            (ID::Building(b), "browse trips") => {
                // TODO Avoid the clone? Just happens once though.
                let mut all_trips = self.trips_from_bldg.get(b).clone();
                all_trips.extend(self.trips_to_bldg.get(b).clone());
                Transition::Push(make_trip_picker(
                    self.scenario.clone(),
                    all_trips,
                    "building",
                    OD::Bldg(b),
                ))
            }
            (ID::Building(b), "show trips to and from") => {
                *self.demand = Some(show_demand(
                    self.scenario,
                    self.trips_from_bldg.get(b),
                    self.trips_to_bldg.get(b),
                    OD::Bldg(b),
                    app,
                    ctx,
                ));
                Transition::Keep
            }
            (ID::Intersection(i), "browse trips") => {
                // TODO Avoid the clone? Just happens once though.
                let mut all_trips = self.trips_from_border.get(i).clone();
                all_trips.extend(self.trips_to_border.get(i).clone());
                Transition::Push(make_trip_picker(
                    self.scenario.clone(),
                    all_trips,
                    "border",
                    OD::Border(i),
                ))
            }
            (ID::Intersection(i), "show trips to and from") => {
                *self.demand = Some(show_demand(
                    self.scenario,
                    self.trips_from_border.get(i),
                    self.trips_to_border.get(i),
                    OD::Border(i),
                    app,
                    ctx,
                ));
                Transition::Keep
            }
            _ => unreachable!(),
        }
    }

    fn is_paused(&self) -> bool {
        true
    }
}
