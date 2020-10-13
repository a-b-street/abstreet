use geom::{ArrowCap, Distance, Time};
use map_model::{LaneID, TurnType};
use sim::AgentID;
use widgetry::{
    Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, ShowEverything};
use crate::common::ColorLegend;
use crate::game::{DrawBaselayer, State, Transition};
use crate::render::{DrawOptions, BIG_ARROW_THICKNESS};

pub struct RoutePreview {
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
                if zoomed {
                    if let Some(trace) = app.primary.sim.trace_route(agent, &app.primary.map, None)
                    {
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

pub struct TurnExplorer {
    l: LaneID,
    // 0 means all turns, otherwise one particular turn
    idx: usize,
    panel: Panel,
}

impl TurnExplorer {
    pub fn new(ctx: &mut EventCtx, app: &App, l: LaneID) -> Box<dyn State> {
        Box::new(TurnExplorer {
            l,
            idx: 0,
            panel: TurnExplorer::make_panel(ctx, app, l, 0),
        })
    }
}

impl State for TurnExplorer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "previous turn" => {
                    self.idx -= 1;
                    self.panel = TurnExplorer::make_panel(ctx, app, self.l, self.idx);
                }
                "next turn" => {
                    self.idx += 1;
                    self.panel = TurnExplorer::make_panel(ctx, app, self.l, self.idx);
                }
                _ => unreachable!(),
            },
            _ => {}
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
        app.draw(g, opts, &ShowEverything::new());

        if self.idx == 0 {
            for turn in &app.primary.map.get_turns_from_lane(self.l) {
                g.draw_polygon(
                    color_turn_type(turn.turn_type).alpha(0.5),
                    turn.geom
                        .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle),
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
                    .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle),
            );
            batch.draw(g);
        }

        self.panel.draw(g);
    }
}

impl TurnExplorer {
    fn make_panel(ctx: &mut EventCtx, app: &App, l: LaneID, idx: usize) -> Panel {
        let turns = app.primary.map.get_turns_from_lane(l);

        let mut col = vec![Widget::row(vec![
            Text::from(
                Line(format!(
                    "Turns from {}",
                    app.primary
                        .map
                        .get_parent(l)
                        .get_name(app.opts.language.as_ref())
                ))
                .small_heading(),
            )
            .draw(ctx),
            Widget::vert_separator(ctx, 50.0),
            if idx == 0 {
                Btn::text_fg("<").inactive(ctx)
            } else {
                Btn::text_fg("<").build(ctx, "previous turn", Key::LeftArrow)
            },
            Text::from(Line(format!("{}/{}", idx, turns.len())).secondary())
                .draw(ctx)
                .centered_vert(),
            if idx == turns.len() {
                Btn::text_fg(">").inactive(ctx)
            } else {
                Btn::text_fg(">").build(ctx, "next turn", Key::RightArrow)
            },
            Btn::text_fg("X").build(ctx, "close", Key::Escape),
        ])];
        if idx == 0 {
            if app.primary.map.get_l(l).is_walkable() {
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
            }
        } else {
            let (lt, lc, slow_lane) = turns[idx - 1].penalty(&app.primary.map);
            let (vehicles, bike) = app
                .primary
                .sim
                .target_lane_penalty(app.primary.map.get_l(turns[idx - 1].id.dst));
            col.push(
                format!(
                    "Penalties: {} for lane types, {} for lane changing, {} for keeping to the \
                     slow lane, {} for vehicles, {} for slow bikes",
                    lt, lc, slow_lane, vehicles, bike
                )
                .draw_text(ctx),
            );
            col.push(ColorLegend::row(ctx, CURRENT_TURN, "current turn"));
            col.push(ColorLegend::row(ctx, CONFLICTING_TURN, "conflicting turn"));
        }

        Panel::new(Widget::col(col))
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
        TurnType::Right => Color::GREEN,
        TurnType::Left => Color::RED,
    }
}

const CURRENT_TURN: Color = Color::GREEN;
const CONFLICTING_TURN: Color = Color::RED.alpha(0.8);
