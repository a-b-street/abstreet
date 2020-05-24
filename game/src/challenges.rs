use crate::app::App;
use crate::game::{State, Transition};
use crate::managed::{Callback, ManagedGUIState, WrappedComposite};
use crate::sandbox::{GameplayMode, SandboxMode, TutorialState};
use abstutil::Timer;
use ezgui::{hotkey, Btn, Color, Composite, EventCtx, Key, Line, Text, TextExt, Widget};
use geom::{Duration, Time};
use map_model::Map;
use sim::{AlertHandler, OrigPersonID, Scenario, Sim, SimFlags, SimOptions};
use std::collections::{BTreeMap, HashSet};

// TODO Also have some kind of screenshot to display for each challenge
pub struct Challenge {
    title: String,
    pub description: Vec<String>,
    pub alias: String,
    pub gameplay: GameplayMode,
    pub cutscene: Option<fn(&mut EventCtx, &App, &GameplayMode) -> Box<dyn State>>,
}

pub struct HighScore {
    // TODO This should be tied to the GameplayMode
    pub goal: String,
    // TODO Assuming we always want to maximize the score
    pub score: Duration,
    pub edits_name: String,
}

impl HighScore {
    pub fn record(self, app: &mut App, mode: GameplayMode) {
        // TODO dedupe
        // TODO mention placement
        // TODO show all of em
        let scores = app.session.high_scores.entry(mode).or_insert_with(Vec::new);
        scores.push(self);
        scores.sort_by_key(|s| s.score);
        scores.reverse();
    }
}

impl Challenge {
    pub fn all() -> BTreeMap<String, Vec<Challenge>> {
        let mut tree = BTreeMap::new();
        tree.insert(
            "Optimize one commute".to_string(),
            // TODO Need to tune both people and goals again.
            vec![
                Challenge {
                    title: "Part 1".to_string(),
                    description: vec!["Speed up one VIP's daily commute, at any cost!".to_string()],
                    alias: "commute/pt1".to_string(),
                    gameplay: GameplayMode::OptimizeCommute(
                        OrigPersonID(140030, 1),
                        Duration::minutes(2),
                    ),
                    cutscene: Some(
                        crate::sandbox::gameplay::commute::OptimizeCommute::cutscene_pt1,
                    ),
                },
                Challenge {
                    title: "Part 2".to_string(),
                    description: vec!["Speed up another VIP's commute".to_string()],
                    alias: "commute/pt2".to_string(),
                    gameplay: GameplayMode::OptimizeCommute(
                        OrigPersonID(140288, 3),
                        Duration::seconds(90.0),
                    ),
                    cutscene: Some(
                        crate::sandbox::gameplay::commute::OptimizeCommute::cutscene_pt2,
                    ),
                },
            ],
        );
        tree.insert(
            "Fix traffic signals".to_string(),
            vec![Challenge {
                title: "Repair traffic signals".to_string(),
                description: vec!["Fix traffic signal timing and unblock vehicles".to_string()],
                alias: "trafficsig/pt1".to_string(),
                gameplay: GameplayMode::FixTrafficSignals,
                cutscene: Some(
                    crate::sandbox::gameplay::fix_traffic_signals::FixTrafficSignals::cutscene_pt1,
                ),
            }],
        );

        tree
    }

