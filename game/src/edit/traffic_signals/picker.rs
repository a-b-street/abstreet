use std::collections::BTreeSet;

use map_model::IntersectionID;
use widgetry::{
    hotkeys, Btn, Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, State, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::common::CommonState;
use crate::edit::TrafficSignalEditor;
use crate::game::Transition;
use crate::helpers::ID;
use crate::sandbox::gameplay::GameplayMode;

pub struct SignalPicker {
    members: BTreeSet<IntersectionID>,
    panel: Panel,
    mode: GameplayMode,
}

impl SignalPicker {
    pub fn new(
        ctx: &mut EventCtx,
        members: BTreeSet<IntersectionID>,
        mode: GameplayMode,
    ) -> Box<dyn State<App>> {
        Box::new(SignalPicker {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Select multiple traffic signals")
                        .small_heading()
                        .draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", Key::Escape)
                        .align_right(),
                ]),
                make_btn(ctx, members.len()),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            members,
            mode,
        })
    }
}

impl State<App> for SignalPicker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.mouseover_unzoomed_roads_and_intersections(ctx);
        }
        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if app.primary.map.maybe_get_traffic_signal(i).is_some() {
                if !self.members.contains(&i)
                    && app.per_obj.left_click(ctx, "add this intersection")
                {
                    self.members.insert(i);
                    let btn = make_btn(ctx, self.members.len());
                    self.panel.replace(ctx, "edit", btn);
                } else if self.members.contains(&i)
                    && app.per_obj.left_click(ctx, "remove this intersection")
                {
                    self.members.remove(&i);
                    let btn = make_btn(ctx, self.members.len());
                    self.panel.replace(ctx, "edit", btn);
                }
            } else {
                app.primary.current_selection = None;
            }
        } else {
            app.primary.current_selection = None;
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "edit" => {
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
        self.panel.draw(g);
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

fn make_btn(ctx: &mut EventCtx, num: usize) -> Widget {
    if num == 0 {
        return Btn::text_bg2("Edit 0 signals").inactive(ctx).named("edit");
    }

    let title = if num == 1 {
        "Edit 1 signal".to_string()
    } else {
        format!("Edit {} signals", num)
    };
    Btn::text_bg2(title).build(ctx, "edit", hotkeys(vec![Key::Enter, Key::E]))
}
