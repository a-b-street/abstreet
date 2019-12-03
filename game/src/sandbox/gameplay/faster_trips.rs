use crate::game::{msg, Transition};
use crate::sandbox::gameplay::{cmp_count_more, cmp_duration_shorter, GameplayState};
use crate::sandbox::overlays::Overlays;
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{hotkey, EventCtx, Key, Line, ModalMenu, Text};
use geom::{Statistic, Time};
use sim::TripMode;

pub struct FasterTrips {
    mode: TripMode,
    time: Time,
}

impl FasterTrips {
    pub fn new(trip_mode: TripMode, ctx: &EventCtx) -> (ModalMenu, Box<dyn GameplayState>) {
        (
            ModalMenu::new(
                &format!("Speed up {} trips", trip_mode),
                vec![(hotkey(Key::H), "help")],
                ctx,
            ),
            Box::new(FasterTrips {
                mode: trip_mode,
                time: Time::START_OF_DAY,
            }),
        )
    }
}

impl GameplayState for FasterTrips {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        _: &mut Overlays,
        menu: &mut ModalMenu,
    ) -> Option<Transition> {
        menu.event(ctx);

        if self.time != ui.primary.sim.time() {
            self.time = ui.primary.sim.time();
            menu.set_info(ctx, faster_trips_panel(self.mode, ui));
        }

        if menu.action("help") {
            return Some(Transition::Push(msg(
                "Help",
                vec!["How can you possibly speed up all trips of some mode?"],
            )));
        }
        None
    }
}

pub fn faster_trips_panel(mode: TripMode, ui: &UI) -> Text {
    let time = ui.primary.sim.time();
    let now = ui.primary.sim.get_analytics().finished_trips(time, mode);
    let baseline = ui.prebaked.finished_trips(time, mode);

    // Enable to debug why sim results don't match prebaked.
    if false && !now.seems_eq(&baseline) {
        abstutil::write_json(
            "../current_sim.json".to_string(),
            &ui.primary.sim.get_analytics().finished_trips,
        );
        let filtered = ui
            .prebaked
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
        txt.add(Line(format!("{}: {} ", stat, now.select(stat))));
        txt.append_all(cmp_duration_shorter(
            now.select(stat),
            baseline.select(stat),
        ));
    }
    txt
}
