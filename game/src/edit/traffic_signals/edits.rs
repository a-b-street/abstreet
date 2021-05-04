use geom::Duration;
use map_gui::tools::{ChooseSomething, PopupMsg};
use map_model::{
    ControlStopSign, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, StageType,
};
use widgetry::{
    Choice, DrawBaselayer, EventCtx, Key, Line, Panel, SimpleState, Spinner, State, Text, TextExt,
    Widget,
};

use crate::app::{App, Transition};
use crate::edit::traffic_signals::{BundleEdits, TrafficSignalEditor};
use crate::edit::{apply_map_edits, check_sidewalk_connectivity, StopSignEditor};
use crate::sandbox::GameplayMode;

pub struct ChangeDuration {
    idx: usize,
}

impl ChangeDuration {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        signal: &ControlTrafficSignal,
        idx: usize,
    ) -> Box<dyn State<App>> {
        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line("How long should this stage last?")
                    .small_heading()
                    .into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Widget::row(vec![
                "Duration:".text_widget(ctx).centered_vert(),
                Spinner::widget(
                    ctx,
                    "duration",
                    (signal.get_min_crossing_time(idx), Duration::minutes(5)),
                    signal.stages[idx].stage_type.simple_duration(),
                    Duration::seconds(1.0),
                ),
            ]),
            Line("Minimum time is set by the time required for crosswalk")
                .secondary()
                .into_widget(ctx),
            Widget::col(vec![
                Text::from_all(match signal.stages[idx].stage_type {
                    StageType::Fixed(_) => vec![
                        Line("Fixed timing").small_heading(),
                        Line(" (Adjust both values below to enable variable timing)"),
                    ],
                    StageType::Variable(_, _, _) => vec![
                        Line("Variable timing").small_heading(),
                        Line(" (Set either values below to 0 to use fixed timing."),
                    ],
                })
                .into_widget(ctx)
                .named("timing type"),
                Widget::row(vec![
                    "How much additional time can this stage last?"
                        .text_widget(ctx)
                        .centered_vert(),
                    Spinner::widget(
                        ctx,
                        "additional",
                        (Duration::ZERO, Duration::minutes(5)),
                        match signal.stages[idx].stage_type {
                            StageType::Fixed(_) => Duration::ZERO,
                            StageType::Variable(_, _, additional) => additional,
                        },
                        Duration::seconds(1.0),
                    ),
                ]),
                Widget::row(vec![
                    "How long with no demand before the stage ends?"
                        .text_widget(ctx)
                        .centered_vert(),
                    Spinner::widget(
                        ctx,
                        "delay",
                        (Duration::ZERO, Duration::seconds(300.0)),
                        match signal.stages[idx].stage_type {
                            StageType::Fixed(_) => Duration::ZERO,
                            StageType::Variable(_, delay, _) => delay,
                        },
                        Duration::seconds(1.0),
                    ),
                ]),
            ])
            .padding(10)
            .bg(app.cs.inner_panel_bg)
            .outline(ctx.style().section_outline),
            ctx.style()
                .btn_solid_primary
                .text("Apply")
                .hotkey(Key::Enter)
                .build_def(ctx),
        ]))
        .build(ctx);
        SimpleState::new(panel, Box::new(ChangeDuration { idx }))
    }
}

impl SimpleState<App> for ChangeDuration {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, panel: &Panel) -> Transition {
        match x {
            "close" => Transition::Pop,
            "Apply" => {
                let dt = panel.spinner("duration");
                let delay = panel.spinner("delay");
                let additional = panel.spinner("additional");
                let new_type = if delay == Duration::ZERO || additional == Duration::ZERO {
                    StageType::Fixed(dt)
                } else {
                    StageType::Variable(dt, delay, additional)
                };
                let idx = self.idx;
                Transition::Multi(vec![
                    Transition::Pop,
                    Transition::ModifyState(Box::new(move |state, ctx, app| {
                        let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                        editor.add_new_edit(ctx, app, idx, |ts| {
                            ts.stages[idx].stage_type = new_type.clone();
                        });
                    })),
                ])
            }
            _ => unreachable!(),
        }
    }

    fn panel_changed(
        &mut self,
        ctx: &mut EventCtx,
        _: &mut App,
        panel: &mut Panel,
    ) -> Option<Transition> {
        let new_label = Text::from_all(
            if panel.spinner::<Duration>("delay") == Duration::ZERO
                || panel.spinner::<Duration>("additional") == Duration::ZERO
            {
                vec![
                    Line("Fixed timing").small_heading(),
                    Line(" (Adjust both values below to enable variable timing)"),
                ]
            } else {
                vec![
                    Line("Variable timing").small_heading(),
                    Line(" (Set either values below to 0 to use fixed timing."),
                ]
            },
        )
        .into_widget(ctx);
        panel.replace(ctx, "timing type", new_label);
        None
    }

    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        if ctx.normal_left_click() && ctx.canvas.get_cursor_in_screen_space().is_none() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }
}

