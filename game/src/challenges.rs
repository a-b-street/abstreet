use crate::app::App;
use crate::game::{State, Transition};
use crate::managed::{Callback, ManagedGUIState, WrappedComposite};
use crate::sandbox::{GameplayMode, SandboxMode, TutorialState};
use abstutil::Timer;
use ezgui::{hotkey, Btn, Color, Composite, EventCtx, Key, Line, Text, Widget};
use geom::{Duration, Time};
use map_model::Map;
use sim::{Scenario, Sim, SimFlags, SimOptions, TripMode};
use std::collections::{BTreeMap, HashSet};

// TODO Also have some kind of screenshot to display for each challenge
#[derive(Clone)]
pub struct Challenge {
    title: String,
    pub description: Vec<String>,
    pub alias: String,
    pub gameplay: GameplayMode,
}
impl abstutil::Cloneable for Challenge {}

pub fn all_challenges(dev: bool) -> BTreeMap<String, Vec<Challenge>> {
    let mut tree = BTreeMap::new();
    tree.insert(
        "Fix traffic signals".to_string(),
        vec![
            Challenge {
                title: "Tutorial 1".to_string(),
                description: vec!["Add or remove a dedicated left phase".to_string()],
                alias: "trafficsig/tut1".to_string(),
                gameplay: GameplayMode::FixTrafficSignalsTutorial(0),
            },
            Challenge {
                title: "Tutorial 2".to_string(),
                description: vec!["Deal with heavy foot traffic".to_string()],
                alias: "trafficsig/tut2".to_string(),
                gameplay: GameplayMode::FixTrafficSignalsTutorial(1),
            },
            Challenge {
                title: "The real challenge!".to_string(),
                description: vec![
                    "A city-wide power surge knocked out all of the traffic signals!".to_string(),
                    "Their timing has been reset to default settings, and drivers are stuck."
                        .to_string(),
                    "It's up to you to repair the signals, choosing appropriate turn phases and \
                     timing."
                        .to_string(),
                    "".to_string(),
                    "Objective: Reduce the average trip time by at least 30s".to_string(),
                ],
                alias: "trafficsig/main".to_string(),
                gameplay: GameplayMode::FixTrafficSignals,
            },
        ],
    );
    if dev {
        tree.insert(
            "Speed up a bus route (WIP)".to_string(),
            vec![
                Challenge {
                    title: "Route 43 in the small Montlake area".to_string(),
                    description: vec![
                        "Decrease the average waiting time between all of route ".to_string(),
                        "43's stops by at least 30s".to_string(),
                    ],
                    alias: "bus/43_montlake".to_string(),
                    gameplay: GameplayMode::OptimizeBus(
                        abstutil::path_map("montlake"),
                        "43".to_string(),
                    ),
                },
                Challenge {
                    title: "Route 43 in a larger area".to_string(),
                    description: vec![
                        "Decrease the average waiting time between all of route ".to_string(),
                        "43's stops by at least 30s".to_string(),
                    ],
                    alias: "bus/43_23rd".to_string(),
                    gameplay: GameplayMode::OptimizeBus(
                        abstutil::path_map("23rd"),
                        "43".to_string(),
                    ),
                },
            ],
        );
        tree.insert(
            "Cause gridlock (WIP)".to_string(),
            vec![Challenge {
                title: "Gridlock all of the everything".to_string(),
                description: vec!["Make traffic as BAD as possible!".to_string()],
                alias: "gridlock".to_string(),
                gameplay: GameplayMode::CreateGridlock(abstutil::path_map("montlake")),
            }],
        );
        tree.insert(
            "Playing favorites (WIP)".to_string(),
            vec![
                Challenge {
                    title: "Speed up all bike trips".to_string(),
                    description: vec![
                        "Reduce the 50%ile trip times of bikes by at least 1 minute".to_string()
                    ],
                    alias: "fave/bike".to_string(),
                    gameplay: GameplayMode::FasterTrips(
                        abstutil::path_map("montlake"),
                        TripMode::Bike,
                    ),
                },
                Challenge {
                    title: "Speed up all car trips".to_string(),
                    description: vec!["Reduce the 50%ile trip times of drivers by at least 5 \
                                       minutes"
                        .to_string()],
                    alias: "fave/car".to_string(),
                    gameplay: GameplayMode::FasterTrips(
                        abstutil::path_map("montlake"),
                        TripMode::Drive,
                    ),
                },
            ],
        );
    }
    tree
}

