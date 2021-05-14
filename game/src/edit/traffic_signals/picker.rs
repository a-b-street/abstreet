use std::collections::BTreeSet;

use map_gui::ID;
use map_model::IntersectionID;
use widgetry::{
    hotkeys, Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    State, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::common::CommonState;
use crate::edit::TrafficSignalEditor;
use crate::sandbox::gameplay::GameplayMode;

pub struct SignalPicker {
    members: BTreeSet<IntersectionID>,
    panel: Panel,
    mode: GameplayMode,
}

impl SignalPicker {
    pub fn new_state(
        ctx: &mut EventCtx,
        members: BTreeSet<IntersectionID>,
        mode: GameplayMode,
    ) -> Box<dyn State<App>> {
        Box::new(SignalPicker {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Select multiple traffic signals")
                        .small_heading()
                        .into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
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
            app.primary.current_selection =
                app.mouseover_unzoomed_intersections(ctx).filter(|id| {
                    app.primary
                        .map
                        .maybe_get_traffic_signal(id.as_intersection())
                        .is_some()
                });
        }
        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if !self.members.contains(&i) && app.per_obj.left_click(ctx, "add this intersection") {
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
        }

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "edit" => {
                    return Transition::Replace(TrafficSignalEditor::new_state(
                        ctx,
                        app,
                        self.members.clone(),
                        self.mode.clone(),
                    ));
                }
                _ => unreachable!(),
            }
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
    let title = match num {
        0 => "Edit 0 signals".to_string(),
        1 => "Edit 1 signal".to_string(),
        _ => format!("Edit {} signals", num),
    };
    ctx.style()
        .btn_solid_primary
        .text(title)
        .disabled(num == 0)
        .hotkey(hotkeys(vec![Key::Enter, Key::E]))
        .build_widget(ctx, "edit")
}
