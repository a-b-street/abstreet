use crate::app::App;
use crate::game::Transition;
use crate::helpers::{cmp_count_more, cmp_duration_shorter};
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::{challenge_controller, GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use abstutil::prettyprint_usize;
use ezgui::{EventCtx, GfxCtx, Line, Text};
use geom::Statistic;
use sim::TripMode;

pub struct FasterTrips {
    top_center: WrappedComposite,
}

impl FasterTrips {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        trip_mode: TripMode,
        mode: GameplayMode,
    ) -> Box<dyn GameplayState> {
        Box::new(FasterTrips {
            top_center: challenge_controller(
                ctx,
                app,
                mode,
                &format!("Faster Trips {} Challenge", trip_mode.ongoing_verb()),
                Vec::new(),
            ),
        })
    }
}

impl GameplayState for FasterTrips {
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

// TODO Revive
#[allow(unused)]
fn faster_trips_panel(mode: TripMode, app: &App) -> Text {
    let time = app.primary.sim.time();
    let now = app
        .primary
        .sim
        .get_analytics()
        .trip_times(time)
        .2
        .remove(&mode)
        .unwrap();
    let baseline = app.prebaked().trip_times(time).2.remove(&mode).unwrap();

    // Enable to debug why sim results don't match prebaked.
    if false && !now.seems_eq(&baseline) {
        abstutil::write_json(
            "../current_sim.json".to_string(),
            &app.primary.sim.get_analytics().finished_trips,
        );
        let filtered = app
            .prebaked()
            .finished_trips
            .iter()
            .filter(|(t, _, _, _)| *t <= time)
            .cloned()
            .collect::<Vec<_>>();
        abstutil::write_json("../prebaked.json".to_string(), &filtered);
        panic!("At {} ({:?}), finished_trips doesn't match", time, time);
    }

    let mut txt = Text::new();
    txt.add_appended(vec![
        Line(format!(
            "{} trips {} (",
            prettyprint_usize(now.count()),
            mode.ongoing_verb()
        )),
        cmp_count_more(now.count(), baseline.count()),
        Line(")"),
    ]);
    if now.count() == 0 || baseline.count() == 0 {
        return txt;
    }

    for stat in Statistic::all() {
        txt.add(Line(format!("{}: {} (", stat, now.select(stat))));
        txt.append_all(cmp_duration_shorter(
            now.select(stat),
            baseline.select(stat),
        ));
        txt.append(Line(")"));
    }
    txt
}
