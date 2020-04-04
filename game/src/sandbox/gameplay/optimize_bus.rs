use crate::app::App;
use crate::game::Transition;
use crate::helpers::cmp_duration_shorter;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::{challenge_controller, GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
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
        app: &App,
        route_name: &str,
        mode: GameplayMode,
    ) -> Box<dyn GameplayState> {
        let route = app.primary.map.get_bus_route(route_name).unwrap();
        Box::new(OptimizeBus {
            _route: route.id,
            top_center: challenge_controller(
                ctx,
                app,
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
        app: &mut App,
        _: &mut SandboxControls,
    ) -> (Option<Transition>, bool) {
        match self.top_center.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => {
                return (Some(t), false);
            }
            Some(WrappedOutcome::Clicked(_)) => unreachable!(),
            None => {}
        }
        (None, false)
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}

// TODO Surface this info differently
#[allow(unused)]
fn bus_route_panel(id: BusRouteID, stat: Statistic, app: &App) -> Text {
    let now = app
        .primary
        .sim
        .get_analytics()
        .bus_arrivals(app.primary.sim.time(), id);
    let baseline = app.prebaked().bus_arrivals(app.primary.sim.time(), id);

    let route = app.primary.map.get_br(id);
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
