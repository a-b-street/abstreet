use crate::game::Transition;
use crate::helpers::{cmp_count_more, cmp_duration_shorter};
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::{challenge_controller, GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{layout, EventCtx, GfxCtx, Line, ModalMenu, Text};
use geom::{Statistic, Time};
use sim::TripMode;

pub struct FasterTrips {
    mode: TripMode,
    time: Time,
    menu: ModalMenu,
    top_center: WrappedComposite,
}

impl FasterTrips {
    pub fn new(trip_mode: TripMode, ctx: &mut EventCtx) -> Box<dyn GameplayState> {
        Box::new(FasterTrips {
            mode: trip_mode,
            time: Time::START_OF_DAY,
            menu: ModalMenu::new::<&str, &str>("", Vec::new(), ctx)
                .set_standalone_layout(layout::ContainerOrientation::TopLeftButDownABit(150.0)),
            top_center: challenge_controller(
                ctx,
                GameplayMode::FasterTrips(trip_mode),
                &format!("Faster {} Trips Challenge", trip_mode),
                Vec::new(),
            ),
        })
    }
}

impl GameplayState for FasterTrips {
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
        self.menu.event(ctx);

        if self.time != ui.primary.sim.time() {
            self.time = ui.primary.sim.time();
            self.menu.set_info(ctx, faster_trips_panel(self.mode, ui));
        }

        (None, false)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.top_center.draw(g);
        self.menu.draw(g);
    }
}

pub fn faster_trips_panel(mode: TripMode, ui: &UI) -> Text {
    let time = ui.primary.sim.time();
    let now = ui.primary.sim.get_analytics().finished_trips(time, mode);
    let baseline = ui.prebaked().finished_trips(time, mode);

    // Enable to debug why sim results don't match prebaked.
    if false && !now.seems_eq(&baseline) {
        abstutil::write_json(
            "../current_sim.json".to_string(),
            &ui.primary.sim.get_analytics().finished_trips,
        );
        let filtered = ui
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
            "{} finished {} trips (",
            prettyprint_usize(now.count()),
            mode
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
