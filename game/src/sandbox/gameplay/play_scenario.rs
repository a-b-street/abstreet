use crate::app::App;
use crate::common::CityPicker;
use crate::edit::EditMode;
use crate::game::{PopupMsg, State, Transition};
use crate::helpers::{checkbox_per_mode, nice_map_name};
use crate::sandbox::gameplay::freeform::make_change_traffic;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{SandboxControls, SandboxMode};
use ezgui::{
    hotkey, lctrl, AreaSlider, Btn, Choice, Color, Composite, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Spinner, Text, TextExt, VerticalAlignment, Widget,
};
use geom::Polygon;
use maplit::btreeset;
use sim::{ScenarioModifier, TripMode};
use std::collections::BTreeSet;

pub struct PlayScenario {
    top_center: Composite,
    scenario_name: String,
    modifiers: Vec<ScenarioModifier>,
}

impl PlayScenario {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
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
                    ctx,
                    app,
                    self.top_center.rect_of("change traffic").clone(),
                    self.scenario_name.clone(),
                ))),
                "edit map" => Some(Transition::Push(EditMode::new(
                    ctx,
                    app,
                    GameplayMode::PlayScenario(
                        abstutil::path_map(app.primary.map.get_name()),
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
}

fn make_top_center(
    ctx: &mut EventCtx,
    app: &App,
    scenario_name: &str,
    modifiers: &Vec<ScenarioModifier>,
) -> Composite {
    let rows = vec![
        Widget::row(vec![
            Line("Sandbox").small_heading().draw(ctx),
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
            ),
            "Map:".draw_text(ctx),
            Btn::text_fg(format!("{} ↓", nice_map_name(app.primary.map.get_name()))).build(
                ctx,
                "change map",
                lctrl(Key::L),
            ),
            "Traffic:".draw_text(ctx),
            Btn::text_fg(format!("{} ↓", scenario_name)).build(
                ctx,
                "change traffic",
                hotkey(Key::S),
            ),
            Btn::svg_def("system/assets/tools/edit_map.svg").build(ctx, "edit map", lctrl(Key::E)),
        ])
        .centered(),
        if scenario_name == "weekday" {
            Widget::row(vec![
                Btn::svg_def("system/assets/tools/pencil.svg").build(
                    ctx,
                    "edit traffic patterns",
                    None,
                ),
                format!("{} modifications to traffic patterns", modifiers.len()).draw_text(ctx),
            ])
            .centered_horiz()
        } else {
            Widget::nothing()
        },
    ];

    Composite::new(Widget::col(rows))
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
        scenario_name: String,
        modifiers: Vec<ScenarioModifier>,
    ) -> Box<dyn State> {
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
        rows.push(Widget::row(vec![
            Spinner::new(ctx, (2, 14), 2).named("repeat_days"),
            Btn::text_bg2("Repeat schedule multiple days").build_def(ctx, None),
        ]));
        rows.push(Widget::row(vec![
            Spinner::new(ctx, (1, 100), 1).named("cancel_pct"),
            Btn::text_bg2("Cancel all trips for some percent of people").build_def(ctx, None),
        ]));
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
            composite: Composite::new(Widget::col(rows))
                .exact_size_percent(80, 80)
                .build(ctx),
        })
    }
}

impl State for EditScenarioModifiers {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
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
                "Change trip mode" => {
                    return Transition::Push(ChangeMode::new(
                        ctx,
                        app,
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    ));
                }
                "Repeat schedule multiple days" => {
                    self.modifiers.push(ScenarioModifier::RepeatDays(
                        self.composite.spinner("repeat_days") as usize,
                    ));
                    return Transition::Replace(EditScenarioModifiers::new(
                        ctx,
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    ));
                }
                "Cancel all trips for some percent of people" => {
                    self.modifiers.push(ScenarioModifier::CancelPeople(
                        self.composite.spinner("cancel_pct") as usize,
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
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}

struct ChangeMode {
    composite: Composite,
    scenario_name: String,
    modifiers: Vec<ScenarioModifier>,
}

impl ChangeMode {
    fn new(
        ctx: &mut EventCtx,
        app: &App,
        scenario_name: String,
        modifiers: Vec<ScenarioModifier>,
    ) -> Box<dyn State> {
        Box::new(ChangeMode {
            scenario_name,
            modifiers,
            composite: Composite::new(Widget::col(vec![
                Line("Change trip mode").small_heading().draw(ctx),
                Widget::row(vec![
                    "Change to trip type:".draw_text(ctx),
                    Widget::dropdown(
                        ctx,
                        "to_mode",
                        TripMode::Bike,
                        TripMode::all()
                            .into_iter()
                            .map(|m| Choice::new(m.ongoing_verb(), m))
                            .collect(),
                    ),
                ]),
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
                    AreaSlider::new(ctx, 0.25 * ctx.canvas.window_width, 0.0).named("depart from"),
                ]),
                Widget::row(vec![
                    "Departing until:".draw_text(ctx),
                    AreaSlider::new(ctx, 0.25 * ctx.canvas.window_width, 0.3).named("depart to"),
                ]),
                Widget::row(vec![
                    Btn::text_bg2("Apply").build_def(ctx, hotkey(Key::Enter)),
                    Btn::text_bg2("Discard changes").build_def(ctx, hotkey(Key::Escape)),
                ])
                .centered(),
            ]))
            .exact_size_percent(80, 80)
            .build(ctx),
        })
    }
}

impl State for ChangeMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Discard changes" => Transition::Pop,
                "Apply" => {
                    let to_mode = self.composite.dropdown_value::<TripMode>("to_mode");
                    let pct_ppl = self.composite.spinner("pct_ppl") as usize;
                    let (p1, p2) = (
                        self.composite.area_slider("depart from").get_percent(),
                        self.composite.area_slider("depart to").get_percent(),
                    );
                    let departure_filter = (
                        app.primary.sim.get_end_of_day().percent_of(p1),
                        app.primary.sim.get_end_of_day().percent_of(p2),
                    );
                    let mut from_modes = TripMode::all()
                        .into_iter()
                        .filter(|m| self.composite.is_checked(m.ongoing_verb()))
                        .collect::<BTreeSet<_>>();
                    from_modes.remove(&to_mode);

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
                    Transition::PopThenReplace(EditScenarioModifiers::new(
                        ctx,
                        self.scenario_name.clone(),
                        mods,
                    ))
                }
                _ => unreachable!(),
            },
            _ => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}
