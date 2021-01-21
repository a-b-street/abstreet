use std::collections::BTreeMap;

use geom::{Duration, Percent};
use sim::OrigPersonID;
use widgetry::{
    Btn, Color, DrawBaselayer, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, StyledButtons,
    Text, TextExt, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::sandbox::gameplay::Tutorial;
use crate::sandbox::{GameplayMode, SandboxMode};

pub mod cutscene;
pub mod prebake;

// TODO Also have some kind of screenshot to display for each challenge
pub struct Challenge {
    title: String,
    pub description: Vec<String>,
    pub alias: String,
    pub gameplay: GameplayMode,
    pub cutscene: Option<fn(&mut EventCtx, &App, &GameplayMode) -> Box<dyn State<App>>>,
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
    panel: Panel,
    links: BTreeMap<String, (String, usize)>,
    challenge: Option<Challenge>,
}

impl ChallengesPicker {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        ChallengesPicker::make(ctx, app, None)
    }

    fn make(
        ctx: &mut EventCtx,
        app: &App,
        challenge_and_stage: Option<(String, usize)>,
    ) -> Box<dyn State<App>> {
        let mut links = BTreeMap::new();
        let mut master_col = vec![
            ctx.style()
                .btn_back_light("Home")
                .hotkey(Key::Escape)
                .build_widget(ctx, "back")
                .align_left(),
            Text::from_multiline(vec![
                Line("A/B STREET").display_title(),
                Line("CHALLENGES").big_heading_styled(),
            ])
            .draw(ctx)
            .centered_horiz(),
            ctx.style()
                .btn_primary_dark_text("Introduction and tutorial")
                .build_def(ctx)
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
                flex_row.push(
                    ctx.style()
                        .btn_primary_dark_text(&name)
                        .disabled(true)
                        .build_def(ctx),
                );
            } else {
                flex_row.push(
                    ctx.style()
                        .btn_primary_dark_text(&name)
                        .hotkey(Key::NUM_KEYS[idx])
                        .build_def(ctx),
                );
                links.insert(name.clone(), (name, 0));
            }
        }
        master_col.push(
            Widget::custom_row(flex_row)
                .flex_wrap(ctx, Percent::int(80))
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
                ctx.style()
                    .btn_secondary_light_text("Start!")
                    .hotkey(Key::Enter)
                    .build_def(ctx),
            ];

            if let Some(scores) = app.session.high_scores.get(&challenge.gameplay) {
                let mut txt = Text::from(Line(format!("{} high scores:", scores.len())));
                txt.add(Line(format!("Goal: {}", scores[0].goal)));
                let mut idx = 1;
                for score in scores {
                    txt.add(Line(format!(
                        "{}) {}, using proposal: {}",
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
            panel: Panel::new(Widget::col(master_col))
                .exact_size_percent(90, 85)
                .build_custom(ctx),
            links,
            challenge: current_challenge,
        })
    }
}

impl State<App> for ChallengesPicker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
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
                    // Constructing the cutscene doesn't require the map/scenario to be loaded
                    let sandbox = SandboxMode::simple_new(ctx, app, challenge.gameplay.clone());
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
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
    }
}
