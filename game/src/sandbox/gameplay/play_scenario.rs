use crate::app::App;
use crate::common::CityPicker;
use crate::edit::EditMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::nice_map_name;
use crate::sandbox::gameplay::freeform::make_change_traffic;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{SandboxControls, SandboxMode};
use ezgui::{
    hotkey, lctrl, Btn, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::Polygon;
use sim::ScenarioModifier;

pub struct PlayScenario {
    top_center: Composite,
    scenario_name: String,
    modifiers: Vec<ScenarioModifier>,
}

impl PlayScenario {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        name: &String,
        modifiers: Vec<ScenarioModifier>,
    ) -> Box<dyn GameplayState> {
        Box::new(PlayScenario {
            top_center: make_top_center(ctx, app, name, &modifiers),
            scenario_name: name.to_string(),
            modifiers,
        })
    }
}

impl GameplayState for PlayScenario {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
    ) -> Option<Transition> {
        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "change map" => {
                    let scenario = self.scenario_name.clone();
                    Some(Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(move |ctx, app| {
                            // The map will be switched before this callback happens.
                            let path = abstutil::path_map(app.primary.map.get_name());
                            // Try to load a scenario with the same name exists
                            let mode = if abstutil::file_exists(abstutil::path_scenario(
                                app.primary.map.get_name(),
                                &scenario,
                            )) {
                                GameplayMode::PlayScenario(path, scenario.clone(), Vec::new())
                            } else {
                                GameplayMode::Freeform(path)
                            };
                            Transition::PopThenReplace(Box::new(SandboxMode::new(ctx, app, mode)))
                        }),
                    )))
                }
                "change traffic" => Some(Transition::Push(make_change_traffic(
                    self.top_center.rect_of("change traffic").clone(),
                    self.scenario_name.clone(),
                ))),
                "edit map" => Some(Transition::Push(Box::new(EditMode::new(
                    ctx,
                    app,
                    GameplayMode::PlayScenario(
                        abstutil::path_map(app.primary.map.get_name()),
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    ),
                )))),
                "edit traffic patterns" => Some(Transition::Push(EditScenarioModifiers::new(
                    ctx,
                    app,
                    self.scenario_name.clone(),
                    self.modifiers.clone(),
                ))),
                _ => unreachable!(),
            },
            None => None,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}

fn make_top_center(
    ctx: &mut EventCtx,
    app: &App,
    scenario_name: &str,
    modifiers: &Vec<ScenarioModifier>,
) -> Composite {
    let rows = vec![
        Widget::row(vec![
            Line("Sandbox").small_heading().draw(ctx).margin(5),
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
            )
            .margin(5),
            "Map:".draw_text(ctx).margin(5),
            Btn::text_fg(format!("{} ↓", nice_map_name(app.primary.map.get_name())))
                .build(ctx, "change map", lctrl(Key::L))
                .margin(5),
            "Traffic:".draw_text(ctx).margin(5),
            Btn::text_fg(format!("{} ↓", scenario_name))
                .build(ctx, "change traffic", hotkey(Key::S))
                .margin(5),
            Btn::svg_def("../data/system/assets/tools/edit_map.svg")
                .build(ctx, "edit map", lctrl(Key::E))
                .margin(5),
        ])
        .centered(),
        if scenario_name == "weekday" {
            Widget::row(vec![
                Btn::svg_def("../data/system/assets/tools/pencil.svg")
                    .build(ctx, "edit traffic patterns", None)
                    .margin_right(15),
                format!("{} modifications to traffic patterns", modifiers.len()).draw_text(ctx),
            ])
            .centered_horiz()
        } else {
            Widget::nothing()
        },
    ];

    Composite::new(Widget::col(rows).bg(app.cs.panel_bg).padding(10))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx)
}

struct EditScenarioModifiers {
    scenario_name: String,
    modifiers: Vec<ScenarioModifier>,
    composite: Composite,
}

impl EditScenarioModifiers {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        scenario_name: String,
        modifiers: Vec<ScenarioModifier>,
    ) -> Box<dyn State> {
        let mut rows = vec![
            Line("Modify traffic patterns")
                .small_heading()
                .draw(ctx)
                .margin_below(10),
            Text::from_multiline(vec![
                Line(
                    "Data for all of the people in this simulation comes from PSRC's 2014 \
                     Soundcast model. The exact trips everybody takes, when they leave, where \
                     they go, and how they choose to get there are fixed.",
                ),
                Line(""),
                Line("You can modify those patterns here. The modifications apply in order."),
            ])
            .wrap_to_pct(ctx, 50)
            .draw(ctx)
            .margin_below(10),
        ];
        for (idx, m) in modifiers.iter().enumerate() {
            rows.push(
                Widget::row(vec![
                    m.describe().draw_text(ctx),
                    Btn::svg_def("../data/system/assets/tools/delete.svg")
                        .build(ctx, format!("delete modifier {}", idx + 1), None)
                        .align_right(),
                ])
                .padding(10)
                .outline(2.0, Color::WHITE)
                .margin_below(10),
            );
        }
        rows.push(
            Btn::text_bg2("New modification")
                .build_def(ctx, None)
                .margin_below(10),
        );
        rows.push(
            Widget::row(vec![
                Btn::text_bg2("Apply").build_def(ctx, hotkey(Key::Enter)),
                Btn::text_bg2("Discard changes").build_def(ctx, hotkey(Key::Escape)),
            ])
            .centered(),
        );

        Box::new(EditScenarioModifiers {
            scenario_name,
            modifiers,
            composite: Composite::new(Widget::col(rows).padding(16).bg(app.cs.panel_bg))
                .exact_size_percent(80, 80)
                .build(ctx),
        })
    }
}

impl State for EditScenarioModifiers {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Discard changes" => {
                    return Transition::Pop;
                }
                "Apply" => {
                    return Transition::PopThenReplace(Box::new(SandboxMode::new(
                        ctx,
                        app,
                        GameplayMode::PlayScenario(
                            abstutil::path_map(&app.primary.map.get_name()),
                            self.scenario_name.clone(),
                            self.modifiers.clone(),
                        ),
                    )));
                }
                "New modification" => {
                    return Transition::Push(new_modifier(
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    ));
                }
                x if x.starts_with("delete modifier ") => {
                    let idx = x["delete modifier ".len()..].parse::<usize>().unwrap() - 1;
                    self.modifiers.remove(idx);
                    return Transition::Replace(EditScenarioModifiers::new(
                        ctx,
                        app,
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    ));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}

// TODO Wizard isn't the right UI for this
fn new_modifier(scenario_name: String, modifiers: Vec<ScenarioModifier>) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, app| {
        let mut wizard = wiz.wrap(ctx);
        let new_mod = match wizard
            .choose_string("", || {
                vec!["repeat days", "cancel all trips for some people"]
            })?
            .as_str()
        {
            x if x == "repeat days" => ScenarioModifier::RepeatDays(
                wizard.input_usize("Repeat everyone's schedule how many days?")?,
            ),
            x if x == "cancel all trips for some people" => ScenarioModifier::CancelPeople(
                wizard.input_percent("What percent of people should cancel trips? (0 to 100)")?,
            ),
            _ => unreachable!(),
        };
        let mut mods = modifiers.clone();
        mods.push(new_mod);
        Some(Transition::PopThenReplace(EditScenarioModifiers::new(
            ctx,
            app,
            scenario_name.clone(),
            mods,
        )))
    }))
}
