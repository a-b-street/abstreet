use crate::common::route_viewer::RouteViewer;
use crate::common::ColorLegend;
use crate::game::{Transition, WizardState};
use crate::render::{AgentColorScheme, MIN_ZOOM_FOR_DETAIL};
use crate::ui::UI;
use ezgui::{Choice, EventCtx, GfxCtx, Key, Line, ModalMenu, Text};
use geom::{Duration, Pt2D};
use sim::{TripID, TripResult};
use std::cell::RefCell;

pub struct AgentTools {
    following: Option<(TripID, Option<Pt2D>, Duration)>,
    route_viewer: RouteViewer,
    // Weird to stash this here and lazily sync it, but...
    agent_cs_legend: RefCell<Option<(AgentColorScheme, ColorLegend)>>,
}

impl AgentTools {
    pub fn new() -> AgentTools {
        AgentTools {
            following: None,
            route_viewer: RouteViewer::Inactive,
            agent_cs_legend: RefCell::new(None),
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
                    if ctx
                        .input
                        .contextual_action(Key::F, format!("follow {}", agent))
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
                    TripResult::TripDoesntExist => {
                        println!("{} doesn't exist yet, so not following", trip);
                        self.following = None;
                    }
                }
            }
            if menu.action("stop following agent") {
                self.following = None;
            }
        }
        self.route_viewer.event(ctx, ui, menu);

        if menu.action("change agent colorscheme") {
            return Some(Transition::Push(WizardState::new(Box::new(
                |wiz, ctx, ui| {
                    let (_, acs) = wiz.wrap(ctx).choose("Which colorscheme for agents?", || {
                        let mut choices = Vec::new();
                        for (acs, name) in AgentColorScheme::all() {
                            if ui.agent_cs != acs {
                                choices.push(Choice::new(name, acs));
                            }
                        }
                        choices
                    })?;
                    ui.agent_cs = acs;
                    ui.primary.draw_map.agents.borrow_mut().invalidate_cache();
                    if let Some(ref mut s) = ui.secondary {
                        s.draw_map.agents.borrow_mut().invalidate_cache();
                    }
                    Some(Transition::Pop)
                },
            ))));
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.route_viewer.draw(g, ui);

        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            let mut maybe_legend = self.agent_cs_legend.borrow_mut();
            if maybe_legend
                .as_ref()
                .map(|(acs, _)| *acs != ui.agent_cs)
                .unwrap_or(true)
            {
                *maybe_legend = Some((ui.agent_cs, ui.agent_cs.make_color_legend(&ui.cs)));
            }
            maybe_legend.as_ref().unwrap().1.draw(g);
        }
    }
}
