use crate::app::App;
use crate::common::{Colorer, CommonState};
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::sandbox::gameplay::freeform::Freeform;
use crate::sandbox::SandboxMode;
use abstutil::Timer;
use ezgui::{
    hotkey, Btn, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, TextExt,
    VerticalAlignment, Widget,
};
use geom::{Distance, Duration, PolyLine};
use map_model::{
    IntersectionID, LaneID, PathConstraints, PathRequest, Position, NORMAL_LANE_THICKNESS,
};
use rand::seq::SliceRandom;
use rand::Rng;
use sim::{
    BorderSpawnOverTime, DrivingGoal, OriginDestination, Scenario, ScenarioGenerator, SidewalkSpot,
    TripEndpoint, TripSpec,
};

const SMALL_DT: Duration = Duration::const_seconds(0.1);

pub fn spawn_agents_around(i: IntersectionID, app: &mut App) {
    let map = &app.primary.map;
    let sim = &mut app.primary.sim;
    let mut rng = app.primary.current_flags.sim_flags.make_rng();
    let mut spawner = sim.make_spawner();

    if map.all_buildings().is_empty() {
        println!("No buildings, can't pick destinations");
        return;
    }

    let mut timer = Timer::new(format!(
        "spawning agents around {} (rng seed {:?})",
        i, app.primary.current_flags.sim_flags.rng_seed
    ));

    let now = sim.time();
    for l in &map.get_i(i).incoming_lanes {
        let lane = map.get_l(*l);
        if lane.is_driving() || lane.is_biking() {
            for _ in 0..10 {
                let vehicle_spec = if rng.gen_bool(0.7) && lane.is_driving() {
                    Scenario::rand_car(&mut rng)
                } else {
                    Scenario::rand_bike(&mut rng)
                };
                if vehicle_spec.length > lane.length() {
                    continue;
                }
                let person = sim.random_person(
                    Scenario::rand_ped_speed(&mut rng),
                    vec![vehicle_spec.clone()],
                );
                spawner.schedule_trip(
                    person,
                    now,
                    TripSpec::VehicleAppearing {
                        start_pos: Position::new(
                            lane.id,
                            Scenario::rand_dist(&mut rng, vehicle_spec.length, lane.length()),
                        ),
                        goal: DrivingGoal::ParkNear(
                            map.all_buildings().choose(&mut rng).unwrap().id,
                        ),
                        use_vehicle: person.vehicles[0].id,
                        retry_if_no_room: false,
                        origin: None,
                    },
                    TripEndpoint::Border(lane.src_i, None),
                    map,
                );
            }
        } else if lane.is_sidewalk() {
            for _ in 0..5 {
                spawner.schedule_trip(
                    sim.random_person(Scenario::rand_ped_speed(&mut rng), Vec::new()),
                    now,
                    TripSpec::JustWalking {
                        start: SidewalkSpot::suddenly_appear(
                            lane.id,
                            Scenario::rand_dist(&mut rng, 0.1 * lane.length(), 0.9 * lane.length()),
                            map,
                        ),
                        goal: SidewalkSpot::building(
                            map.all_buildings().choose(&mut rng).unwrap().id,
                            map,
                        ),
                    },
                    TripEndpoint::Border(lane.src_i, None),
                    map,
                );
            }
        }
    }

    sim.flush_spawner(spawner, map, &mut timer);
    sim.normal_step(map, SMALL_DT);
}

// New experiment, stop squeezing in all these options into one thing, specialize.
struct SpawnManyAgents {
    composite: Composite,
    from: LaneID,
    maybe_goal: Option<(LaneID, Option<PolyLine>)>,
    schedule: Option<(usize, Duration)>,
    colorer: Colorer,
}

