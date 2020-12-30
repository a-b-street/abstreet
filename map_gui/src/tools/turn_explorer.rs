use geom::{ArrowCap, Distance};
use map_model::{LaneID, TurnType};
use widgetry::{
    Btn, Color, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Panel, State, Text, TextExt, Transition, VerticalAlignment, Widget,
};

use crate::render::{DrawOptions, BIG_ARROW_THICKNESS};
use crate::tools::ColorLegend;
use crate::AppLike;

/// A tool to explore all of the turns from a single lane.
pub struct TurnExplorer {
    l: LaneID,
    // 0 means all turns, otherwise one particular turn
    idx: usize,
    panel: Panel,
}

impl TurnExplorer {
    pub fn new<A: AppLike + 'static>(ctx: &mut EventCtx, app: &A, l: LaneID) -> Box<dyn State<A>> {
        Box::new(TurnExplorer {
            l,
            idx: 0,
            panel: TurnExplorer::make_panel(ctx, app, l, 0),
        })
    }
}

impl<A: AppLike + 'static> State<A> for TurnExplorer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
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

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        let mut opts = DrawOptions::new();
        {
            let l = app.map().get_l(self.l);
            opts.suppress_traffic_signal_details.push(l.src_i);
            opts.suppress_traffic_signal_details.push(l.dst_i);
        }
        app.draw_with_opts(g, opts);

        if self.idx == 0 {
            for turn in &app.map().get_turns_from_lane(self.l) {
                g.draw_polygon(
                    TurnExplorer::color_turn_type(turn.turn_type).alpha(0.5),
                    turn.geom
                        .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle),
                );
            }
        } else {
            let current = &app.map().get_turns_from_lane(self.l)[self.idx - 1];

            let mut batch = GeomBatch::new();
            for t in app.map().get_turns_in_intersection(current.id.parent) {
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
    fn make_panel<A: AppLike>(ctx: &mut EventCtx, app: &A, l: LaneID, idx: usize) -> Panel {
        let turns = app.map().get_turns_from_lane(l);

        let mut col = vec![Widget::row(vec![
            Text::from(
                Line(format!(
                    "Turns from {}",
                    app.map()
                        .get_parent(l)
                        .get_name(app.opts().language.as_ref())
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
            Btn::close(ctx),
        ])];
        if idx == 0 {
            if app.map().get_l(l).is_walkable() {
                col.push(ColorLegend::row(
                    ctx,
                    TurnExplorer::color_turn_type(TurnType::Crosswalk),
                    "crosswalk",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    TurnExplorer::color_turn_type(TurnType::SharedSidewalkCorner),
                    "sidewalk connection",
                ));
            } else {
                col.push(ColorLegend::row(
                    ctx,
                    TurnExplorer::color_turn_type(TurnType::Straight),
                    "straight",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    TurnExplorer::color_turn_type(TurnType::Right),
                    "right turn",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    TurnExplorer::color_turn_type(TurnType::Left),
                    "left turn",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    TurnExplorer::color_turn_type(TurnType::UTurn),
                    "U-turn",
                ));
            }
        } else {
            let (lt, lc, slow_lane) = turns[idx - 1].penalty(app.map());
            let (vehicles, bike) = app
                .sim()
                .target_lane_penalty(app.map().get_l(turns[idx - 1].id.dst));
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

    // Since this is extremely localized and probably changing, not going to put this in
    // ColorScheme.
    pub fn color_turn_type(t: TurnType) -> Color {
        match t {
            TurnType::SharedSidewalkCorner => Color::BLACK,
            TurnType::Crosswalk => Color::WHITE,
            TurnType::Straight => Color::BLUE,
            TurnType::Right => Color::GREEN,
            TurnType::Left => Color::RED,
            TurnType::UTurn => Color::PURPLE,
        }
    }
}

const CURRENT_TURN: Color = Color::GREEN;
const CONFLICTING_TURN: Color = Color::RED.alpha(0.8);
