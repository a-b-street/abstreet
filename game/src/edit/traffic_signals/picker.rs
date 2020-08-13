use crate::app::App;
use crate::common::CommonState;
use crate::edit::TrafficSignalEditor;
use crate::game::{PopupMsg, State, Transition};
use crate::helpers::ID;
use crate::sandbox::gameplay::GameplayMode;
use ezgui::{
    hotkey, hotkeys, Btn, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, VerticalAlignment, Widget,
};
use map_model::IntersectionID;
use std::collections::BTreeSet;

pub struct SignalPicker {
    members: BTreeSet<IntersectionID>,
    composite: Composite,
    mode: GameplayMode,
}

impl SignalPicker {
    pub fn new(
        ctx: &mut EventCtx,
        members: BTreeSet<IntersectionID>,
        mode: GameplayMode,
    ) -> Box<dyn State> {
        Box::new(SignalPicker {
            members,
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Select multiple traffic signals")
                        .small_heading()
                        .draw(ctx),
                    Btn::text_fg("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                // TODO Change label based on number of intersections and disable when 0
                Btn::text_bg2("Continue").build_def(ctx, hotkeys(vec![Key::Enter, Key::E])),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            mode,
        })
    }
}

impl State for SignalPicker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }
        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if app.primary.map.maybe_get_traffic_signal(i).is_some() {
                if !self.members.contains(&i)
                    && app.per_obj.left_click(ctx, "add this intersection")
                {
                    self.members.insert(i);
                } else if self.members.contains(&i)
                    && app.per_obj.left_click(ctx, "remove this intersection")
                {
                    self.members.remove(&i);
                }
            } else {
                app.primary.current_selection = None;
            }
        } else {
            app.primary.current_selection = None;
        }

        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Continue" => {
                    if self.members.is_empty() {
                        return Transition::Push(PopupMsg::new(
                            ctx,
                            "Error",
                            vec!["Select at least one intersection"],
                        ));
                    }
                    return Transition::Replace(TrafficSignalEditor::new(
                        ctx,
                        app,
                        self.members.clone(),
                        self.mode.clone(),
                    ));
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        CommonState::draw_osd(g, app);

        let mut batch = GeomBatch::new();
        for i in &self.members {
            batch.push(
                Color::RED.alpha(0.8),
                app.primary.map.get_i(*i).polygon.clone(),
            );
        }
        let draw = g.upload(batch);
        g.redraw(&draw);
    }
}
