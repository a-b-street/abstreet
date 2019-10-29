use crate::game::{State, Transition, WizardState};
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, ModalMenu, Text,
    VerticalAlignment, Wizard,
};
use geom::{Duration, DurationHistogram};
use itertools::Itertools;
use sim::{TripID, TripMode};
use std::collections::BTreeSet;

pub struct Scoreboard {
    menu: ModalMenu,
    summary: Text,
}

impl Scoreboard {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Scoreboard {
        let menu = ModalMenu::new(
            "Scoreboard",
            vec![
                (hotkey(Key::Escape), "quit"),
                (hotkey(Key::B), "browse trips"),
            ],
            ctx,
        );
        let t = ui.primary.sim.get_finished_trips();

        let mut summary = Text::new();
        summary.add_appended(vec![
            Line("Score at "),
            Line(ui.primary.sim.time().to_string()).fg(Color::RED),
        ]);
        summary.add_appended(vec![
            Line(prettyprint_usize(t.unfinished_trips)).fg(Color::CYAN),
            Line(" unfinished trips"),
        ]);
        summary.add_appended(vec![
            Line(prettyprint_usize(t.aborted_trips)).fg(Color::CYAN),
            Line(" aborted trips"),
        ]);

        for (mode, trips) in &t
            .finished_trips
            .into_iter()
            .sorted_by_key(|(_, m, _)| *m)
            .group_by(|(_, m, _)| *m)
        {
            let mut distrib: DurationHistogram = std::default::Default::default();
            for (_, _, dt) in trips {
                distrib.add(dt);
            }
            summary.add_appended(vec![
                Line(format!("{:?}", mode)).fg(Color::CYAN),
                Line(format!(" trips: {}", distrib.describe())),
            ]);
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
    let (_, mode) = wizard.choose("Browse which trips?", || {
        let trips = ui.primary.sim.get_finished_trips();
        let modes = trips
            .finished_trips
            .iter()
            .map(|(_, m, _)| *m)
            .collect::<BTreeSet<TripMode>>();

        vec![
            Choice::new("walk", TripMode::Walk).active(modes.contains(&TripMode::Walk)),
            Choice::new("bike", TripMode::Bike).active(modes.contains(&TripMode::Bike)),
            Choice::new("transit", TripMode::Transit).active(modes.contains(&TripMode::Transit)),
            Choice::new("drive", TripMode::Drive).active(modes.contains(&TripMode::Drive)),
        ]
    })?;
    wizard.choose("Examine which trip?", || {
        let trips = ui.primary.sim.get_finished_trips();
        let mut filtered: Vec<&(TripID, TripMode, Duration)> = trips
            .finished_trips
            .iter()
            .filter(|(_, m, _)| *m == mode)
            .collect();
        filtered.sort_by_key(|(_, _, dt)| *dt);
        filtered.reverse();
        filtered
            .into_iter()
            // TODO Show percentile for time
            .map(|(id, _, dt)| Choice::new(format!("{} taking {}", id, dt), *id))
            .collect()
    })?;
    // TODO show trip departure, where it started and ended
    Some(Transition::Pop)
}
