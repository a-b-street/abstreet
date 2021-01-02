use std::collections::BTreeSet;

use maplit::btreeset;

use map_gui::tools::{grey_out_map, nice_map_name, ChooseSomething, CityPicker, PopupMsg};
use sim::{ScenarioModifier, TripMode};
use widgetry::{
    lctrl, Btn, Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    Slider, Spinner, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::checkbox_per_mode;
use crate::edit::EditMode;
use crate::sandbox::gameplay::freeform::make_change_traffic;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{Actions, SandboxControls, SandboxMode};

pub struct PlayScenario {
    top_center: Panel,
    scenario_name: String,
    modifiers: Vec<ScenarioModifier>,
}

impl PlayScenario {
    pub fn new(
        ctx: &mut EventCtx,
        name: &String,
        modifiers: Vec<ScenarioModifier>,
    ) -> Box<dyn GameplayState> {
        Box::new(PlayScenario {
            top_center: Panel::empty(ctx),
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
        _: &mut Actions,
    ) -> Option<Transition> {
        // This should really happen in the constructor once, but the old PlayScenario's
        // on_destroy can wipe this out.
        app.primary.has_modified_trips = !self.modifiers.is_empty();

        match self.top_center.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "change map" => {
                    let scenario = self.scenario_name.clone();
                    Some(Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(move |ctx, app| {
                            // Try to load a scenario with the same name exists
                            let mode = if abstio::file_exists(abstio::path_scenario(
                                app.primary.map.get_name(),
                                &scenario,
                            )) {
                                GameplayMode::PlayScenario(
                                    app.primary.map.get_name().clone(),
                                    scenario,
                                    Vec::new(),
                                )
                            } else {
                                GameplayMode::Freeform(app.primary.map.get_name().clone())
                            };
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(SandboxMode::simple_new(ctx, app, mode)),
                            ])
                        }),
                    )))
                }
                "change traffic" => Some(Transition::Push(make_change_traffic(
                    ctx,
                    app,
                    self.top_center.rect_of("change traffic").clone(),
                    self.scenario_name.clone(),
                ))),
                "edit map" => Some(Transition::Push(EditMode::new(
                    ctx,
                    app,
                    GameplayMode::PlayScenario(
                        app.primary.map.get_name().clone(),
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    ),
                ))),
                "edit traffic patterns" => Some(Transition::Push(EditScenarioModifiers::new(
                    ctx,
                    self.scenario_name.clone(),
                    self.modifiers.clone(),
                ))),
                _ => unreachable!(),
            },
            _ => None,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }

    fn on_destroy(&self, app: &mut App) {
        app.primary.has_modified_trips = false;
    }

    fn recreate_panels(&mut self, ctx: &mut EventCtx, app: &App) {
        let rows = vec![
            Widget::row(vec![
                Line("Sandbox").small_heading().draw(ctx),
                Widget::vert_separator(ctx, 50.0),
                "Map:".draw_text(ctx),
                Btn::pop_up(ctx, Some(nice_map_name(app.primary.map.get_name()))).build(
                    ctx,
                    "change map",
                    lctrl(Key::L),
                ),
                "Traffic:".draw_text(ctx),
                Btn::pop_up(ctx, Some(&self.scenario_name)).build(ctx, "change traffic", Key::S),
                Btn::svg_def("system/assets/tools/edit_map.svg").build(
                    ctx,
                    "edit map",
                    lctrl(Key::E),
                ),
            ])
            .centered(),
            if self.scenario_name == "weekday" {
                Widget::row(vec![
                    Btn::svg_def("system/assets/tools/pencil.svg").build(
                        ctx,
                        "edit traffic patterns",
                        None,
                    ),
                    format!("{} modifications to traffic patterns", self.modifiers.len())
                        .draw_text(ctx),
                ])
                .centered_horiz()
            } else {
                Widget::nothing()
            },
        ];

        self.top_center = Panel::new(Widget::col(rows))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx);
    }
}

