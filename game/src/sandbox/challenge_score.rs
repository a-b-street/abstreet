use crate::game::Transition;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, HorizontalAlignment, Line, Text, VerticalAlignment};
use geom::Duration;
use map_model::BusRouteID;

pub enum ChallengeScoreboard {
    Inactive,
    BusRoute {
        route: BusRouteID,
        time: Duration,
        panel: Text,
    },
}

impl ChallengeScoreboard {
    pub fn event(&mut self, _ctx: &mut EventCtx, ui: &UI) -> Option<Transition> {
        match self {
            ChallengeScoreboard::Inactive => {}
            ChallengeScoreboard::BusRoute {
                route,
                time,
                ref mut panel,
            } => {
                if *time != ui.primary.sim.time() {
                    *time = ui.primary.sim.time();
                    *panel = bus_route_panel(*route, ui);
                }
            }
        }
        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            ChallengeScoreboard::Inactive => {}
            ChallengeScoreboard::BusRoute { ref panel, .. } => {
                g.draw_blocking_text(
                    panel,
                    (HorizontalAlignment::Right, VerticalAlignment::Center),
                );
            }
        }
    }
}

fn bus_route_panel(id: BusRouteID, ui: &UI) -> Text {
    let route = ui.primary.map.get_br(id);
    let arrivals = &ui.primary.sim.get_analytics().bus_arrivals;

    let mut txt = Text::prompt(&route.name);
    for (idx, stop) in route.stops.iter().enumerate() {
        let prev = if idx == 0 { route.stops.len() } else { idx };
        let this = idx + 1;

        txt.add(Line(format!("Stop {}->{}: ", prev, this)));
        if let Some(ref times) = arrivals.get(&(*stop, route.id)) {
            txt.append(Line(format!(
                "{} ago",
                (ui.primary.sim.time() - *times.last().unwrap()).minimal_tostring()
            )));
        } else {
            txt.append(Line("no arrivals yet"));
        }
    }
    txt
}