    // Also returns the next stage, if there is one
    pub fn find(mode: &GameplayMode) -> (Challenge, Option<Challenge>) {
        // Find the next stage
        for (_, stages) in Challenge::all() {
            let mut current = None;
            for challenge in stages {
                if current.is_some() {
                    return (current.unwrap(), Some(challenge));
                }
                if &challenge.gameplay == mode {
                    current = Some(challenge);
                }
            }
            if let Some(c) = current {
                return (c, None);
            }
        }
        unreachable!()
    }
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
        for (idx, (name, _)) in Challenge::all().into_iter().enumerate() {
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
            for (idx, stage) in Challenge::all()
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
            let challenge = Challenge::all().remove(name).unwrap().remove(current);
            let mut txt = Text::new();
            for l in &challenge.description {
                txt.add(Line(l));
            }

            let mut inner_col = vec![
                txt.draw(ctx),
                Btn::text_fg("Start!")
                    .build_def(ctx, hotkey(Key::Enter))
                    .margin(10),
            ];

            if let Some(scores) = app.session.high_scores.get(&challenge.gameplay) {
                let mut txt = Text::from(Line(format!("{} high scores:", scores.len())));
                txt.add(Line(format!("Goal: {}", scores[0].goal)));
                let mut idx = 1;
                for score in scores {
                    txt.add(Line(format!(
                        "{}) {}, using edits: {}",
                        idx, score.score, score.edits_name
                    )));
                    idx += 1;
                }
                inner_col.push(txt.draw(ctx));
            } else {
                inner_col.push("No attempts yet".draw_text(ctx));
            }

            main_row.push(
                Widget::col(inner_col)
                    .bg(app.cs.panel_bg)
                    .padding(10)
                    .margin(10)
                    .outline(10.0, Color::BLACK),
            );
            cbs.push((
                "Start!".to_string(),
                Box::new(move |ctx, app| {
                    let sandbox = Box::new(SandboxMode::new(ctx, app, challenge.gameplay.clone()));
                    if let Some(cutscene) = challenge.cutscene {
                        Some(Transition::ReplaceThenPush(
                            sandbox,
                            cutscene(ctx, app, &challenge.gameplay),
                        ))
                    } else {
                        Some(Transition::Replace(sandbox))
                    }
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

    {
        let map = map_model::Map::new(abstutil::path_map("montlake"), &mut timer);
        let scenario: Scenario =
            abstutil::read_binary(abstutil::path_scenario("montlake", "weekday"), &mut timer);
        prebake(&map, scenario, None, &mut timer);

        for generator in TutorialState::scenarios_to_prebake(&map) {
            let scenario = generator.generate(
                &map,
                &mut SimFlags::for_test("prebaked").make_rng(),
                &mut timer,
            );
            prebake(&map, scenario, None, &mut timer);
        }
    }

    for name in vec!["23rd", "lakeslice"] {
        let map = map_model::Map::new(abstutil::path_map(name), &mut timer);
        let scenario: Scenario =
            abstutil::read_binary(abstutil::path_scenario(name, "weekday"), &mut timer);
        prebake(&map, scenario, None, &mut timer);
    }
}

// TODO This variant will be more useful when all scenarios tend to actually complete. ;)
#[allow(unused)]
pub fn generic_prebake_all() {
    let mut timer = Timer::new("prebake all challenge results");

    let mut per_map: BTreeMap<String, Vec<Challenge>> = BTreeMap::new();
    for (_, list) in Challenge::all() {
        for c in list {
            per_map
                .entry(c.gameplay.map_path())
                .or_insert_with(Vec::new)
                .push(c);
        }
    }
    for (map_path, list) in per_map {
        timer.start(format!("prebake for {}", map_path));
        let map = map_model::Map::new(map_path.clone(), &mut timer);

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

                prebake(&map, scenario, None, &mut timer);
            }
        }
        // TODO A weird hack to glue up tutorial scenarios.
        if map.get_name() == "montlake" {
            for generator in TutorialState::scenarios_to_prebake(&map) {
                let scenario = generator.generate(
                    &map,
                    &mut SimFlags::for_test("prebaked").make_rng(),
                    &mut timer,
                );
                prebake(&map, scenario, None, &mut timer);
            }
        }

        timer.stop(format!("prebake for {}", map_path));
    }
}

fn prebake(map: &Map, scenario: Scenario, time_limit: Option<Duration>, timer: &mut Timer) {
    timer.start(format!(
        "prebake for {} / {}",
        scenario.map_name, scenario.scenario_name
    ));

    let mut opts = SimOptions::new("prebaked");
    opts.alerts = AlertHandler::Silence;
    let mut sim = Sim::new(&map, opts, timer);
    // Bit of an abuse of this, but just need to fix the rng seed.
    let mut rng = SimFlags::for_test("prebaked").make_rng();
    scenario.instantiate(&mut sim, &map, &mut rng, timer);
    if let Some(dt) = time_limit {
        sim.timed_step(&map, dt, timer);
    } else {
        sim.timed_step(&map, sim.get_end_of_day() - Time::START_OF_DAY, timer);
    }

    abstutil::write_binary(
        abstutil::path_prebaked_results(&scenario.map_name, &scenario.scenario_name),
        sim.get_analytics(),
    );
    timer.stop(format!(
        "prebake for {} / {}",
        scenario.map_name, scenario.scenario_name
    ));
}
