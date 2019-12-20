use crate::edit::apply_map_edits;
use crate::game::{State, Transition, WizardState};
use crate::managed::{ManagedGUIState, ManagedWidget};
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{
    hotkey, Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, ModalMenu, Text,
    VerticalAlignment,
};
use geom::{Duration, Time};
use sim::{Sim, SimFlags, SimOptions, TripMode};
use std::collections::{BTreeMap, HashSet};

// TODO Also have some kind of screenshot to display for each challenge
#[derive(Clone)]
struct Challenge {
    title: String,
    description: Vec<String>,
    map_path: String,
    gameplay: GameplayMode,
}
impl abstutil::Cloneable for Challenge {}

fn all_challenges() -> BTreeMap<String, Vec<Challenge>> {
    let mut tree = BTreeMap::new();
    tree.insert("Fix all of the traffic signals".to_string(), vec![
        Challenge {
            title: "Tutorial 1".to_string(),
            description: vec!["Add or remove a dedicated left phase".to_string()],
            map_path: abstutil::path_synthetic_map("signal_single"),
            gameplay: GameplayMode::FixTrafficSignalsTutorial(0),
        },
        Challenge {
            title: "Tutorial 2".to_string(),
            description: vec!["Deal with heavy foot traffic".to_string()],
            map_path: abstutil::path_synthetic_map("signal_single"),
            gameplay: GameplayMode::FixTrafficSignalsTutorial(1),
        },
        Challenge {
            title: "The real challenge!".to_string(),
            description: vec![
                "A city-wide power surge knocked out all of the traffic signals!".to_string(),
                "Their timing has been reset to default settings, and drivers are stuck.".to_string(),
                "It's up to you to repair the signals, choosing appropriate turn phases and timing.".to_string(),
                "".to_string(),
                "Objective: Reduce the 50%ile trip time of all drivers by at least 30s".to_string()
            ],
            map_path: abstutil::path_map("montlake"),
            gameplay: GameplayMode::FixTrafficSignals,
        },
    ]);
    tree.insert(
        "Speed up bus route 48".to_string(),
        vec![
            Challenge {
                title: "Just Montlake area".to_string(),
                description: vec![
                "Decrease the average waiting time between all of route 48's stops by at least 30s"
                    .to_string()],
                map_path: abstutil::path_map("montlake"),
                gameplay: GameplayMode::OptimizeBus("48".to_string()),
            },
            Challenge {
                title: "Larger area of the 48".to_string(),
                description: vec![
                    "Decrease the average waiting time between all of 48's stops by at least 30s"
                        .to_string(),
                ],
                map_path: abstutil::path_map("23rd"),
                gameplay: GameplayMode::OptimizeBus("48".to_string()),
            },
        ],
    );
    tree.insert(
        "Gridlock all of the everything".to_string(),
        vec![Challenge {
            title: "Gridlock all of the everything".to_string(),
            description: vec!["Make traffic as BAD as possible!".to_string()],
            map_path: abstutil::path_map("montlake"),
            gameplay: GameplayMode::CreateGridlock,
        }],
    );
    tree.insert(
        "Playing favorites".to_string(),
        vec![
            Challenge {
                title: "Speed up all bike trips".to_string(),
                description: vec![
                    "Reduce the 50%ile trip times of bikes by at least 1 minute".to_string()
                ],
                map_path: abstutil::path_map("montlake"),
                gameplay: GameplayMode::FasterTrips(TripMode::Bike),
            },
            Challenge {
                title: "Speed up all car trips".to_string(),
                description: vec![
                    "Reduce the 50%ile trip times of drivers by at least 5 minutes".to_string(),
                ],
                map_path: abstutil::path_map("montlake"),
                gameplay: GameplayMode::FasterTrips(TripMode::Drive),
            },
        ],
    );
    tree
}

