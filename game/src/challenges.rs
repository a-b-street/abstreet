use crate::app::App;
use crate::game::{DrawBaselayer, State, Transition};
use crate::sandbox::gameplay::Tutorial;
use crate::sandbox::{GameplayMode, SandboxMode, TutorialState};
use abstutil::{prettyprint_usize, Timer};
use ezgui::{
    hotkey, Btn, Color, Composite, EventCtx, GfxCtx, Key, Line, Outcome, Text, TextExt, Widget,
};
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
                        OrigPersonID(140824, 2),
                        Duration::minutes(2) + Duration::seconds(30.0),
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
                        OrigPersonID(141039, 2),
                        Duration::minutes(5),
                    ),
                    cutscene: Some(
                        crate::sandbox::gameplay::commute::OptimizeCommute::cutscene_pt2,
                    ),
                },
            ],
        );
        tree.insert(
            "Traffic signal survivor".to_string(),
            vec![Challenge {
                title: "Traffic signal survivor".to_string(),
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

pub struct ChallengesPicker {
    composite: Composite,
    links: BTreeMap<String, (String, usize)>,
    challenge: Option<Challenge>,
}

impl ChallengesPicker {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        ChallengesPicker::make(ctx, app, None)
    }

    fn make(
        ctx: &mut EventCtx,
        app: &App,
        challenge_and_stage: Option<(String, usize)>,
    ) -> Box<dyn State> {
        let mut links = BTreeMap::new();
        let mut master_col = vec![
            Btn::svg_def("system/assets/pregame/back.svg")
                .build(ctx, "back", hotkey(Key::Escape))
                .align_left(),
            Text::from_multiline(vec![
                Line("A/B STREET").display_title(),
                Line("CHALLENGES").big_heading_styled(),
            ])
            .draw(ctx)
            .centered_horiz(),
            Btn::text_bg2("Introduction and tutorial")
                .build_def(ctx, None)
                .centered_horiz()
                .bg(app.cs.panel_bg)
                .padding(16)
                .outline(2.0, Color::BLACK),
        ];

        // First list challenges
        let mut flex_row = Vec::new();
        for (idx, (name, _)) in Challenge::all().into_iter().enumerate() {
            if challenge_and_stage
                .as_ref()
                .map(|(n, _)| n == &name)
                .unwrap_or(false)
            {
                flex_row.push(Btn::text_bg2(&name).inactive(ctx));
            } else {
                flex_row.push(Btn::text_bg2(&name).build_def(ctx, hotkey(Key::NUM_KEYS[idx])));
                links.insert(name.clone(), (name, 0));
            }
        }
        master_col.push(
            Widget::custom_row(flex_row)
                .flex_wrap(ctx, 80)
                .bg(app.cs.panel_bg)
                .padding(16)
                .outline(2.0, Color::BLACK),
        );

        let mut main_row = Vec::new();

        // List stages
        if let Some((ref name, current)) = challenge_and_stage {
            let mut col = Vec::new();
            for (idx, stage) in Challenge::all()
                .remove(name)
                .unwrap()
                .into_iter()
                .enumerate()
            {
                if current == idx {
                    col.push(Btn::text_fg(&stage.title).inactive(ctx));
                } else {
                    col.push(Btn::text_fg(&stage.title).build_def(ctx, None));
                    links.insert(stage.title, (name.to_string(), idx));
                }
            }
            main_row.push(
                Widget::col(col)
                    .bg(app.cs.panel_bg)
                    .padding(16)
                    .outline(2.0, Color::BLACK),
            );
        }

        // Describe the specific stage
        let mut current_challenge = None;
        if let Some((ref name, current)) = challenge_and_stage {
            let challenge = Challenge::all().remove(name).unwrap().remove(current);
            let mut txt = Text::new();
            for l in &challenge.description {
                txt.add(Line(l));
            }

            let mut inner_col = vec![
                txt.draw(ctx),
                Btn::text_fg("Start!").build_def(ctx, hotkey(Key::Enter)),
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
                    .padding(16)
                    .outline(2.0, Color::BLACK),
            );
            current_challenge = Some(challenge);
        }

        master_col.push(Widget::row(main_row));

        Box::new(ChallengesPicker {
            composite: Composite::new(Widget::col(master_col))
                .exact_size_percent(90, 85)
                .build_custom(ctx),
            links,
            challenge: current_challenge,
        })
    }
}

impl State for ChallengesPicker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "back" => {
                    return Transition::Pop;
                }
                "Introduction and tutorial" => {
                    // Slightly inconsistent: pushes twice and leaves this challenge picker open
                    return Tutorial::start(ctx, app);
                }
                "Start!" => {
                    let challenge = self.challenge.take().unwrap();
                    let sandbox = SandboxMode::new(ctx, app, challenge.gameplay.clone());
                    if let Some(cutscene) = challenge.cutscene {
                        Transition::Multi(vec![
                            Transition::Replace(sandbox),
                            Transition::Push(cutscene(ctx, app, &challenge.gameplay)),
                        ])
                    } else {
                        Transition::Replace(sandbox)
                    }
                }
                x => {
                    return Transition::Replace(ChallengesPicker::make(
                        ctx,
                        app,
                        self.links.remove(x),
                    ));
                }
            },
            _ => Transition::Keep,
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.grass);
        self.composite.draw(g);
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

    for name in vec!["lakeslice"] {
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
        sim.timed_step(&map, dt, &mut None, timer);
    } else {
        sim.timed_step(
            &map,
            sim.get_end_of_day() - Time::START_OF_DAY,
            &mut None,
            timer,
        );
    }

    abstutil::write_binary(
        abstutil::path_prebaked_results(&scenario.map_name, &scenario.scenario_name),
        sim.get_analytics(),
    );
    let agents_left = sim.num_agents().sum();
    timer.note(format!("{} agents left by end of day", agents_left));
    timer.stop(format!(
        "prebake for {} / {}",
        scenario.map_name, scenario.scenario_name
    ));

    // TODO Ah, it's people waiting on a bus that never spawned. Woops.
    if agents_left > 500 && false {
        panic!(
            "{} agents left by end of day on {}; seems bad",
            prettyprint_usize(agents_left),
            scenario.map_name
        );
    }
}
