use crate::game::{msg, Transition};
use crate::sandbox::gameplay::{cmp_count_more, cmp_duration_shorter, State};
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{hotkey, EventCtx, Key, Line, ModalMenu, Text};
use geom::{Duration, Statistic};
use sim::{Analytics, TripMode};

pub struct FasterTrips {
    mode: TripMode,
    time: Duration,
}

impl FasterTrips {
    pub fn new(trip_mode: TripMode, ctx: &EventCtx) -> (ModalMenu, State) {
        (
            ModalMenu::new(
                &format!("Speed up {} trips", trip_mode),
                vec![(hotkey(Key::H), "help")],
                ctx,
            ),
            State::FasterTrips(FasterTrips {
                mode: trip_mode,
                time: Duration::ZERO,
            }),
        )
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        menu: &mut ModalMenu,
        prebaked: &Analytics,
    ) -> Option<Transition> {
        menu.event(ctx);

        if self.time != ui.primary.sim.time() {
            self.time = ui.primary.sim.time();
            menu.set_info(ctx, faster_trips_panel(self.mode, ui, prebaked));
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

fn faster_trips_panel(mode: TripMode, ui: &UI, prebaked: &Analytics) -> Text {
    let now = ui
        .primary
        .sim
        .get_analytics()
        .finished_trips(ui.primary.sim.time(), mode);
    let baseline = prebaked.finished_trips(ui.primary.sim.time(), mode);

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
        txt.add(Line(format!("{}: ", stat)));
        txt.append_all(cmp_duration_shorter(
            now.select(stat),
            baseline.select(stat),
        ));
    }
    txt
}
