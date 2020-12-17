use std::collections::BTreeSet;

use geom::{Distance, Time};
use map_gui::ID;
use map_model::IntersectionID;
use sim::AgentID;
use widgetry::{
    Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, State, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::CommonState;

/// Draws a preview of the path for the agent under the mouse cursor.
pub struct RoutePreview {
    // (the agent we're hovering on, the sim time, whether we're zoomed in, the drawn path)
    preview: Option<(AgentID, Time, bool, Drawable)>,
}

impl RoutePreview {
    pub fn new() -> RoutePreview {
        RoutePreview { preview: None }
    }
}

impl RoutePreview {
    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
        if let Some(agent) = app
            .primary
            .current_selection
            .as_ref()
            .and_then(|id| id.agent_id())
        {
            let now = app.primary.sim.time();
            let zoomed = ctx.canvas.cam_zoom >= app.opts.min_zoom_for_detail;
            if self
                .preview
                .as_ref()
                .map(|(a, t, z, _)| agent != *a || now != *t || zoomed != *z)
                .unwrap_or(true)
            {
                let mut batch = GeomBatch::new();
                // Only draw the preview when zoomed in. If we wanted to do this unzoomed, we'd
                // want a different style; the dashed lines don't show up well.
                if zoomed {
                    if let Some(trace) = app.primary.sim.trace_route(agent, &app.primary.map) {
                        batch.extend(
                            app.cs.route,
                            trace.dashed_lines(
                                Distance::meters(0.75),
                                Distance::meters(1.0),
                                Distance::meters(0.4),
                            ),
                        );
                    }
                }
                self.preview = Some((agent, now, zoomed, batch.upload(ctx)));
            }
            return None;
        }
        self.preview = None;

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let Some((_, _, _, ref d)) = self.preview {
            g.redraw(d);
        }
    }
}

// TODO Refactor with SignalPicker
pub struct TrafficRecorder {
    members: BTreeSet<IntersectionID>,
    panel: Panel,
}

impl TrafficRecorder {
    pub fn new(ctx: &mut EventCtx, members: BTreeSet<IntersectionID>) -> Box<dyn State<App>> {
        Box::new(TrafficRecorder {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Select the bounding intersections for recording traffic")
                        .small_heading()
                        .draw(ctx),
                    Btn::close(ctx),
                ]),
                make_btn(ctx, members.len()),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            members,
        })
    }
}

impl State<App> for TrafficRecorder {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.mouseover_unzoomed_intersections(ctx);
        }
        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if !self.members.contains(&i) && app.per_obj.left_click(ctx, "add this intersection") {
                self.members.insert(i);
                let btn = make_btn(ctx, self.members.len());
                self.panel.replace(ctx, "record", btn);
            } else if self.members.contains(&i)
                && app.per_obj.left_click(ctx, "remove this intersection")
            {
                self.members.remove(&i);
                let btn = make_btn(ctx, self.members.len());
                self.panel.replace(ctx, "record", btn);
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "record" => {
                    app.primary.sim.record_traffic_for(self.members.clone());
                    return Transition::Pop;
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
        return Btn::text_bg2("Record 0 intersections")
            .inactive(ctx)
            .named("record");
    }

    let title = if num == 1 {
        "Record 1 intersection".to_string()
    } else {
        format!("Record {} intersections", num)
    };
    Btn::text_bg2(title).build(ctx, "record", Key::Enter)
}