pub fn edit_entire_signal(
    ctx: &mut EventCtx,
    app: &App,
    i: IntersectionID,
    mode: GameplayMode,
    original: BundleEdits,
) -> Box<dyn State<App>> {
    let has_sidewalks = app
        .primary
        .map
        .get_turns_in_intersection(i)
        .into_iter()
        .any(|t| t.between_sidewalks());

    let use_template = "use template";
    let all_walk = "add an all-walk stage at the end";
    let major_minor_timing = "use timing pattern for a major/minor intersection";
    let stop_sign = "convert to stop signs";
    let close = "close intersection for construction";
    let reset = "reset to default";
    let gmns = "import from GMNS timing.csv";

    let mut choices = vec![use_template];
    if has_sidewalks {
        choices.push(all_walk);
    }
    choices.push(major_minor_timing);
    // TODO Conflating stop signs and construction here
    if mode.can_edit_stop_signs() {
        choices.push(stop_sign);
        choices.push(close);
    }
    choices.push(reset);
    if app.opts.dev {
        choices.push(gmns);
    }

    ChooseSomething::new(
        ctx,
        "What do you want to change?",
        Choice::strings(choices),
        Box::new(move |x, ctx, app| match x.as_str() {
            x if x == use_template => Transition::Replace(ChooseSomething::new(
                ctx,
                "Use which preset for this intersection?",
                Choice::from(ControlTrafficSignal::get_possible_policies(
                    &app.primary.map,
                    i,
                )),
                Box::new(move |new_signal, _, _| {
                    Transition::Multi(vec![
                        Transition::Pop,
                        Transition::ModifyState(Box::new(move |state, ctx, app| {
                            let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                            editor.add_new_edit(ctx, app, 0, |ts| {
                                *ts = new_signal.clone();
                            });
                        })),
                    ])
                }),
            )),
            x if x == all_walk => Transition::Multi(vec![
                Transition::Pop,
                Transition::ModifyState(Box::new(move |state, ctx, app| {
                    let mut new_signal = app.primary.map.get_traffic_signal(i).clone();
                    if new_signal.convert_to_ped_scramble() {
                        let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                        editor.add_new_edit(ctx, app, 0, |ts| {
                            *ts = new_signal.clone();
                        });
                    }
                })),
            ]),
            x if x == major_minor_timing => Transition::Replace(ChooseSomething::new(
                ctx,
                "Use what timing split?",
                vec![
                    Choice::new(
                        "120s cycle: 96s major roads, 24s minor roads",
                        (Duration::seconds(96.0), Duration::seconds(24.0)),
                    ),
                    Choice::new(
                        "60s cycle: 36s major roads, 24s minor roads",
                        (Duration::seconds(36.0), Duration::seconds(24.0)),
                    ),
                ],
                Box::new(move |timing, ctx, app| {
                    let mut new_signal = app.primary.map.get_traffic_signal(i).clone();
                    match new_signal.adjust_major_minor_timing(timing.0, timing.1, &app.primary.map)
                    {
                        Ok(()) => Transition::Multi(vec![
                            Transition::Pop,
                            Transition::ModifyState(Box::new(move |state, ctx, app| {
                                let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                                editor.add_new_edit(ctx, app, 0, |ts| {
                                    *ts = new_signal.clone();
                                });
                            })),
                        ]),
                        Err(err) => {
                            Transition::Replace(PopupMsg::new(ctx, "Error", vec![err.to_string()]))
                        }
                    }
                }),
            )),
            x if x == stop_sign => {
                original.apply(app);

                let mut edits = app.primary.map.get_edits().clone();
                edits.commands.push(EditCmd::ChangeIntersection {
                    i,
                    old: app.primary.map.get_i_edit(i),
                    new: EditIntersection::StopSign(ControlStopSign::new(&app.primary.map, i)),
                });
                apply_map_edits(ctx, app, edits);
                Transition::Multi(vec![
                    Transition::Pop,
                    Transition::Replace(StopSignEditor::new(ctx, app, i, mode.clone())),
                ])
            }
            x if x == close => {
                original.apply(app);

                let cmd = EditCmd::ChangeIntersection {
                    i,
                    old: app.primary.map.get_i_edit(i),
                    new: EditIntersection::Closed,
                };
                if let Some(err) = check_sidewalk_connectivity(ctx, app, cmd.clone()) {
                    Transition::Replace(err)
                } else {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.commands.push(cmd);
                    apply_map_edits(ctx, app, edits);

                    Transition::Multi(vec![Transition::Pop, Transition::Pop])
                }
            }
            x if x == reset => Transition::Multi(vec![
                Transition::Pop,
                Transition::ModifyState(Box::new(move |state, ctx, app| {
                    let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                    let new_signal =
                        ControlTrafficSignal::get_possible_policies(&app.primary.map, i)
                            .remove(0)
                            .1;
                    editor.add_new_edit(ctx, app, 0, |ts| {
                        *ts = new_signal.clone();
                    });
                })),
            ]),
            x if x == gmns => Transition::Multi(vec![
                Transition::Pop,
                // TODO File picker
                match crate::edit::traffic_signals::gmns::import(
                    &app.primary.map,
                    i,
                    "/home/dabreegster/timing.csv",
                ) {
                    Ok(new_signal) => Transition::ModifyState(Box::new(move |state, ctx, app| {
                        let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();
                        editor.add_new_edit(ctx, app, 0, |ts| {
                            *ts = new_signal.clone();
                        });
                    })),
                    Err(err) => {
                        Transition::Push(PopupMsg::new(ctx, "Error", vec![err.to_string()]))
                    }
                },
            ]),
            _ => unreachable!(),
        }),
    )
}
