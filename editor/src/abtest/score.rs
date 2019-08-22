use crate::game::{State, Transition, WizardState};
use crate::ui::PerMapUI;
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, ModalMenu, Text, VerticalAlignment,
    Wizard,
};
use geom::Duration;
use itertools::Itertools;
use sim::{FinishedTrips, TripID, TripMode};
use std::collections::BTreeMap;

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
        summary.add_line("Score at ".to_string());
        summary.append(primary.sim.time().to_string(), Some(Color::RED));
        summary.append(
            format!(
                "... {} / {}",
                primary.map.get_edits().edits_name,
                secondary.map.get_edits().edits_name
            ),
            None,
        );
        summary.add_styled_line(
            prettyprint_usize(t1.unfinished_trips),
            Some(Color::CYAN),
            None,
            None,
        );
        summary.append(" | ".to_string(), None);
        summary.append(prettyprint_usize(t2.unfinished_trips), Some(Color::RED));
        summary.append(" unfinished trips".to_string(), None);

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
                    deltas.push(t1 - t2);
                }
            }
            deltas.sort();
            let len = deltas.len() as f64;

            summary.add_styled_line(format!("{:?}", mode), Some(Color::CYAN), None, None);
            summary.append(
                format!(
                    " trips: {} same, {} different",
                    abstutil::prettyprint_usize(num_same),
                    abstutil::prettyprint_usize(deltas.len())
                ),
                None,
            );
            if !deltas.is_empty() {
                summary.add_line("  deltas: ".to_string());
                summary.append("50%ile".to_string(), Some(Color::RED));
                summary.append(
                    format!(
                        " {}, ",
                        handle_negative(deltas[(0.5 * len).floor() as usize])
                    ),
                    None,
                );
                summary.append("90%ile".to_string(), Some(Color::RED));
                summary.append(
                    format!(
                        " {}, ",
                        handle_negative(deltas[(0.9 * len).floor() as usize])
                    ),
                    None,
                );
                summary.append("99%ile".to_string(), Some(Color::RED));
                summary.append(
                    format!(
                        " {}",
                        handle_negative(deltas[(0.99 * len).floor() as usize])
                    ),
                    None,
                );
            }
        }

        Scoreboard { menu, summary }
    }
}

impl State for Scoreboard {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut UI) -> Transition {
        self.menu.handle_event(ctx, None);
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
        .choose_something("Browse which trips?", || {
            vec![
                ("walk".to_string(), TripMode::Walk),
                ("bike".to_string(), TripMode::Bike),
                ("transit".to_string(), TripMode::Transit),
                ("drive".to_string(), TripMode::Drive),
            ]
        })?
        .1;
    wizard.choose_something("Examine which trip?", || {
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
            .map(|(id, _, t1, t2)| (format!("{} taking {} vs {}", id, t1, t2), *id))
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
fn handle_negative(x: Duration) -> String {
    if x >= Duration::ZERO {
        format!("+{}", x)
    } else {
        format!("-{}", -x)
    }
}
