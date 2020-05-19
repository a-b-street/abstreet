use crate::app::{App, ShowEverything};
use crate::common::ColorLegend;
use crate::game::{DrawBaselayer, State, Transition};
use crate::render::{draw_signal_phase, make_signal_diagram, DrawOptions, BIG_ARROW_THICKNESS};
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Text, VerticalAlignment, Widget,
};
use geom::{ArrowCap, Distance, Polygon, Time};
use map_model::{IntersectionID, LaneID, TurnType};
use sim::{AgentID, DontDrawAgents};

pub struct RoutePreview {
    preview: Option<(AgentID, Time, Drawable)>,
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
            if self
                .preview
                .as_ref()
                .map(|(a, t, _)| agent != *a || now != *t)
                .unwrap_or(true)
            {
                if let Some(trace) = app.primary.sim.trace_route(agent, &app.primary.map, None) {
                    let mut batch = GeomBatch::new();
                    batch.extend(
                        app.cs.route,
                        trace.dashed_lines(
                            Distance::meters(0.75),
                            Distance::meters(1.0),
                            Distance::meters(0.4),
                        ),
                    );
                    self.preview = Some((agent, now, batch.upload(ctx)));
                }
            }
            return None;
        }
        self.preview = None;

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let Some((_, _, ref d)) = self.preview {
            g.redraw(d);
        }
    }
}

pub struct ShowTrafficSignal {
    i: IntersectionID,
    composite: Composite,
    current_phase: usize,
}

impl ShowTrafficSignal {
    pub fn new(ctx: &mut EventCtx, app: &App, i: IntersectionID) -> Box<dyn State> {
        let (idx, _, _) = app
            .primary
            .map
            .get_traffic_signal(i)
            .current_phase_and_remaining_time(app.primary.sim.time());
        return Box::new(ShowTrafficSignal {
            i,
            composite: make_signal_diagram(ctx, app, i, idx, false),
            current_phase: idx,
        });
    }

    fn change_phase(&mut self, idx: usize, app: &App, ctx: &mut EventCtx) {
        if self.current_phase != idx {
            self.current_phase = idx;
            self.composite = make_signal_diagram(ctx, app, self.i, self.current_phase, false);
            self.composite
                .scroll_to_member(ctx, format!("phase {}", idx + 1));
        }
    }
}

impl State for ShowTrafficSignal {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        // TODO Buttons for these...
        if self.current_phase != 0 && ctx.input.new_was_pressed(&hotkey(Key::UpArrow).unwrap()) {
            self.change_phase(self.current_phase - 1, app, ctx);
        }

        if self.current_phase != app.primary.map.get_traffic_signal(self.i).phases.len() - 1
            && ctx.input.new_was_pressed(&hotkey(Key::DownArrow).unwrap())
        {
            self.change_phase(self.current_phase + 1, app, ctx);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => {
                    self.change_phase(x["phase ".len()..].parse::<usize>().unwrap() - 1, app, ctx);
                }
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let mut opts = DrawOptions::new();
        opts.suppress_traffic_signal_details.push(self.i);
        app.draw(g, opts, &DontDrawAgents {}, &ShowEverything::new());
        let mut batch = GeomBatch::new();
        draw_signal_phase(
            g.prerender,
            &app.primary.map.get_traffic_signal(self.i).phases[self.current_phase],
            self.i,
            None,
            &mut batch,
            app,
            app.opts.traffic_signal_style.clone(),
        );
        batch.draw(g);

        self.composite.draw(g);
    }
}

pub struct TurnExplorer {
    l: LaneID,
    // 0 means all turns, otherwise one particular turn
    idx: usize,
    composite: Composite,
}

impl TurnExplorer {
    pub fn new(ctx: &mut EventCtx, app: &App, l: LaneID) -> Box<dyn State> {
        Box::new(TurnExplorer {
            l,
            idx: 0,
            composite: TurnExplorer::make_panel(ctx, app, l, 0),
        })
    }
}

