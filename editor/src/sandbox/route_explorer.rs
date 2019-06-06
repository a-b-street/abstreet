use crate::common::{CommonState, Warper};
use crate::helpers::ID;
use crate::render::DrawTurn;
use crate::ui::{ShowEverything, UI};
use ezgui::{hotkey, Color, EventCtx, EventLoopMode, GfxCtx, ItemSlider, Key, Text};
use geom::{Distance, Polygon};
use map_model::{Traversable, LANE_THICKNESS};
use sim::AgentID;

pub struct RouteExplorer {
    slider: ItemSlider<Traversable>,
    agent: AgentID,
    entire_trace: Option<Polygon>,
    warper: Option<Warper>,
}

impl RouteExplorer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<RouteExplorer> {
        let (agent, path) = if true {
            let agent = ui.primary.current_selection.and_then(|id| id.agent_id())?;
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

        let steps: Vec<Traversable> = path
            .get_steps()
            .iter()
            .map(|step| step.as_traversable())
            .collect();
        Some(RouteExplorer {
            agent,
            warper: Some(Warper::new(
                ctx,
                steps[0]
                    .dist_along(steps[0].length(&ui.primary.map) / 2.0, &ui.primary.map)
                    .0,
                match steps[0] {
                    Traversable::Lane(l) => ID::Lane(l),
                    Traversable::Turn(t) => ID::Turn(t),
                },
            )),
            slider: ItemSlider::new(
                steps,
                "Route Explorer",
                "step",
                vec![(hotkey(Key::Escape), "quit")],
                ctx,
            ),
            entire_trace,
        })
    }

    // Done when None
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<EventLoopMode> {
        // Don't block while we're warping
        let ev_mode = if let Some(ref warper) = self.warper {
            if let Some(mode) = warper.event(ctx, ui) {
                mode
            } else {
                self.warper = None;
                EventLoopMode::InputOnly
            }
        } else {
            EventLoopMode::InputOnly
        };

        let (idx, step) = self.slider.get();
        let mut txt = Text::prompt(&format!("Route Explorer for {:?}", self.agent));
        txt.add_line(format!("Step {}/{}", idx + 1, self.slider.len()));
        txt.add_line(format!("{:?}", step));
        let changed = self.slider.event(ctx, Some(txt));
        ctx.canvas.handle_event(ctx.input);
        if ctx.redo_mouseover() {
            ui.primary.current_selection = ui.recalculate_current_selection(
                ctx,
                &ui.primary.sim,
                &ShowEverything::new(),
                false,
            );
        }

        if self.slider.action("quit") {
            return None;
        } else if !changed {
            return Some(ev_mode);
        }

        let (_, step) = self.slider.get();
        self.warper = Some(Warper::new(
            ctx,
            step.dist_along(step.length(&ui.primary.map) / 2.0, &ui.primary.map)
                .0,
            match step {
                Traversable::Lane(l) => ID::Lane(*l),
                Traversable::Turn(t) => ID::Turn(*t),
            },
        ));
        // We just created a new warper, so...
        Some(EventLoopMode::Animation)
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
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
        CommonState::draw_osd(g, ui, ui.primary.current_selection);
    }
}
