use crate::edit::apply_map_edits;
use crate::game::{State, Transition, WizardState};
use crate::managed::{LayoutStyle, ManagedGUIState, ManagedWidget};
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{
    hotkey, Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, ModalMenu, Text,
    VerticalAlignment,
};
use geom::Time;
use sim::{SimFlags, SimOptions, TripMode};

// TODO Also have some kind of screenshot to display for each challenge
#[derive(Clone)]
struct Challenge {
    title: String,
    description: Vec<String>,
    map_name: String,
    gameplay: GameplayMode,
}
impl abstutil::Cloneable for Challenge {}

fn all_challenges() -> Vec<Challenge> {
    vec![
        Challenge {
            title: "Fix all of the traffic signals".to_string(),
            description: vec![
                "A city-wide power surge knocked out all of the traffic signals!".to_string(),
                "Their timing has been reset to default settings, and drivers are stuck.".to_string(),
                "It's up to you to repair the signals, choosing appropriate turn phases and timing.".to_string(),
                "".to_string(),
                "Objective: Reduce the 50%ile trip time of all drivers by at least 30s".to_string()
            ],
            map_name: "montlake".to_string(),
            gameplay: GameplayMode::FixTrafficSignals,
        },
        Challenge {
            title: "Speed up route 48 (just Montlake area)".to_string(),
            description: vec![
                "Decrease the average waiting time between all of route 48's stops by at least 30s"
                    .to_string()],
            map_name: "montlake".to_string(),
            gameplay: GameplayMode::OptimizeBus("48".to_string()),
        },
        Challenge {
            title: "Speed up route 48 (larger section)".to_string(),
            description: vec![
                "Decrease the average waiting time between all of 48's stops by at least 30s"
                    .to_string()],
            map_name: "23rd".to_string(),
            gameplay: GameplayMode::OptimizeBus("48".to_string()),
        },
        Challenge {
            title: "Gridlock all of the everything".to_string(),
            description: vec!["Make traffic as BAD as possible!".to_string()],
            map_name: "montlake".to_string(),
            gameplay: GameplayMode::CreateGridlock,
        },
        Challenge {
            title: "Speed up all bike trips".to_string(),
            description: vec!["Reduce the 50%ile trip times of bikes by at least 1 minute".to_string()],
            map_name: "montlake".to_string(),
            gameplay: GameplayMode::FasterTrips(TripMode::Bike),
        },
        Challenge {
            title: "Speed up all car trips".to_string(),
            description: vec!["Reduce the 50%ile trip times of drivers by at least 5 minutes"
                .to_string()],
            map_name: "montlake".to_string(),
            gameplay: GameplayMode::FasterTrips(TripMode::Drive),
        },
    ]
}

pub fn challenges_picker(ctx: &EventCtx) -> Box<dyn State> {
    let mut col = Vec::new();

    col.push(ManagedWidget::Row(
        LayoutStyle::Neutral,
        vec![
            ManagedWidget::img_button_no_bg(
                ctx,
                "assets/pregame/back.png",
                "back",
                hotkey(Key::Escape),
                Box::new(|_, _| Some(Transition::Pop)),
            ),
            ManagedWidget::draw_text(ctx, Text::from(Line("A/B STREET").size(50)).no_bg()),
        ],
    ));

    col.push(ManagedWidget::draw_text(
        ctx,
        Text::from(Line("CHALLENGES")).no_bg(),
    ));
    col.push(ManagedWidget::draw_text(
        ctx,
        Text::from(Line("Make changes to achieve a goal")).no_bg(),
    ));

    let mut flex_row = Vec::new();
    for challenge in all_challenges() {
        let edits = abstutil::list_all_objects(abstutil::EDITS, &challenge.map_name);

        let mut txt = Text::new();
        txt.add(Line(&challenge.title).size(40).fg(Color::BLACK));
        txt.add(Line(""));
        // TODO Real values
        txt.add(Line("Not completed").fg(Color::BLACK));
        txt.add(Line(format!("{} attempts", edits.len())).fg(Color::BLACK));
        txt.add(Line("Last opened ???").fg(Color::BLACK));

        flex_row.push(ManagedWidget::detailed_text_button(
            ctx,
            txt,
            None,
            Box::new(move |ctx, _| {
                let edits = abstutil::list_all_objects(abstutil::EDITS, &challenge.map_name);
                let mut summary = Text::new();
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
            }),
        ));
    }
    col.push(ManagedWidget::Row(LayoutStyle::FlexWrap, flex_row));

    ManagedGUIState::new(ManagedWidget::Column(LayoutStyle::Centered, col))
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
            let map_name = self.challenge.map_name.clone();
            let gameplay = self.challenge.gameplay.clone();
            return Transition::Push(WizardState::new(Box::new(move |wiz, ctx, ui| {
                let mut wizard = wiz.wrap(ctx);
                let (_, new_edits) = wizard.choose("Load which map edits?", || {
                    Choice::from(abstutil::load_all_objects(abstutil::EDITS, &map_name))
                })?;
                if &map_name != ui.primary.map.get_name() {
                    ui.switch_map(ctx, &map_name);
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
            if &self.challenge.map_name != ui.primary.map.get_name() {
                ui.switch_map(ctx, &self.challenge.map_name);
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

    for map_name in vec!["montlake", "23rd"] {
        timer.start(&format!("run normal sim for {}", map_name));
        let (map, mut sim, _) = SimFlags {
            load: abstutil::path1_bin(
                map_name,
                abstutil::SCENARIOS,
                "weekday_typical_traffic_from_psrc",
            ),
            use_map_fixes: true,
            rng_seed: Some(42),
            opts: SimOptions::new("prebaked"),
        }
        .load(&mut timer);
        sim.timed_step(&map, Time::END_OF_DAY - Time::START_OF_DAY, &mut timer);
        timer.stop(&format!("run normal sim for {}", map_name));

        abstutil::write_binary(
            &abstutil::path_prebaked_results(map_name),
            sim.get_analytics(),
        )
        .unwrap();
    }
}
