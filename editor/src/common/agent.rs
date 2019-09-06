use crate::common::route_viewer::RouteViewer;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, Key, Line, ModalMenu, Text};
use geom::{Duration, Pt2D};
use sim::{TripID, TripResult};

pub struct AgentTools {
    following: Option<(TripID, Option<Pt2D>, Duration)>,
    route_viewer: RouteViewer,
}

impl AgentTools {
    pub fn new() -> AgentTools {
        AgentTools {
            following: None,
            route_viewer: RouteViewer::Inactive,
        }
    }

    pub fn update_menu_info(&self, txt: &mut Text) {
        if let Some((trip, _, _)) = self.following {
            txt.add(Line(format!("Following {}", trip)));
        }
        if let RouteViewer::Active(_, trip, _) = self.route_viewer {
            txt.add(Line(format!("Showing {}'s route", trip)));
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI, menu: &mut ModalMenu) {
        if self.following.is_none() {
            if let Some(agent) = ui
                .primary
                .current_selection
                .as_ref()
                .and_then(|id| id.agent_id())
            {
                if let Some(trip) = ui.primary.sim.agent_to_trip(agent) {
                    if ctx
                        .input
                        .contextual_action(Key::F, &format!("follow {}", agent))
                    {
                        self.following = Some((
                            trip,
                            ui.primary
                                .sim
                                .get_canonical_pt_per_trip(trip, &ui.primary.map)
                                .ok(),
                            ui.primary.sim.time(),
                        ));
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
                        println!("{} is done or aborted, so no more following", trip);
                        self.following = None;
                    }
                }
            }
            if menu.action("stop following agent") {
                self.following = None;
            }
        }
        self.route_viewer.event(ctx, ui, menu);
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.route_viewer.draw(g, ui);
    }
}
