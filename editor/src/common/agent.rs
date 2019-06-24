use crate::common::route_viewer::RouteViewer;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, Key, ModalMenu, Text};
use sim::TripID;

pub struct AgentTools {
    following: Option<TripID>,
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
        if let Some(trip) = self.following {
            txt.add_line(format!("Following {}", trip));
        }
        match self.route_viewer {
            RouteViewer::Active(_, trip, _) => {
                txt.add_line(format!("Showing {}'s route", trip));
            }
            RouteViewer::DebugAllRoutes(_, _) => {
                txt.add_line("Showing all routes".to_string());
            }
            _ => {}
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI, menu: &mut ModalMenu) {
        if self.following.is_none() {
            if let Some(agent) = ui.primary.current_selection.and_then(|id| id.agent_id()) {
                if let Some(trip) = ui.primary.sim.agent_to_trip(agent) {
                    if ctx
                        .input
                        .contextual_action(Key::F, &format!("follow {}", agent))
                    {
                        self.following = Some(trip);
                    }
                }
            }
        }
        if let Some(trip) = self.following {
            if let Some(pt) = ui
                .primary
                .sim
                .get_canonical_pt_per_trip(trip, &ui.primary.map)
            {
                ctx.canvas.center_on_map_pt(pt);
            } else {
                // TODO ideally they wouldnt vanish for so long according to
                // get_canonical_point_for_trip
                println!("{} is gone... temporarily or not?", trip);
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
