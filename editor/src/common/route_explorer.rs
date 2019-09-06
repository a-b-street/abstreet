use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::render::DrawTurn;
use crate::ui::UI;
use ezgui::{Color, EventCtx, GfxCtx, Key, Line, Text, WarpingItemSlider};
use geom::{Distance, Polygon, Pt2D};
use map_model::{Traversable, LANE_THICKNESS};
use sim::AgentID;

pub struct RouteExplorer {
    slider: WarpingItemSlider<Traversable>,
    entire_trace: Option<Polygon>,
}

impl RouteExplorer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<RouteExplorer> {
        let (agent, path) = if true {
            let agent = ui
                .primary
                .current_selection
                .as_ref()
                .and_then(|id| id.agent_id())?;
            (agent, ui.primary.sim.get_path(agent)?.clone())
        } else {
            use map_model::{LaneID, PathRequest, Position};

            // TODO Temporary for debugging
            let agent = AgentID::Pedestrian(sim::PedestrianID(42));
            let path = ui.primary.map.pathfind(PathRequest {
                start: Position::new(LaneID(4409), Distance::meters(146.9885)),
                end: Position::new(LaneID(8188), Distance::meters(82.4241)),
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            });
            (agent, path?)
        };

        if !ctx.input.contextual_action(Key::E, "explore route") {
            return None;
        }

        // TODO Actual start dist
        let entire_trace = path
            .trace(&ui.primary.map, Distance::ZERO, None)
            .map(|pl| pl.make_polygons(LANE_THICKNESS));

        let steps: Vec<(Pt2D, Traversable, Text)> = path
            .get_steps()
            .iter()
            .map(|step| {
                let t = step.as_traversable();
                (
                    t.dist_along(t.length(&ui.primary.map) / 2.0, &ui.primary.map)
                        .0,
                    t,
                    Text::from(Line(format!("{:?}", t))),
                )
            })
            .collect();
        Some(RouteExplorer {
            slider: WarpingItemSlider::new(
                steps,
                &format!("Route Explorer for {}", agent),
                "step",
                ctx,
            ),
            entire_trace,
        })
    }
}

impl State for RouteExplorer {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        ctx.canvas.handle_event(ctx.input);

        // We don't really care about setting current_selection to the current step; drawing covers
        // it up anyway.
        if let Some((evmode, _)) = self.slider.event(ctx) {
            Transition::KeepWithMode(evmode)
        } else {
            Transition::Pop
        }
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if let Some(ref poly) = self.entire_trace {
            g.draw_polygon(ui.cs.get_def("entire route", Color::BLUE.alpha(0.2)), poly);
        }

        let color = ui.cs.get_def("current step", Color::RED);
        match self.slider.get().1 {
            Traversable::Lane(l) => {
                g.draw_polygon(color, &ui.primary.draw_map.get_l(*l).polygon);
            }
            Traversable::Turn(t) => {
                DrawTurn::draw_full(ui.primary.map.get_t(*t), g, color);
            }
        }
        self.slider.draw(g);
        CommonState::draw_osd(g, ui, &ui.primary.current_selection);
    }
}
