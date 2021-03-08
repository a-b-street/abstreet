use std::collections::{BTreeMap, HashSet};

use abstutil::Counter;
use geom::{ArrowCap, Circle, Distance, Duration, PolyLine, Polygon, Pt2D};
use map_gui::tools::PopupMsg;
use sim::{AgentID, DelayCause, VehicleType};
use widgetry::{
    Cached, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Outcome,
    Panel, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::common::{warp_to_id, CommonState};

/// Visualize the graph of what agents are blocked by others.
pub struct Viewer {
    panel: Panel,
    graph: BTreeMap<AgentID, (Duration, DelayCause)>,
    agent_positions: BTreeMap<AgentID, Pt2D>,
    arrows: Drawable,

    root_cause: Cached<AgentID, (Drawable, Text)>,
}

impl Viewer {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut viewer = Viewer {
            graph: app.primary.sim.get_blocked_by_graph(&app.primary.map),
            agent_positions: app
                .primary
                .sim
                .get_unzoomed_agents(&app.primary.map)
                .into_iter()
                .map(|a| (a.id, a.pos))
                .collect(),
            arrows: Drawable::empty(ctx),
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("What agents are blocked by others?")
                        .small_heading()
                        .draw(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Text::from(Line("Root causes"))
                    .draw(ctx)
                    .named("root causes"),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),

            root_cause: Cached::new(),
        };

        let mut arrows = GeomBatch::new();
        for id in viewer.agent_positions.keys() {
            if let Some((arrow, color)) = viewer.arrow_for(app, *id) {
                arrows.push(color.alpha(0.5), arrow);
            }
        }
        let (batch, col) = viewer.find_worst_problems(ctx, app);
        arrows.append(batch);
        viewer.panel.replace(ctx, "root causes", col);

        viewer.arrows = ctx.upload(arrows);
        Box::new(viewer)
    }

    fn arrow_for(&self, app: &App, id: AgentID) -> Option<(Polygon, Color)> {
        let (_, cause) = self.graph.get(&id)?;
        let (to, color) = match cause {
            DelayCause::Agent(a) => {
                if let Some(pos) = self.agent_positions.get(a) {
                    (*pos, Color::RED)
                } else {
                    warn!("{} blocked by {}, but they're gone?", id, a);
                    return None;
                }
            }
            DelayCause::Intersection(i) => {
                (app.primary.map.get_i(*i).polygon.center(), Color::BLUE)
            }
        };
        let arrow = PolyLine::must_new(vec![self.agent_positions[&id], to])
            .make_arrow(Distance::meters(0.5), ArrowCap::Triangle);
        Some((arrow, color))
    }

    /// Figure out why some agent is blocked. Draws an arrow for each hop in the dependency chain,
    /// and gives a description of the root cause.
    fn trace_root_cause(&self, app: &App, start: AgentID) -> (GeomBatch, String) {
        let mut batch = GeomBatch::new();
        let mut seen: HashSet<AgentID> = HashSet::new();

        let mut current = start;
        let reason;
        loop {
            if seen.contains(&current) {
                reason = format!("cycle involving {}", current);
                break;
            }
            seen.insert(current);
            if let Some((arrow, _)) = self.arrow_for(app, current) {
                batch.push(Color::CYAN, arrow);
            }
            match self.graph.get(&current) {
                Some((_, DelayCause::Agent(a))) => {
                    current = *a;
                }
                Some((_, DelayCause::Intersection(i))) => {
                    reason = i.to_string();
                    break;
                }
                None => {
                    reason = current.to_string();
                    break;
                }
            }
        }
        (batch, reason)
    }

    /// Trace the root cause for everyone, find the most common sources, highlight them, and
    /// describe them.
    fn find_worst_problems(&self, ctx: &EventCtx, app: &App) -> (GeomBatch, Widget) {
        let mut problems: Counter<DelayCause> = Counter::new();
        for start in self.graph.keys() {
            problems.inc(self.simple_root_cause(*start));
        }

        let mut batch = GeomBatch::new();
        let mut col = vec!["Root causes".draw_text(ctx)];
        for (cause, cnt) in problems.highest_n(3) {
            let pt = match cause {
                DelayCause::Agent(a) => {
                    let warp_id = match a {
                        AgentID::Car(c) => match c.1 {
                            VehicleType::Bike => format!("b{}", c.0),
                            _ => format!("c{}", c.0),
                        },
                        AgentID::Pedestrian(p) => format!("p{}", p.0),
                        // There's always that ONE passenger lugging some inappropriate amount of
                        // furniture, somehow causing gridlock, right?
                        AgentID::BusPassenger(_, c) => format!("c{}", c.0),
                    };
                    col.push(
                        ctx.style()
                            .btn_plain
                            .icon("system/assets/tools/location.svg")
                            .label_text(&format!("{} is blocking {} agents", a, cnt))
                            .build_widget(ctx, &warp_id),
                    );

                    if let Some(pt) = self.agent_positions.get(&a) {
                        *pt
                    } else {
                        continue;
                    }
                }
                DelayCause::Intersection(i) => {
                    col.push(
                        ctx.style()
                            .btn_plain
                            .icon("system/assets/tools/location.svg")
                            .label_text(&format!("{} is blocking {} agents", i, cnt))
                            .build_widget(ctx, &format!("i{}", i.0)),
                    );

                    app.primary.map.get_i(i).polygon.center()
                }
            };
            batch.push(
                Color::YELLOW,
                Circle::new(pt, Distance::meters(5.0))
                    .to_outline(Distance::meters(1.0))
                    .unwrap(),
            );
        }

        (batch, Widget::col(col))
    }

    fn simple_root_cause(&self, start: AgentID) -> DelayCause {
        let mut seen: HashSet<AgentID> = HashSet::new();

        let mut current = start;
        loop {
            if seen.contains(&current) {
                return DelayCause::Agent(current);
            }
            seen.insert(current);
            match self.graph.get(&current) {
                Some((_, DelayCause::Agent(a))) => {
                    current = *a;
                }
                Some((_, DelayCause::Intersection(i))) => {
                    return DelayCause::Intersection(*i);
                }
                None => {
                    return DelayCause::Agent(current);
                }
            }
        }
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);

            // TODO Awkward dances around the borrow checker. Maybe make a method in Cached if we
            // need to do this frequently.
            let mut root_cause = std::mem::replace(&mut self.root_cause, Cached::new());
            root_cause.update(
                app.primary
                    .current_selection
                    .as_ref()
                    .and_then(|id| id.agent_id()),
                |agent| {
                    if let Some((delay, _)) = self.graph.get(&agent) {
                        let (batch, problem) = self.trace_root_cause(app, agent);
                        let txt = Text::from_multiline(vec![
                            Line(format!("Waiting {}", delay)),
                            Line(problem),
                        ]);
                        (ctx.upload(batch), txt)
                    } else {
                        (Drawable::empty(ctx), Text::new())
                    }
                },
            );
            self.root_cause = root_cause;
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                x => {
                    // warp_to_id always replaces the current state, so insert a dummy one for it
                    // to clobber
                    return Transition::Multi(vec![
                        Transition::Push(PopupMsg::new(ctx, "Warping", vec![""])),
                        warp_to_id(ctx, app, x),
                    ]);
                }
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        CommonState::draw_osd(g, app);
        g.redraw(&self.arrows);

        if let Some((draw, txt)) = self.root_cause.value() {
            g.redraw(draw);
            if !txt.is_empty() {
                g.draw_mouse_tooltip(txt.clone());
            }
        }
    }
}
