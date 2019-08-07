mod agent;
mod associated;
mod navigate;
mod route_explorer;
mod route_viewer;
mod shortcuts;
mod speed;
mod time;
mod trip_explorer;
mod turn_cycler;
mod warp;

pub use self::agent::AgentTools;
pub use self::route_explorer::RouteExplorer;
pub use self::speed::SpeedControls;
pub use self::time::time_controls;
pub use self::trip_explorer::TripExplorer;
use crate::game::Transition;
use crate::helpers::ID;
use crate::render::{DrawOptions, MIN_ZOOM_FOR_DETAIL};
use crate::ui::UI;
use ezgui::{
    Color, EventCtx, EventLoopMode, GeomBatch, GfxCtx, HorizontalAlignment, ModalMenu, Text,
    VerticalAlignment,
};
use geom::{Circle, Distance, Duration};
use std::collections::BTreeSet;

pub struct CommonState {
    associated: associated::ShowAssociatedState,
    turn_cycler: turn_cycler::TurnCyclerState,
    // TODO Have a more general colorscheme that can be changed and affect everything. Show a
    // little legend when it's first activated.
    show_delayed_agents: bool,
}

impl CommonState {
    pub fn new() -> CommonState {
        CommonState {
            associated: associated::ShowAssociatedState::Inactive,
            turn_cycler: turn_cycler::TurnCyclerState::Inactive,
            show_delayed_agents: false,
        }
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        menu: &mut ModalMenu,
    ) -> Option<Transition> {
        if menu.action("warp") {
            return Some(Transition::Push(warp::EnteringWarp::new()));
        }
        if menu.action("navigate") {
            return Some(Transition::Push(Box::new(navigate::Navigator::new(ui))));
        }
        if menu.action("shortcuts") {
            return Some(Transition::Push(shortcuts::ChoosingShortcut::new()));
        }
        // TODO But it's too late to influence the menu's text to say if this is active or not.
        // This kind of belongs in AgentTools, except that can't influence DrawOptions as easily.
        if menu.action("show/hide delayed traffic") {
            self.show_delayed_agents = !self.show_delayed_agents;
        }

        self.associated.event(ui);
        if let Some(t) = self.turn_cycler.event(ctx, ui) {
            return Some(t);
        }
        if menu.action("take a screenshot") {
            return Some(Transition::KeepWithMode(
                EventLoopMode::ScreenCaptureCurrentShot,
            ));
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.turn_cycler.draw(g, ui);

        if self.show_delayed_agents && g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            let mut batch = GeomBatch::new();
            let radius = Distance::meters(10.0) / g.canvas.cam_zoom;
            for agent in ui
                .primary
                .sim
                .get_unzoomed_agents_with_delay(&ui.primary.map)
            {
                batch.push(
                    delay_color(agent.time_since_last_turn),
                    Circle::new(agent.pos, radius).to_polygon(),
                );
            }
            batch.draw(g);
        }

        CommonState::draw_osd(g, ui, ui.primary.current_selection);
    }

    pub fn draw_osd(g: &mut GfxCtx, ui: &UI, id: Option<ID>) {
        let map = &ui.primary.map;
        let id_color = ui.cs.get_def("OSD ID color", Color::RED);
        let name_color = ui.cs.get_def("OSD name color", Color::CYAN);
        let mut osd = Text::new();
        match id {
            None => {
                osd.append("...".to_string(), None);
            }
            Some(ID::Lane(l)) => {
                osd.append(format!("{}", l), Some(id_color));
                osd.append(" is ".to_string(), None);
                osd.append(map.get_parent(l).get_name(), Some(name_color));
            }
            Some(ID::Building(b)) => {
                osd.append(format!("{}", b), Some(id_color));
                osd.append(" is ".to_string(), None);
                osd.append(map.get_b(b).get_name(), Some(name_color));
            }
            Some(ID::Turn(t)) => {
                osd.append(
                    format!("TurnID({})", map.get_t(t).lookup_idx),
                    Some(id_color),
                );
                osd.append(" between ".to_string(), None);
                osd.append(map.get_parent(t.src).get_name(), Some(name_color));
                osd.append(" and ".to_string(), None);
                osd.append(map.get_parent(t.dst).get_name(), Some(name_color));
            }
            Some(ID::Intersection(i)) => {
                osd.append(format!("{}", i), Some(id_color));
                osd.append(" of ".to_string(), None);

                let mut road_names = BTreeSet::new();
                for r in &map.get_i(i).roads {
                    road_names.insert(map.get_r(*r).get_name());
                }
                let len = road_names.len();
                for (idx, n) in road_names.into_iter().enumerate() {
                    osd.append(n, Some(name_color));
                    if idx != len - 1 {
                        osd.append(", ".to_string(), None);
                    }
                }
            }
            Some(ID::Car(c)) => {
                osd.append(format!("{}", c), Some(id_color));
                if let Some(r) = ui.primary.sim.bus_route_name(c) {
                    osd.append(" serving ".to_string(), None);
                    osd.append(map.get_br(r).name.to_string(), Some(name_color));
                }
            }
            Some(ID::BusStop(bs)) => {
                osd.append(format!("{}", bs), Some(id_color));
                osd.append(" serving ".to_string(), None);

                let routes = map.get_routes_serving_stop(bs);
                let len = routes.len();
                for (idx, n) in routes.into_iter().enumerate() {
                    osd.append(n.name.clone(), Some(name_color));
                    if idx != len - 1 {
                        osd.append(", ".to_string(), None);
                    }
                }
            }
            Some(id) => {
                osd.append(format!("{:?}", id), Some(id_color));
            }
        }
        CommonState::draw_custom_osd(g, osd);
    }

    pub fn draw_custom_osd(g: &mut GfxCtx, mut osd: Text) {
        let keys = g.get_active_context_menu_keys();
        if !keys.is_empty() {
            osd.append("   Hotkeys: ".to_string(), None);
            for (idx, key) in keys.into_iter().enumerate() {
                if idx != 0 {
                    osd.append(", ".to_string(), None);
                }
                osd.append(key.describe(), Some(ezgui::HOTKEY_COLOR));
            }
        }

        g.draw_blocking_text(
            &osd,
            (HorizontalAlignment::FillScreen, VerticalAlignment::Bottom),
        );
    }

    pub fn draw_options(&self, ui: &UI) -> DrawOptions {
        let mut opts = DrawOptions::new();
        self.associated
            .override_colors(&mut opts.override_colors, ui);
        opts.suppress_traffic_signal_details = self
            .turn_cycler
            .suppress_traffic_signal_details(&ui.primary.map);
        opts.suppress_unzoomed_agents = self.show_delayed_agents;
        opts
    }
}

fn delay_color(delay: Duration) -> Color {
    // TODO Better gradient
    if delay <= Duration::minutes(1) {
        return Color::BLUE.alpha(0.3);
    }
    if delay <= Duration::minutes(5) {
        return Color::ORANGE.alpha(0.5);
    }
    Color::RED.alpha(0.8)
}