impl State for SpawnManyAgents {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        // TODO Weird pattern for handling "return value" from the wizard we launched? Maybe
        // PopWithData is a weird pattern; we should have a resume() handler that handles the
        // context
        if let Some((count, duration)) = self.schedule {
            let dst_l = self.maybe_goal.take().unwrap().0;
            create_swarm(app, self.from, dst_l, count, duration);
            let src = app.primary.map.get_l(self.from).src_i;
            let dst = app.primary.map.get_l(dst_l).dst_i;
            return Transition::PopWithData(Box::new(move |state, _, _| {
                let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                let freeform = sandbox.gameplay.downcast_mut::<Freeform>().unwrap();
                freeform.spawn_pts.insert(src);
                freeform.spawn_pts.insert(dst);
            }));
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        let map = &app.primary.map;

        let new_goal = match app.primary.current_selection {
            Some(ID::Lane(l)) if map.get_l(l).is_driving() => l,
            _ => {
                self.maybe_goal = None;
                return Transition::Keep;
            }
        };

        let recalculate = match self.maybe_goal {
            Some((l, _)) => l != new_goal,
            None => true,
        };

        if recalculate {
            if let Some(path) = map.pathfind(PathRequest {
                start: Position::new(self.from, Distance::ZERO),
                end: Position::new(new_goal, map.get_l(new_goal).length()),
                constraints: PathConstraints::Car,
            }) {
                self.maybe_goal = Some((new_goal, path.trace(map, Distance::ZERO, None)));
            } else {
                self.maybe_goal = None;
            }
        }

        if self.maybe_goal.is_some()
            && self.schedule.is_none()
            && app.per_obj.left_click(ctx, "end the swarm here")
        {
            return Transition::Push(WizardState::new(Box::new(move |wiz, ctx, _| {
                let mut wizard = wiz.wrap(ctx);
                let count =
                    wizard.input_usize_prefilled("How many cars to spawn?", "1000".to_string())?;
                let duration = Duration::seconds(wizard.input_usize_prefilled(
                    "How many seconds to spawn them over?",
                    "90".to_string(),
                )? as f64);
                // TODO Or call create_swarm here and pop twice. Nice to play with two patterns
                // though.
                // TODO Another alt is to extend the wizard pattern and have a sort of
                // general-purpose wizard block.
                Some(Transition::PopWithData(Box::new(move |state, _, _| {
                    let mut spawn = state.downcast_mut::<SpawnManyAgents>().unwrap();
                    spawn.schedule = Some((count, duration));
                })))
            })));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.colorer.draw(g, app);

        if let Some((_, Some(ref trace))) = self.maybe_goal {
            g.draw_polygon(app.cs.route, &trace.make_polygons(NORMAL_LANE_THICKNESS));
        }

        self.composite.draw(g);
        CommonState::draw_osd(g, app, &app.primary.current_selection);
    }
}

fn create_swarm(app: &mut App, from: LaneID, to: LaneID, count: usize, duration: Duration) {
    let mut scenario = ScenarioGenerator::empty("swarm");
    scenario.border_spawn_over_time.push(BorderSpawnOverTime {
        num_peds: 0,
        num_cars: count,
        num_bikes: 0,
        start_time: app.primary.sim.time() + SMALL_DT,
        stop_time: app.primary.sim.time() + SMALL_DT + duration,
        start_from_border: app
            .primary
            .map
            .get_l(from)
            .get_directed_parent(&app.primary.map),
        goal: OriginDestination::EndOfRoad(
            app.primary
                .map
                .get_l(to)
                .get_directed_parent(&app.primary.map),
        ),
        percent_use_transit: 0.0,
    });
    let mut rng = app.primary.current_flags.sim_flags.make_rng();
    scenario
        .generate(&app.primary.map, &mut rng, &mut Timer::throwaway())
        .instantiate(
            &mut app.primary.sim,
            &app.primary.map,
            &mut rng,
            &mut Timer::throwaway(),
        );
}

fn make_top_bar(ctx: &mut EventCtx, app: &App, title: &str, howto: &str) -> Composite {
    Composite::new(
        Widget::col(vec![
            Widget::row(vec![
                Line(title).small_heading().draw(ctx),
                Btn::text_fg("X")
                    .build_def(ctx, hotkey(Key::Escape))
                    .align_right(),
            ]),
            howto.draw_text(ctx),
        ])
        .padding(10)
        .bg(app.cs.panel_bg),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

pub fn actions(app: &App, id: ID) -> Vec<(Key, String)> {
    let mut actions = Vec::new();
    let map = &app.primary.map;

    match id {
        ID::Lane(id) => {
            if map.get_l(id).is_driving() {
                actions.push((Key::F2, "spawn many cars starting here".to_string()));
            }
        }
        ID::Intersection(_) => {
            actions.push((Key::Z, "spawn agents here".to_string()));
        }
        _ => {}
    }
    actions
}

pub fn execute(ctx: &mut EventCtx, app: &mut App, id: ID, action: String) -> Transition {
    match (id, action.as_ref()) {
        (ID::Lane(l), "spawn many cars starting here") => {
            let color = app.cs.selected;
            let mut c = Colorer::discrete(
                ctx,
                "Spawning many agents",
                Vec::new(),
                vec![("start", color)],
            );
            c.add_l(l, color, &app.primary.map);

            Transition::Push(Box::new(SpawnManyAgents {
                composite: make_top_bar(
                    ctx,
                    app,
                    "Spawning many agents",
                    "Pick a driving lane as a destination",
                ),
                from: l,
                maybe_goal: None,
                schedule: None,
                colorer: c.build_both(ctx, app),
            }))
        }
        (ID::Intersection(id), "spawn agents here") => {
            spawn_agents_around(id, app);
            Transition::Keep
        }
        _ => unreachable!(),
    }
}