struct EditScenarioModifiers {
    scenario_name: String,
    modifiers: Vec<ScenarioModifier>,
    panel: Panel,
}

impl EditScenarioModifiers {
    pub fn new(
        ctx: &mut EventCtx,
        scenario_name: String,
        modifiers: Vec<ScenarioModifier>,
    ) -> Box<dyn State<App>> {
        let mut rows = vec![
            Line("Modify traffic patterns").small_heading().draw(ctx),
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
            .draw(ctx),
        ];
        for (idx, m) in modifiers.iter().enumerate() {
            rows.push(
                Widget::row(vec![
                    m.describe().draw_text(ctx),
                    Btn::svg_def("system/assets/tools/delete.svg")
                        .build(ctx, format!("delete modifier {}", idx + 1), None)
                        .align_right(),
                ])
                .padding(10)
                .outline(2.0, Color::WHITE),
            );
        }
        rows.push(Btn::text_bg2("Change trip mode").build_def(ctx, None));
        rows.push(Btn::text_bg2("Add extra new trips").build_def(ctx, None));
        rows.push(Widget::row(vec![
            Spinner::new(ctx, (2, 14), 2).named("repeat_days"),
            Btn::text_bg2("Repeat schedule multiple days").build_def(ctx, None),
        ]));
        rows.push(Widget::horiz_separator(ctx, 0.5));
        rows.push(
            Widget::row(vec![
                Btn::text_bg2("Apply").build_def(ctx, Key::Enter),
                Btn::text_bg2("Discard changes").build_def(ctx, Key::Escape),
            ])
            .centered(),
        );

        Box::new(EditScenarioModifiers {
            scenario_name,
            modifiers,
            panel: Panel::new(Widget::col(rows))
                .exact_size_percent(80, 80)
                .build(ctx),
        })
    }
}

impl State<App> for EditScenarioModifiers {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Discard changes" => {
                    return Transition::Pop;
                }
                "Apply" => {
                    info!("To apply these modifiers in the future:");
                    info!(
                        "--scenario_modifiers='{}'",
                        abstutil::to_json_terse(&self.modifiers)
                    );

                    return Transition::Multi(vec![
                        Transition::Pop,
                        Transition::Replace(SandboxMode::simple_new(
                            ctx,
                            app,
                            GameplayMode::PlayScenario(
                                app.primary.map.get_name().clone(),
                                self.scenario_name.clone(),
                                self.modifiers.clone(),
                            ),
                        )),
                    ]);
                }
                "Change trip mode" => {
                    return Transition::Push(ChangeMode::new(
                        ctx,
                        app,
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    ));
                }
                "Add extra new trips" => {
                    return Transition::Push(ChooseSomething::new(
                        ctx,
                        "Which trips do you want to add in?",
                        // TODO Exclude weekday?
                        Choice::strings(abstio::list_all_objects(abstio::path_all_scenarios(
                            app.primary.map.get_name(),
                        ))),
                        Box::new(|name, _, _| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::ReplaceWithData(Box::new(|state, ctx, _| {
                                    let mut state =
                                        state.downcast::<EditScenarioModifiers>().ok().unwrap();
                                    state.modifiers.push(ScenarioModifier::AddExtraTrips(name));
                                    vec![EditScenarioModifiers::new(
                                        ctx,
                                        state.scenario_name,
                                        state.modifiers,
                                    )]
                                })),
                            ])
                        }),
                    ));
                }
                "Repeat schedule multiple days" => {
                    self.modifiers.push(ScenarioModifier::RepeatDays(
                        self.panel.spinner("repeat_days") as usize,
                    ));
                    return Transition::Replace(EditScenarioModifiers::new(
                        ctx,
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    ));
                }
                x => {
                    if let Some(x) = x.strip_prefix("delete modifier ") {
                        self.modifiers.remove(x.parse::<usize>().unwrap() - 1);
                        return Transition::Replace(EditScenarioModifiers::new(
                            ctx,
                            self.scenario_name.clone(),
                            self.modifiers.clone(),
                        ));
                    } else {
                        unreachable!()
                    }
                }
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

