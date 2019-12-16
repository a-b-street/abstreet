use crate::common::route_viewer::RouteViewer;
use crate::common::{RouteExplorer, TripExplorer};
use crate::game::{msg, Transition};
use crate::render::MIN_ZOOM_FOR_DETAIL;
use crate::ui::UI;
use ezgui::{hotkey, EventCtx, GfxCtx, Key, ModalMenu};
use geom::{Pt2D, Time};
use sim::{TripID, TripResult};

pub struct AgentTools {
    following: Option<(TripID, Option<Pt2D>, Time)>,
    route_viewer: RouteViewer,
}

impl AgentTools {
    pub fn new() -> AgentTools {
        AgentTools {
            following: None,
            route_viewer: RouteViewer::Inactive,
        }
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &UI,
        menu: &mut ModalMenu,
    ) -> Option<Transition> {
        if self.following.is_none() {
            if let Some(agent) = ui
                .primary
                .current_selection
                .as_ref()
                .and_then(|id| id.agent_id())
            {
                if let Some(trip) = ui.primary.sim.agent_to_trip(agent) {
                    if ui.per_obj.action(ctx, Key::F, format!("follow {}", agent)) {
                        self.following = Some((
                            trip,
                            ui.primary
                                .sim
                                .get_canonical_pt_per_trip(trip, &ui.primary.map)
                                .ok(),
                            ui.primary.sim.time(),
                        ));
                        menu.push_action(hotkey(Key::F), "stop following agent", ctx);
                    }
                }
            }
        }
        if let Some((trip, _, time)) = self.following {
            if ui.primary.sim.time() != time {
                match ui
                    .primary
                    .sim
                    .get_canonical_pt_per_trip(trip, &ui.primary.map)
                {
                    TripResult::Ok(pt) => {
                        ctx.canvas.center_on_map_pt(pt);
                        self.following = Some((trip, Some(pt), ui.primary.sim.time()));
                    }
                    TripResult::ModeChange => {
                        self.following = Some((trip, None, ui.primary.sim.time()));
                    }
                    TripResult::TripDone => {
                        self.following = None;
                        menu.remove_action("stop following agent", ctx);
                        return Some(Transition::Push(msg(
                            "Follower",
                            vec![format!("{} is done or aborted, so no more following", trip)],
                        )));
                    }
                    TripResult::TripDoesntExist => {
                        println!("{} doesn't exist yet, so not following", trip);
                        self.following = None;
                        menu.remove_action("stop following agent", ctx);
                    }
                }
            }
            if self.following.is_some() && menu.consume_action("stop following agent", ctx) {
                self.following = None;
            }
        }
        self.route_viewer.event(ctx, ui, menu);

        if let Some(explorer) = RouteExplorer::new(ctx, ui) {
            return Some(Transition::Push(Box::new(explorer)));
        }
        if let Some(trip) = ui
            .primary
            .current_selection
            .as_ref()
            .and_then(|id| id.agent_id())
            .and_then(|agent| ui.primary.sim.agent_to_trip(agent))
        {
            if ui.per_obj.action(ctx, Key::T, format!("explore {}", trip)) {
                return Some(Transition::Push(Box::new(TripExplorer::new(trip, ctx, ui))));
            }
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.route_viewer.draw(g);

        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            ui.agent_cs_legend.draw(g);
        }
    }
}
