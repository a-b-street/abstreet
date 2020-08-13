use crate::app::App;
use crate::edit::traffic_signals::{make_top_panel, BundleEdits, TrafficSignalEditor};
use crate::edit::{apply_map_edits, check_sidewalk_connectivity, StopSignEditor};
use crate::game::{ChooseSomething, DrawBaselayer, State, Transition};
use crate::sandbox::GameplayMode;
use abstutil::Timer;
use ezgui::{
    hotkey, Btn, Checkbox, Choice, Composite, EventCtx, GfxCtx, Key, Line, Outcome, Spinner,
    TextExt, Widget,
};
use geom::Duration;
use map_model::{
    ControlStopSign, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, PhaseType,
};

pub struct ChangeDuration {
    composite: Composite,
    idx: usize,
}

impl ChangeDuration {
    pub fn new(ctx: &mut EventCtx, current: PhaseType, idx: usize) -> Box<dyn State> {
        Box::new(ChangeDuration {
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Line("How long should this phase last?")
                        .small_heading()
                        .draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Widget::row(vec![
                    "Seconds:".draw_text(ctx),
                    Spinner::new(
                        ctx,
                        (5, 300),
                        current.simple_duration().inner_seconds() as isize,
                    )
                    .named("duration"),
                ]),
                Widget::row(vec![
                    "Type:".draw_text(ctx),
                    Checkbox::toggle(
                        ctx,
                        "phase type",
                        "fixed",
                        "adaptive",
                        None,
                        match current {
                            PhaseType::Fixed(_) => true,
                            PhaseType::Adaptive(_) => false,
                        },
                    ),
                ]),
                Btn::text_bg2("Apply").build_def(ctx, hotkey(Key::Enter)),
            ]))
            .build(ctx),
            idx,
        })
    }
}

impl State for ChangeDuration {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                "Apply" => {
                    let dt = Duration::seconds(self.composite.spinner("duration") as f64);
                    let new_type = if self.composite.is_checked("phase type") {
                        PhaseType::Fixed(dt)
                    } else {
                        PhaseType::Adaptive(dt)
                    };
                    let idx = self.idx;
                    return Transition::PopWithData(Box::new(move |state, ctx, app| {
                        let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();

                        let mut bundle = BundleEdits::get_current(app, &editor.members);
                        editor.command_stack.push(bundle.clone());
                        editor.redo_stack.clear();
                        for ts in &mut bundle.signals {
                            ts.phases[idx].phase_type = new_type.clone();
                        }
                        bundle.apply(app);

                        editor.top_panel = make_top_panel(ctx, app, true, false);
                        editor.change_phase(ctx, app, idx);
                    }));
                }
                _ => unreachable!(),
            },
            _ => {
                if ctx.normal_left_click() && ctx.canvas.get_cursor_in_screen_space().is_none() {
                    return Transition::Pop;
                }
                Transition::Keep
            }
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
    }
}

pub fn edit_entire_signal(
    ctx: &mut EventCtx,
    app: &App,
    i: IntersectionID,
    mode: GameplayMode,
    original: BundleEdits,
) -> Box<dyn State> {
    let has_sidewalks = app
        .primary
        .map
        .get_turns_in_intersection(i)
        .any(|t| t.between_sidewalks());

    let use_template = "use template";
    let all_walk = "add an all-walk phase at the end";
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
                    &mut Timer::throwaway(),
                )),
                Box::new(move |new_signal, _, _| {
                    Transition::PopWithData(Box::new(move |state, ctx, app| {
                        let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();

                        let mut bundle = BundleEdits::get_current(app, &editor.members);
                        editor.command_stack.push(bundle.clone());
                        editor.redo_stack.clear();
                        bundle.signals = vec![new_signal];
                        bundle.apply(app);

                        editor.top_panel = make_top_panel(ctx, app, true, false);
                        editor.change_phase(ctx, app, 0);
                    }))
                }),
            )),
            x if x == all_walk => Transition::PopWithData(Box::new(move |state, ctx, app| {
                let mut new_signal = app.primary.map.get_traffic_signal(i).clone();
                if new_signal.convert_to_ped_scramble() {
                    let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();

                    let mut bundle = BundleEdits::get_current(app, &editor.members);
                    editor.command_stack.push(bundle.clone());
                    editor.redo_stack.clear();
                    bundle.signals = vec![new_signal];
                    bundle.apply(app);

                    editor.top_panel = make_top_panel(ctx, app, true, false);
                    editor.change_phase(ctx, app, 0);
                }
            })),
            x if x == stop_sign => {
                original.apply(app);

                let mut edits = app.primary.map.get_edits().clone();
                edits.commands.push(EditCmd::ChangeIntersection {
                    i,
                    old: app.primary.map.get_i_edit(i),
                    new: EditIntersection::StopSign(ControlStopSign::new(&app.primary.map, i)),
                });
                apply_map_edits(ctx, app, edits);
                Transition::PopThenReplace(Box::new(StopSignEditor::new(ctx, app, i, mode.clone())))
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

                    Transition::PopTwice
                }
            }
            x if x == reset => Transition::PopWithData(Box::new(move |state, ctx, app| {
                let editor = state.downcast_mut::<TrafficSignalEditor>().unwrap();

                let mut bundle = BundleEdits::get_current(app, &editor.members);
                editor.command_stack.push(bundle.clone());
                editor.redo_stack.clear();
                let new_signal = ControlTrafficSignal::get_possible_policies(
                    &app.primary.map,
                    i,
                    &mut Timer::throwaway(),
                )
                .remove(0)
                .1;
                bundle.signals = vec![new_signal];
                bundle.apply(app);

                editor.top_panel = make_top_panel(ctx, app, true, false);
                editor.change_phase(ctx, app, 0);
            })),
            _ => unreachable!(),
        }),
    )
}