impl State for TurnExplorer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "previous turn" => {
                    self.idx -= 1;
                    self.composite = TurnExplorer::make_panel(ctx, app, self.l, self.idx);
                }
                "next turn" => {
                    self.idx += 1;
                    self.composite = TurnExplorer::make_panel(ctx, app, self.l, self.idx);
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let mut opts = DrawOptions::new();
        {
            let l = app.primary.map.get_l(self.l);
            opts.suppress_traffic_signal_details.push(l.src_i);
            opts.suppress_traffic_signal_details.push(l.dst_i);
        }
        app.draw(g, opts, &DontDrawAgents {}, &ShowEverything::new());

        if self.idx == 0 {
            for turn in &app.primary.map.get_turns_from_lane(self.l) {
                g.draw_polygon(
                    color_turn_type(turn.turn_type).alpha(0.5),
                    &turn
                        .geom
                        .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle)
                        .unwrap(),
                );
            }
        } else {
            let current = &app.primary.map.get_turns_from_lane(self.l)[self.idx - 1];

            let mut batch = GeomBatch::new();
            for t in app.primary.map.get_turns_in_intersection(current.id.parent) {
                if current.conflicts_with(t) {
                    batch.extend(
                        CONFLICTING_TURN,
                        t.geom.dashed_arrow(
                            BIG_ARROW_THICKNESS,
                            Distance::meters(1.0),
                            Distance::meters(0.5),
                            ArrowCap::Triangle,
                        ),
                    );
                }
            }
            batch.push(
                CURRENT_TURN,
                current
                    .geom
                    .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle)
                    .unwrap(),
            );
            batch.draw(g);
        }

        self.composite.draw(g);
    }
}

impl TurnExplorer {
    fn make_panel(ctx: &mut EventCtx, app: &App, l: LaneID, idx: usize) -> Composite {
        let num_turns = app.primary.map.get_turns_from_lane(l).len();

        let mut col = vec![Widget::row(vec![
            Text::from(
                Line(format!(
                    "Turns from {}",
                    app.primary.map.get_parent(l).get_name()
                ))
                .small_heading(),
            )
            .draw(ctx)
            .margin(5),
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
            )
            .margin(5),
            if idx == 0 {
                Btn::text_fg("<").inactive(ctx)
            } else {
                Btn::text_fg("<").build(ctx, "previous turn", hotkey(Key::LeftArrow))
            }
            .margin(5),
            Text::from(Line(format!("{}/{}", idx, num_turns)).secondary())
                .draw(ctx)
                .margin(5)
                .centered_vert(),
            if idx == num_turns {
                Btn::text_fg(">").inactive(ctx)
            } else {
                Btn::text_fg(">").build(ctx, "next turn", hotkey(Key::RightArrow))
            }
            .margin(5),
            Btn::text_fg("X").build_def(ctx, hotkey(Key::Escape)),
        ])];
        if idx == 0 {
            if app.primary.map.get_l(l).is_sidewalk() {
                col.push(ColorLegend::row(
                    ctx,
                    color_turn_type(TurnType::Crosswalk),
                    "crosswalk",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    color_turn_type(TurnType::SharedSidewalkCorner),
                    "sidewalk connection",
                ));
            } else {
                col.push(ColorLegend::row(
                    ctx,
                    color_turn_type(TurnType::Straight),
                    "straight",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    color_turn_type(TurnType::Right),
                    "right turn",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    color_turn_type(TurnType::Left),
                    "left turn",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    color_turn_type(TurnType::LaneChangeLeft),
                    "straight, but lane-change left",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    color_turn_type(TurnType::LaneChangeRight),
                    "straight, but lane-change right",
                ));
            }
        } else {
            col.push(ColorLegend::row(ctx, CURRENT_TURN, "current turn"));
            col.push(ColorLegend::row(ctx, CONFLICTING_TURN, "conflicting turn"));
        }

        Composite::new(Widget::col(col).bg(app.cs.panel_bg))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx)
    }
}

// Since this is extremely localized and probably changing, not going to put this in ColorScheme.
fn color_turn_type(t: TurnType) -> Color {
    match t {
        TurnType::SharedSidewalkCorner => Color::BLACK,
        TurnType::Crosswalk => Color::WHITE,
        TurnType::Straight => Color::BLUE,
        TurnType::LaneChangeLeft => Color::CYAN,
        TurnType::LaneChangeRight => Color::PURPLE,
        TurnType::Right => Color::GREEN,
        TurnType::Left => Color::RED,
    }
}

const CURRENT_TURN: Color = Color::GREEN;
const CONFLICTING_TURN: Color = Color::RED.alpha(0.8);
