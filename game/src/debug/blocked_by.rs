use std::collections::BTreeMap;

use geom::{ArrowCap, Distance, Duration, PolyLine, Pt2D};
use sim::{AgentID, DelayCause};
use widgetry::{
    Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Outcome, Panel,
    State, Text, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::common::CommonState;

/// Visualize the graph of what agents are blocked by others.
pub struct Viewer {
    panel: Panel,
    graph: BTreeMap<AgentID, (Duration, DelayCause)>,
    arrows: Drawable,
}

impl Viewer {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let graph = app.primary.sim.get_blocked_by_graph(&app.primary.map);
        let agent_positions: BTreeMap<AgentID, Pt2D> = app
            .primary
            .sim
            .get_unzoomed_agents(&app.primary.map)
            .into_iter()
            .map(|a| (a.id, a.pos))
            .collect();

        let mut arrows = GeomBatch::new();
        for (id, (_, cause)) in &graph {
            let (to, color) = match cause {
                DelayCause::Agent(a) => {
                    if let Some(pos) = agent_positions.get(a) {
                        (*pos, Color::RED)
                    } else {
                        warn!("{} blocked by {}, but they're gone?", id, a);
                        continue;
                    }
                }
                DelayCause::Intersection(i) => {
                    (app.primary.map.get_i(*i).polygon.center(), Color::BLUE)
                }
            };
            let arrow = PolyLine::must_new(vec![agent_positions[id], to])
                .make_arrow(Distance::meters(0.5), ArrowCap::Triangle);
            arrows.push(color.alpha(0.5), arrow);
        }

        Box::new(Viewer {
            graph,
            arrows: ctx.upload(arrows),
            panel: Panel::new(
                Widget::row(vec![
                    Line("What agents are blocked by others?")
                        .small_heading()
                        .draw(ctx),
                    Btn::close(ctx),
                ]),
                // TODO info about cycles
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
        })
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
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
        g.redraw(&self.arrows);

        if let Some(id) = app
            .primary
            .current_selection
            .as_ref()
            .and_then(|id| id.agent_id())
        {
            if let Some((delay, _)) = self.graph.get(&id) {
                g.draw_mouse_tooltip(Text::from(Line(format!("Waiting {}", delay))));
            }
        }
    }
}