pub fn challenges_picker(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State> {
    Tab::NothingChosen.make(ctx, app)
}

enum Tab {
    NothingChosen,
    ChallengeStage(String, usize),
}

impl Tab {
    fn make(self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State> {
        let mut master_col = Vec::new();
        let mut cbs: Vec<(String, Callback)> = Vec::new();

        master_col.push(
            Btn::svg_def("../data/system/assets/pregame/back.svg")
                .build(ctx, "back", hotkey(Key::Escape))
                .align_left(),
        );
        master_col.push({
            let mut txt = Text::from(Line("A/B STREET").display_title());
            txt.add(Line("CHALLENGES").big_heading_styled());
            txt.draw(ctx).centered_horiz()
        });

        // First list challenges
        let mut flex_row = Vec::new();
        for (idx, (name, _)) in all_challenges(app.opts.dev).into_iter().enumerate() {
            let current = match self {
                Tab::NothingChosen => false,
                Tab::ChallengeStage(ref n, _) => &name == n,
            };
            if current {
                flex_row.push(Btn::text_bg2(&name).inactive(ctx).margin(10));
            } else {
                flex_row.push(
                    Btn::text_bg2(&name)
                        .build_def(ctx, hotkey(Key::NUM_KEYS[idx]))
                        .margin(10),
                );
                cbs.push((
                    name.clone(),
                    Box::new(move |ctx, app| {
                        Some(Transition::Replace(
                            Tab::ChallengeStage(name.clone(), 0).make(ctx, app),
                        ))
                    }),
                ));
            }
        }
        master_col.push(
            Widget::row(flex_row)
                .flex_wrap(ctx, 80)
                .bg(app.cs.panel_bg)
                .padding(10)
                .margin(10)
                .outline(10.0, Color::BLACK),
        );

        let mut main_row = Vec::new();

        // List stages
        if let Tab::ChallengeStage(ref name, current) = self {
            let mut col = Vec::new();
            for (idx, stage) in all_challenges(app.opts.dev)
                .remove(name)
                .unwrap()
                .into_iter()
                .enumerate()
            {
                if current == idx {
                    col.push(Btn::text_fg(&stage.title).inactive(ctx).margin(10));
                } else {
                    col.push(Btn::text_fg(&stage.title).build_def(ctx, None).margin(10));
                    let name = name.to_string();
                    cbs.push((
                        stage.title,
                        Box::new(move |ctx, app| {
                            Some(Transition::Replace(
                                Tab::ChallengeStage(name.clone(), idx).make(ctx, app),
                            ))
                        }),
                    ));
                }
            }
            main_row.push(
                Widget::col(col)
                    .bg(app.cs.panel_bg)
                    .padding(10)
                    .margin(10)
                    .outline(10.0, Color::BLACK),
            );
        }

        // Describe the specific stage
        if let Tab::ChallengeStage(ref name, current) = self {
            let challenge = all_challenges(app.opts.dev)
                .remove(name)
                .unwrap()
                .remove(current);
            let mut txt = Text::new();
            for l in &challenge.description {
                txt.add(Line(l));
            }
            main_row.push(
                Widget::col(vec![
                    txt.draw(ctx),
                    Btn::text_fg("Start!")
                        .build_def(ctx, hotkey(Key::Enter))
                        .margin(10),
                ])
                .bg(app.cs.panel_bg)
                .padding(10)
                .margin(10)
                .outline(10.0, Color::BLACK),
            );
            cbs.push((
                "Start!".to_string(),
                Box::new(move |ctx, app| {
                    Some(Transition::Replace(Box::new(SandboxMode::new(
                        ctx,
                        app,
                        challenge.gameplay.clone(),
                    ))))
                }),
            ));
        }

        master_col.push(Widget::row(main_row));

        let mut c = WrappedComposite::new(
            Composite::new(Widget::col(master_col))
                .exact_size_percent(90, 85)
                .build(ctx),
        )
        .cb("back", Box::new(|_, _| Some(Transition::Pop)));
        for (name, cb) in cbs {
            c = c.cb(&name, cb);
        }
        ManagedGUIState::fullscreen(c)
    }
}

pub fn prebake_all() {
    let mut timer = Timer::new("prebake all challenge results");

    let mut per_map: BTreeMap<String, Vec<Challenge>> = BTreeMap::new();
    for (_, list) in all_challenges(true) {
        for c in list {
            per_map
                .entry(c.gameplay.map_path())
                .or_insert_with(Vec::new)
                .push(c);
        }
    }
    for (map_path, list) in per_map {
        timer.start(format!("prebake for {}", map_path));
        let map = map_model::Map::new(map_path.clone(), false, &mut timer);

        let mut done_scenarios = HashSet::new();
        for challenge in list {
            // Bit of an abuse of this, but just need to fix the rng seed.
            if let Some(scenario) = challenge.gameplay.scenario(
                &map,
                None,
                SimFlags::for_test("prebaked").make_rng(),
                &mut timer,
            ) {
                if done_scenarios.contains(&scenario.scenario_name) {
                    continue;
                }
                done_scenarios.insert(scenario.scenario_name.clone());

                prebake(&map, scenario, &mut timer);
            }
        }
        // TODO A weird hack to glue up tutorial scenarios.
        if map.get_name() == "montlake" {
            for generator in TutorialState::scenarios_to_prebake() {
                let scenario = generator.generate(
                    &map,
                    &mut SimFlags::for_test("prebaked").make_rng(),
                    &mut timer,
                );
                prebake(&map, scenario, &mut timer);
            }
        }

        timer.stop(format!("prebake for {}", map_path));
    }
}

fn prebake(map: &Map, scenario: Scenario, timer: &mut Timer) {
    timer.start(format!(
        "prebake for {} / {}",
        scenario.map_name, scenario.scenario_name
    ));

    let mut opts = SimOptions::new("prebaked");
    opts.savestate_every = Some(Duration::hours(1));
    let mut sim = Sim::new(&map, opts, timer);
    // Bit of an abuse of this, but just need to fix the rng seed.
    let mut rng = SimFlags::for_test("prebaked").make_rng();
    scenario.instantiate(&mut sim, &map, &mut rng, timer);
    sim.timed_step(&map, Time::END_OF_DAY - Time::START_OF_DAY, timer);

    abstutil::write_binary(
        abstutil::path_prebaked_results(&scenario.map_name, &scenario.scenario_name),
        sim.get_analytics(),
    );
    timer.stop(format!(
        "prebake for {} / {}",
        scenario.map_name, scenario.scenario_name
    ));
}
