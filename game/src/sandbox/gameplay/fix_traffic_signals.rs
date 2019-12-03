use crate::game::{msg, Transition};
use crate::sandbox::gameplay::faster_trips::faster_trips_panel;
use crate::sandbox::gameplay::{manage_overlays, GameplayState};
use crate::sandbox::overlays::Overlays;
use crate::ui::UI;
use ezgui::{hotkey, EventCtx, Key, ModalMenu};
use geom::{Duration, Statistic, Time};
use sim::TripMode;

pub struct FixTrafficSignals {
    time: Time,
}

impl FixTrafficSignals {
    pub fn new(ctx: &EventCtx) -> (ModalMenu, Box<dyn GameplayState>) {
        (
            ModalMenu::new(
                "Fix traffic signals",
                vec![
                    (hotkey(Key::F), "find slowest traffic signals"),
                    (hotkey(Key::H), "help"),
                    (hotkey(Key::S), "final score"),
                ],
                ctx,
            ),
            Box::new(FixTrafficSignals {
                time: Time::START_OF_DAY,
            }),
        )
    }
}

impl GameplayState for FixTrafficSignals {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        overlays: &mut Overlays,
        menu: &mut ModalMenu,
    ) -> Option<Transition> {
        menu.event(ctx);

        // Technically this shows stop signs too, but mostly the bottlenecks are signals.
        if manage_overlays(
            menu,
            ctx,
            "find slowest traffic signals",
            "hide slowest traffic signals",
            overlays,
            match overlays {
                Overlays::IntersectionDelay(_, _) => true,
                _ => false,
            },
            self.time != ui.primary.sim.time(),
        ) {
            *overlays = Overlays::intersection_delay(ctx, ui);
        }

        if self.time != ui.primary.sim.time() {
            self.time = ui.primary.sim.time();
            menu.set_info(ctx, faster_trips_panel(TripMode::Drive, ui));
        }

        if menu.action("help") {
            return Some(Transition::Push(msg(
                "Help",
                vec![
                    "All of the traffic signals follow one timing plan through the whole day.",
                    "(Due to budget cuts, none of the vehicle-actuated signals are working -- don't worry if you don't know what these are.)",
                ])));
        }

        if menu.action("final score") {
            return Some(Transition::Push(msg("Final score", final_score(ui))));
        }

        if ui.primary.sim.time() >= Time::END_OF_DAY {
            // TODO Stop the challenge somehow
            return Some(Transition::Push(msg("Final score", final_score(ui))));
        }

        None
    }
}

fn final_score(ui: &UI) -> Vec<String> {
    let time = ui.primary.sim.time();
    let now = ui
        .primary
        .sim
        .get_analytics()
        .finished_trips(time, TripMode::Drive);
    let baseline = ui.prebaked.finished_trips(time, TripMode::Drive);
    // TODO Annoying to repeat this everywhere; any refactor possible?
    if now.count() == 0 || baseline.count() == 0 {
        return vec!["No data yet, run the simulation for longer".to_string()];
    }
    let now_50p = now.select(Statistic::P50);
    let baseline_50p = baseline.select(Statistic::P50);
    let mut lines = Vec::new();

    if time < Time::END_OF_DAY {
        lines.push(format!("You have to run the simulation until the end of the day to get final results; {} to go", Time::END_OF_DAY - time));
    }

    if now_50p < baseline_50p - Duration::seconds(30.0) {
        lines.push(format!(
            "COMPLETED! 50%ile trip times are now {}, which is {} faster than the baseline {}",
            now_50p,
            baseline_50p - now_50p,
            baseline_50p
        ));
    } else if now_50p < baseline_50p {
        lines.push(format!("Almost there! 50%ile trip times are now {}, which is {} faster than the baseline {}. Can you reduce the times by 30s?", now_50p, baseline_50p - now_50p, baseline_50p));
    } else if now_50p.epsilon_eq(baseline_50p) {
        lines.push(format!(
            "... Did you change anything? 50% ile trip times are {}, same as the baseline",
            now_50p
        ));
    } else {
        lines.push(format!("Err... how did you make things WORSE?! 50%ile trip times are {}, which is {} slower than the baseline {}", now_50p, now_50p - baseline_50p, baseline_50p));
    }
    lines
}
