use crate::game::Transition;
use crate::helpers::cmp_duration_shorter;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::{challenge_controller, GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, Line, Text};
use geom::Statistic;
use map_model::BusRouteID;

pub struct OptimizeBus {
    _route: BusRouteID,
    top_center: WrappedComposite,
}

impl OptimizeBus {
    pub fn new(
        ctx: &mut EventCtx,
        ui: &UI,
        route_name: &str,
        mode: GameplayMode,
    ) -> Box<dyn GameplayState> {
        let route = ui.primary.map.get_bus_route(route_name).unwrap();
        Box::new(OptimizeBus {
            _route: route.id,
            top_center: challenge_controller(
                ctx,
                mode,
                &format!("Optimize {} Challenge", route_name),
                Vec::new(),
            ),
        })
    }
}

impl GameplayState for OptimizeBus {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        _: &mut SandboxControls,
    ) -> (Option<Transition>, bool) {
        match self.top_center.event(ctx, ui) {
            Some(WrappedOutcome::Transition(t)) => {
                return (Some(t), false);
            }
            Some(WrappedOutcome::Clicked(_)) => unreachable!(),
            None => {}
        }
        (None, false)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.top_center.draw(g);
    }
}

// TODO Surface this info differently
#[allow(unused)]
fn bus_route_panel(id: BusRouteID, stat: Statistic, ui: &UI) -> Text {
    let now = ui
        .primary
        .sim
        .get_analytics()
        .bus_arrivals(ui.primary.sim.time(), id);
    let baseline = ui.prebaked().bus_arrivals(ui.primary.sim.time(), id);

    let route = ui.primary.map.get_br(id);
    let mut txt = Text::new();
    txt.add(Line(format!("{} delay between stops", stat)));
    for idx1 in 0..route.stops.len() {
        let idx2 = if idx1 == route.stops.len() - 1 {
            0
        } else {
            idx1 + 1
        };
        // TODO Also display number of arrivals...
        txt.add(Line(format!("Stop {}->{}: ", idx1 + 1, idx2 + 1)));
        if let Some(ref stats1) = now.get(&route.stops[idx2]) {
            let a = stats1.select(stat);
            txt.append(Line(a.to_string()));

            if let Some(ref stats2) = baseline.get(&route.stops[idx2]) {
                txt.append(Line(" ("));
                txt.append_all(cmp_duration_shorter(a, stats2.select(stat)));
                txt.append(Line(")"));
            }
        } else {
            txt.append(Line("no arrivals yet"));
        }
    }
    txt
}
