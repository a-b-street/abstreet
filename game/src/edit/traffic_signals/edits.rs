use geom::Duration;
use map_gui::tools::ChooseSomething;
use map_model::{
    ControlStopSign, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, StageType,
};
use widgetry::{
    Btn, Checkbox, Choice, DrawBaselayer, EventCtx, Key, Line, Panel, SimpleState, Spinner, State,
    TextExt, Widget,
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
        signal: &ControlTrafficSignal,
        idx: usize,
    ) -> Box<dyn State<App>> {
        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line("How long should this stage last?")
                    .small_heading()
                    .draw(ctx),
                Btn::close(ctx),
            ]),
            Widget::row(vec![
                "Seconds:".draw_text(ctx).centered_vert(),
                Spinner::new(
                    ctx,
                    (
                        signal.get_min_crossing_time(idx).inner_seconds() as isize,
                        300,
                    ),
                    signal.stages[idx]
                        .stage_type
                        .simple_duration()
                        .inner_seconds() as isize,
                )
                .named("duration"),
            ]),
            Widget::row(vec![
                "Type:".draw_text(ctx),
                Checkbox::toggle(
                    ctx,
                    "stage type",
                    "fixed",
                    "variable",
                    None,
                    match signal.stages[idx].stage_type {
                        StageType::Fixed(_) => true,
                        StageType::Variable(_, _, _) => false,
                    },
                ),
            ]),
            Widget::row(vec![Line("Additional time this stage can last?")
                .small_heading()
                .draw(ctx)]),
            Widget::row(vec![
                "Seconds:".draw_text(ctx).centered_vert(),
                Spinner::new(
                    ctx,
                    (1, 300),
                    match signal.stages[idx].stage_type {
                        StageType::Fixed(_) => 0,
                        StageType::Variable(_, _, additional) => {
                            additional.inner_seconds() as isize
                        }
                    },
                )
                .named("additional"),
            ]),
            Widget::row(vec![Line("How long with no demand to end stage?")
                .small_heading()
                .draw(ctx)]),
            Widget::row(vec![
                "Seconds:".draw_text(ctx).centered_vert(),
                Spinner::new(
                    ctx,
                    (1, 300),
                    match signal.stages[idx].stage_type {
                        StageType::Fixed(_) => 0,
                        StageType::Variable(_, delay, _) => delay.inner_seconds() as isize,
                    },
                )
                .named("delay"),
            ]),
            Line("Minimum time is set by the time required for crosswalk")
                .secondary()
                .draw(ctx),
            Btn::text_bg2("Apply").build_def(ctx, Key::Enter),
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
                let dt = Duration::seconds(panel.spinner("duration") as f64);
                let new_type = if panel.is_checked("stage type") {
                    StageType::Fixed(dt)
                } else {
                    let delay = Duration::seconds(panel.spinner("delay") as f64);
                    let additional = Duration::seconds(panel.spinner("additional") as f64);
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
    let stop_sign = "convert to stop signs";
    let close = "close intersection for construction";
    let reset = "reset to default";

    let mut choices = vec![use_template];
    if has_sidewalks {
        choices.push(all_walk);
    }
    // TODO Conflating stop signs and construction here
    if mode.can_edit_stop_signs() {
        choices.push(stop_sign);
        choices.push(close);
    }
    choices.push(reset);

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
            _ => unreachable!(),
        }),
    )
}
