use crate::colors;
use crate::game::{State, Transition};
use crate::managed::{Callback, ManagedGUIState, WrappedComposite};
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{hotkey, Button, Color, Composite, EventCtx, Key, Line, ManagedWidget, Text};
use geom::{Duration, Time};
use sim::{Sim, SimFlags, SimOptions, TripMode};
use std::collections::{BTreeMap, HashSet};

// TODO Also have some kind of screenshot to display for each challenge
#[derive(Clone)]
pub struct Challenge {
    title: String,
    pub description: Vec<String>,
    pub map_path: String,
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
                map_path: abstutil::path_synthetic_map("signal_single"),
                alias: "trafficsig/tut1".to_string(),
                gameplay: GameplayMode::FixTrafficSignalsTutorial(0),
            },
            Challenge {
                title: "Tutorial 2".to_string(),
                description: vec!["Deal with heavy foot traffic".to_string()],
                map_path: abstutil::path_synthetic_map("signal_single"),
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
                map_path: abstutil::path_map("montlake"),
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
                    description: vec!["Decrease the average waiting time between all of route \
                                       43's stops by at least 30s"
                        .to_string()],
                    map_path: abstutil::path_map("montlake"),
                    alias: "bus/43_montlake".to_string(),
                    gameplay: GameplayMode::OptimizeBus("43".to_string()),
                },
                Challenge {
                    title: "Route 43 in a larger area".to_string(),
                    description: vec!["Decrease the average waiting time between all of 43's \
                                       stops by at least 30s"
                        .to_string()],
                    map_path: abstutil::path_map("23rd"),
                    alias: "bus/43_23rd".to_string(),
                    gameplay: GameplayMode::OptimizeBus("43".to_string()),
                },
            ],
        );
        tree.insert(
            "Cause gridlock (WIP)".to_string(),
            vec![Challenge {
                title: "Gridlock all of the everything".to_string(),
                description: vec!["Make traffic as BAD as possible!".to_string()],
                map_path: abstutil::path_map("montlake"),
                alias: "gridlock".to_string(),
                gameplay: GameplayMode::CreateGridlock,
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
                    map_path: abstutil::path_map("montlake"),
                    alias: "fave/bike".to_string(),
                    gameplay: GameplayMode::FasterTrips(TripMode::Bike),
                },
                Challenge {
                    title: "Speed up all car trips".to_string(),
                    description: vec!["Reduce the 50%ile trip times of drivers by at least 5 \
                                       minutes"
                        .to_string()],
                    map_path: abstutil::path_map("montlake"),
                    alias: "fave/car".to_string(),
                    gameplay: GameplayMode::FasterTrips(TripMode::Drive),
                },
            ],
        );
    }
    tree
}

pub fn challenges_picker(ctx: &mut EventCtx, ui: &mut UI) -> Box<dyn State> {
    Tab::NothingChosen.make(ctx, ui)
}

enum Tab {
    NothingChosen,
    ChallengeStage(String, usize),
}