pub fn challenges_picker(ctx: &EventCtx) -> Box<dyn State> {
    let mut col = Vec::new();

    col.push(ManagedWidget::row(vec![
        ManagedWidget::svg_button(
            ctx,
            "assets/pregame/back.svg",
            "back",
            hotkey(Key::Escape),
            Box::new(|_, _| Some(Transition::Pop)),
        ),
        ManagedWidget::draw_text(ctx, Text::from(Line("A/B STREET").size(50))),
    ]));

    col.push(ManagedWidget::draw_text(
        ctx,
        Text::from(Line("CHALLENGES")),
    ));

    let mut flex_row = Vec::new();
    for (idx, (name, stages)) in all_challenges().into_iter().enumerate() {
        flex_row.push(ManagedWidget::detailed_text_button(
            ctx,
            Text::from(Line(name).size(40).fg(Color::BLACK)),
            hotkey(Key::NUM_KEYS[idx]),
            Box::new(move |_, _| {
                // TODO Lifetime madness
                let stages = stages.clone();
                Some(Transition::Push(WizardState::new(Box::new(
                    move |wiz, ctx, _| {
                        let stages = stages.clone();
                        let mut wizard = wiz.wrap(ctx);
                        let (_, challenge) =
                            wizard.choose("Which stage of this challenge?", move || {
                                stages
                                    .iter()
                                    .map(|c| Choice::new(c.title.clone(), c.clone()))
                                    .collect()
                            })?;
                        wizard.reset();
                        let edits = abstutil::list_all_objects(abstutil::path_all_edits(
                            &abstutil::basename(&challenge.map_path),
                        ));
                        let mut summary = Text::new().with_bg();
                        for l in &challenge.description {
                            summary.add(Line(l));
                        }
                        summary.add(Line(""));
                        summary.add(Line(format!("{} proposals:", edits.len())));
                        summary.add(Line(""));
                        for e in edits {
                            summary.add(Line(format!("- {} (untested)", e)));
                        }

                        Some(Transition::Push(Box::new(ChallengeSplash {
                            summary,
                            menu: ModalMenu::new(
                                &challenge.title,
                                vec![
                                    (hotkey(Key::Escape), "back to challenges"),
                                    (hotkey(Key::S), "start challenge fresh"),
                                    (hotkey(Key::L), "load existing proposal"),
                                ],
                                ctx,
                            ),
                            challenge: challenge.clone(),
                        })))
                    },
                ))))
            }),
        ));
    }
    col.push(ManagedWidget::row(flex_row).flex_wrap());

    ManagedGUIState::new(ctx, ManagedWidget::col(col).centered())
}

struct ChallengeSplash {
    menu: ModalMenu,
    summary: Text,
    challenge: Challenge,
}

impl State for ChallengeSplash {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.event(ctx);
        if self.menu.action("back to challenges") {
            return Transition::Pop;
        }
        if self.menu.action("load existing proposal") {
            let map_path = self.challenge.map_path.clone();
            let gameplay = self.challenge.gameplay.clone();
            return Transition::Push(WizardState::new(Box::new(move |wiz, ctx, ui| {
                let mut wizard = wiz.wrap(ctx);
                let (_, new_edits) = wizard.choose("Load which map edits?", || {
                    Choice::from(
                        abstutil::load_all_objects(abstutil::path_all_edits(&abstutil::basename(
                            &map_path,
                        )))
                        .into_iter()
                        .filter(|(_, edits)| gameplay.allows(edits))
                        .collect(),
                    )
                })?;
                if &abstutil::basename(&map_path) != ui.primary.map.get_name() {
                    ui.switch_map(ctx, map_path.clone());
                }
                apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
                ui.primary.map.mark_edits_fresh();
                ui.primary
                    .map
                    .recalculate_pathfinding_after_edits(&mut Timer::new("finalize loaded edits"));
                Some(Transition::PopThenReplace(Box::new(SandboxMode::new(
                    ctx,
                    ui,
                    gameplay.clone(),
                ))))
            })));
        }
        if self.menu.action("start challenge fresh") {
            if &abstutil::basename(&self.challenge.map_path) != ui.primary.map.get_name() {
                ui.switch_map(ctx, self.challenge.map_path.clone());
            }
            return Transition::Replace(Box::new(SandboxMode::new(
                ctx,
                ui,
                self.challenge.gameplay.clone(),
            )));
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        g.draw_blocking_text(
            &self.summary,
            (HorizontalAlignment::Center, VerticalAlignment::Center),
        );
        self.menu.draw(g);
    }
}

// TODO Move to sim crate
pub fn prebake() {
    let mut timer = Timer::new("prebake all challenge results");

    let mut per_map: BTreeMap<String, Vec<Challenge>> = BTreeMap::new();
    for (_, list) in all_challenges() {
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