struct ChangeMode {
    panel: Panel,
    scenario_name: String,
    modifiers: Vec<ScenarioModifier>,
}

impl ChangeMode {
    fn new(
        ctx: &mut EventCtx,
        app: &App,
        scenario_name: String,
        modifiers: Vec<ScenarioModifier>,
    ) -> Box<dyn State<App>> {
        Box::new(ChangeMode {
            scenario_name,
            modifiers,
            panel: Panel::new(Widget::col(vec![
                Line("Change trip mode").small_heading().draw(ctx),
                Widget::row(vec![
                    "Percent of people to modify:"
                        .draw_text(ctx)
                        .centered_vert(),
                    Spinner::new(ctx, (1, 100), 50).named("pct_ppl"),
                ]),
                "Types of trips to convert:".draw_text(ctx),
                checkbox_per_mode(ctx, app, &btreeset! { TripMode::Drive }),
                Widget::row(vec![
                    "Departing from:".draw_text(ctx),
                    Slider::area(ctx, 0.25 * ctx.canvas.window_width, 0.0).named("depart from"),
                ]),
                Widget::row(vec![
                    "Departing until:".draw_text(ctx),
                    Slider::area(ctx, 0.25 * ctx.canvas.window_width, 0.3).named("depart to"),
                ]),
                Widget::horiz_separator(ctx, 0.5),
                Widget::row(vec![
                    "Change to trip type:".draw_text(ctx),
                    Widget::dropdown(ctx, "to_mode", Some(TripMode::Bike), {
                        let mut choices = vec![Choice::new("cancel trip", None)];
                        for m in TripMode::all() {
                            choices.push(Choice::new(m.ongoing_verb(), Some(m)));
                        }
                        choices
                    }),
                ]),
                Widget::row(vec![
                    Btn::text_bg2("Apply").build_def(ctx, Key::Enter),
                    Btn::text_bg2("Discard changes").build_def(ctx, Key::Escape),
                ])
                .centered(),
            ]))
            .exact_size_percent(80, 80)
            .build(ctx),
        })
    }
}

impl State<App> for ChangeMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Discard changes" => Transition::Pop,
                "Apply" => {
                    let to_mode = self.panel.dropdown_value::<Option<TripMode>, _>("to_mode");
                    let pct_ppl = self.panel.spinner("pct_ppl") as usize;
                    let (p1, p2) = (
                        self.panel.slider("depart from").get_percent(),
                        self.panel.slider("depart to").get_percent(),
                    );
                    let departure_filter = (
                        app.primary.sim.get_end_of_day().percent_of(p1),
                        app.primary.sim.get_end_of_day().percent_of(p2),
                    );
                    let mut from_modes = TripMode::all()
                        .into_iter()
                        .filter(|m| self.panel.is_checked(m.ongoing_verb()))
                        .collect::<BTreeSet<_>>();
                    if let Some(ref m) = to_mode {
                        from_modes.remove(m);
                    }

                    if from_modes.is_empty() {
                        return Transition::Push(PopupMsg::new(
                            ctx,
                            "Error",
                            vec!["You have to select at least one mode to convert from"],
                        ));
                    }
                    if p1 >= p2 {
                        return Transition::Push(PopupMsg::new(
                            ctx,
                            "Error",
                            vec!["Your time range is backwards"],
                        ));
                    }

                    let mut mods = self.modifiers.clone();
                    mods.push(ScenarioModifier::ChangeMode {
                        to_mode,
                        pct_ppl,
                        departure_filter,
                        from_modes,
                    });
                    Transition::Multi(vec![
                        Transition::Pop,
                        Transition::Replace(EditScenarioModifiers::new(
                            ctx,
                            self.scenario_name.clone(),
                            mods,
                        )),
                    ])
                }
                _ => unreachable!(),
            },
            _ => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}