impl Tab {
    fn make(self, ctx: &mut EventCtx, ui: &mut UI) -> Box<dyn State> {
        let mut col = Vec::new();
        let mut cbs: Vec<(String, Callback)> = Vec::new();

        col.push(
            WrappedComposite::svg_button(
                ctx,
                "assets/pregame/back.svg",
                "back",
                hotkey(Key::Escape),
            )
            .align_left(),
        );
        col.push({
            let mut txt = Text::from(Line("A/B STREET").size(100));
            txt.add(Line("CHALLENGES").size(50));
            ManagedWidget::draw_text(ctx, txt).centered_horiz()
        });

        // First list challenges
        let mut flex_row = Vec::new();
        for (idx, (name, _)) in all_challenges(ui.opts.dev).into_iter().enumerate() {
            let current = match self {
                Tab::NothingChosen => false,
                Tab::ChallengeStage(ref n, _) => &name == n,
            };
            if current {
                flex_row.push(Button::inactive_button(ctx, name));
            } else {
                flex_row.push(ManagedWidget::btn(Button::text_bg(
                    Text::from(Line(&name).size(40).fg(Color::BLACK)),
                    Color::WHITE,
                    colors::HOVERING,
                    hotkey(Key::NUM_KEYS[idx]),
                    &name,
                    ctx,
                )));
                cbs.push((
                    name.clone(),
                    Box::new(move |ctx, ui| {
                        Some(Transition::Replace(
                            Tab::ChallengeStage(name.clone(), 0).make(ctx, ui),
                        ))
                    }),
                ));
            }
        }
        col.push(
            ManagedWidget::row(flex_row)
                .flex_wrap(ctx, 80)
                .bg(colors::PANEL_BG)
                .padding(10),
        );

        // List stages
        if let Tab::ChallengeStage(ref name, current) = self {
            let mut flex_row = Vec::new();
            for (idx, stage) in all_challenges(ui.opts.dev)
                .remove(name)
                .unwrap()
                .into_iter()
                .enumerate()
            {
                if current == idx {
                    flex_row.push(Button::inactive_button(ctx, &stage.title));
                } else {
                    flex_row.push(WrappedComposite::text_button(ctx, &stage.title, None));
                    let name = name.to_string();
                    cbs.push((
                        stage.title,
                        Box::new(move |ctx, ui| {
                            Some(Transition::Replace(
                                Tab::ChallengeStage(name.clone(), idx).make(ctx, ui),
                            ))
                        }),
                    ));
                }
            }
            col.push(
                ManagedWidget::row(flex_row)
                    .flex_wrap(ctx, 80)
                    .bg(colors::PANEL_BG)
                    .padding(10),
            );
        }

        // Describe the specific stage
        if let Tab::ChallengeStage(ref name, current) = self {
            let challenge = all_challenges(ui.opts.dev)
                .remove(name)
                .unwrap()
                .remove(current);
            let mut txt = Text::new();
            for l in &challenge.description {
                txt.add(Line(l));
            }
            col.push(
                ManagedWidget::col(vec![
                    ManagedWidget::draw_text(ctx, txt),
                    WrappedComposite::text_button(ctx, "Start!", hotkey(Key::Enter)).margin(10),
                ])
                .bg(colors::PANEL_BG)
                .padding(10),
            );
            cbs.push((
                "Start!".to_string(),
                Box::new(move |ctx, ui| {
                    if &abstutil::basename(&challenge.map_path) != ui.primary.map.get_name() {
                        ui.switch_map(ctx, challenge.map_path.clone());
                    }
                    Some(Transition::Replace(Box::new(SandboxMode::new(
                        ctx,
                        ui,
                        challenge.gameplay.clone(),
                    ))))
                }),
            ));
        }

        let mut c = WrappedComposite::new(
            Composite::new(ManagedWidget::col(col).evenly_spaced())
                .exact_size_percent(90, 90)
                .build(ctx),
        )
        .cb("back", Box::new(|_, _| Some(Transition::Pop)));
        for (name, cb) in cbs {
            c = c.cb(&name, cb);
        }
        ManagedGUIState::fullscreen(c)
    }
}

pub fn prebake() {
    let mut timer = Timer::new("prebake all challenge results");

    let mut per_map: BTreeMap<String, Vec<Challenge>> = BTreeMap::new();
    for (_, list) in all_challenges(true) {
        for c in list {
            per_map
                .entry(c.map_path.clone())
                .or_insert_with(Vec::new)
                .push(c);
        }
    }
    for (map_path, list) in per_map {
        timer.start(format!("prebake for {}", map_path));
        let map = map_model::Map::new(map_path.clone(), false, &mut timer);

        let mut done_scenarios = HashSet::new();
        for challenge in list {
            if let Some(scenario) = challenge.gameplay.scenario(&map, None, &mut timer) {
                if done_scenarios.contains(&scenario.scenario_name) {
                    continue;
                }
                done_scenarios.insert(scenario.scenario_name.clone());
                timer.start(format!(
                    "prebake for {} / {}",
                    scenario.map_name, scenario.scenario_name
                ));

                let mut opts = SimOptions::new("prebaked");
                opts.savestate_every = Some(Duration::hours(1));
                let mut sim = Sim::new(&map, opts, &mut timer);
                // Bit of an abuse of this, but just need to fix the rng seed.
                let mut rng = SimFlags::for_test("prebaked").make_rng();
                scenario.instantiate(&mut sim, &map, &mut rng, &mut timer);
                sim.timed_step(&map, Time::END_OF_DAY - Time::START_OF_DAY, &mut timer);

                abstutil::write_binary(
                    abstutil::path_prebaked_results(&scenario.map_name, &scenario.scenario_name),
                    sim.get_analytics(),
                );
                timer.stop(format!(
                    "prebake for {} / {}",
                    scenario.map_name, scenario.scenario_name
                ));
            }
        }

        timer.stop(format!("prebake for {}", map_path));
    }
}
