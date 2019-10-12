use crate::game::{State, Transition, WizardState};
use crate::ui::PerMapUI;
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, ModalMenu, Text,
    TextSpan, VerticalAlignment, Wizard,
};
use geom::Duration;
use itertools::Itertools;
use sim::{FinishedTrips, TripID, TripMode};
use std::collections::{BTreeMap, BTreeSet};

pub struct Scoreboard {
    menu: ModalMenu,
    summary: Text,
}

impl Scoreboard {
    pub fn new(ctx: &mut EventCtx, primary: &PerMapUI, secondary: &PerMapUI) -> Scoreboard {
        let menu = ModalMenu::new(
            "Scoreboard",
            vec![vec![
                (hotkey(Key::Escape), "quit"),
                (hotkey(Key::B), "browse trips"),
            ]],
            ctx,
        );
        let t1 = primary.sim.get_finished_trips();
        let t2 = secondary.sim.get_finished_trips();

        let mut summary = Text::new();
        summary.add_appended(vec![
            Line(format!("Score at {}... ", primary.sim.time())),
            // TODO Should we use consistent colors for the A and B?
            Line(&primary.map.get_edits().edits_name).fg(Color::RED),
            Line(" / "),
            Line(&secondary.map.get_edits().edits_name).fg(Color::CYAN),
        ]);
        summary.add_appended(vec![
            Line(prettyprint_usize(t1.unfinished_trips)).fg(Color::RED),
            Line(" | "),
            Line(prettyprint_usize(t2.unfinished_trips)).fg(Color::CYAN),
            Line(" unfinished trips"),
        ]);
        summary.add_appended(vec![
            Line(prettyprint_usize(t1.aborted_trips)).fg(Color::RED),
            Line(" | "),
            Line(prettyprint_usize(t2.aborted_trips)).fg(Color::CYAN),
            Line(" aborted trips"),
        ]);
        summary.add_appended(vec![
            Line("faster (better)").fg(Color::GREEN),
            Line(" / "),
            Line("slower (worse)").fg(Color::YELLOW),
        ]);

        let cmp = CompareTrips::new(t1, t2);
        for (mode, trips) in &cmp
            .finished_trips
            .into_iter()
            .sorted_by_key(|(_, m, _, _)| *m)
            .group_by(|(_, m, _, _)| *m)
        {
            let mut num_same = 0;

            // DurationHistogram doesn't handle deltas. Since the number of trips isn't huge,
            // manually do this...
            let mut deltas = Vec::new();
            for (_, _, t1, t2) in trips {
                if t1 == t2 {
                    num_same += 1;
                } else {
                    // Negative means the primary is faster
                    deltas.push(t1 - t2);
                }
            }
            deltas.sort();
            let len = deltas.len() as f64;

            summary.add_appended(vec![
                Line(format!("{:?}", mode)).fg(Color::PURPLE),
                Line(format!(
                    " trips: {} same, {} different",
                    abstutil::prettyprint_usize(num_same),
                    abstutil::prettyprint_usize(deltas.len())
                )),
            ]);
            if !deltas.is_empty() {
                summary.add_appended(vec![
                    Line("  deltas: 50%ile "),
                    print_delta(deltas[(0.5 * len).floor() as usize]),
                    Line(", 90%ile "),
                    print_delta(deltas[(0.9 * len).floor() as usize]),
                    Line(", 99%ile "),
                    print_delta(deltas[(0.99 * len).floor() as usize]),
                ]);
            }
        }

        Scoreboard { menu, summary }
    }
}

impl State for Scoreboard {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut UI) -> Transition {
        self.menu.event(ctx);
        if self.menu.action("quit") {
            return Transition::Pop;
        }
        if self.menu.action("browse trips") {
            return Transition::Push(WizardState::new(Box::new(browse_trips)));
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        g.draw_blocking_text(
            &self.summary,
            (HorizontalAlignment::Center, VerticalAlignment::Center),
        );
        self.menu.draw(g);
    }
}

fn browse_trips(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let mut wizard = wiz.wrap(ctx);
    let mode = wizard
        .choose("Browse which trips?", || {
            let trips = CompareTrips::new(
                ui.primary.sim.get_finished_trips(),
                ui.secondary.as_ref().unwrap().sim.get_finished_trips(),
            );
            let modes = trips
                .finished_trips
                .iter()
                .map(|(_, m, _, _)| *m)
                .collect::<BTreeSet<TripMode>>();

            vec![
                Choice::new("walk", TripMode::Walk).active(modes.contains(&TripMode::Walk)),
                Choice::new("bike", TripMode::Bike).active(modes.contains(&TripMode::Bike)),
                Choice::new("transit", TripMode::Transit)
                    .active(modes.contains(&TripMode::Transit)),
                Choice::new("drive", TripMode::Drive).active(modes.contains(&TripMode::Drive)),
            ]
        })?
        .1;
    wizard.choose("Examine which trip?", || {
        let trips = CompareTrips::new(
            ui.primary.sim.get_finished_trips(),
            ui.secondary.as_ref().unwrap().sim.get_finished_trips(),
        );
        let mut filtered: Vec<&(TripID, TripMode, Duration, Duration)> = trips
            .finished_trips
            .iter()
            .filter(|(_, m, t1, t2)| *m == mode && *t1 != *t2)
            .collect();
        filtered.sort_by_key(|(_, _, t1, t2)| *t1 - *t2);
        filtered.reverse();
        filtered
            .into_iter()
            .map(|(id, _, t1, t2)| Choice::new(format!("{} taking {} vs {}", id, t1, t2), *id))
            .collect()
    })?;
    // TODO show more details...
    Some(Transition::Pop)
}

pub struct CompareTrips {
    // Just finished in both, for now
    finished_trips: Vec<(TripID, TripMode, Duration, Duration)>,
}

impl CompareTrips {
    fn new(t1: FinishedTrips, t2: FinishedTrips) -> CompareTrips {
        let trips1: BTreeMap<TripID, (TripMode, Duration)> = t1
            .finished_trips
            .into_iter()
            .map(|(id, mode, time)| (id, (mode, time)))
            .collect();
        let trips2: BTreeMap<TripID, (TripMode, Duration)> = t2
            .finished_trips
            .into_iter()
            .map(|(id, mode, time)| (id, (mode, time)))
            .collect();

        let mut cmp = CompareTrips {
            finished_trips: Vec::new(),
        };
        for (id, (mode, time1)) in trips1 {
            if let Some((_, time2)) = trips2.get(&id) {
                cmp.finished_trips.push((id, mode, time1, *time2));
            }
        }
        cmp
    }
}

// TODO I think it's time for a proper Time and Duration distinction.
fn print_delta(x: Duration) -> TextSpan {
    if x >= Duration::ZERO {
        Line(x.minimal_tostring()).fg(Color::YELLOW)
    } else {
        Line((-x).minimal_tostring()).fg(Color::GREEN)
    }
}
