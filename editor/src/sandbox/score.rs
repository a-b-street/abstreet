use crate::game::{State, Transition, WizardState};
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, ModalMenu, Text, VerticalAlignment,
    Wizard,
};
use geom::{Duration, DurationHistogram};
use itertools::Itertools;
use sim::{TripID, TripMode};

pub struct Scoreboard {
    menu: ModalMenu,
    summary: Text,
}

impl Scoreboard {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Scoreboard {
        let menu = ModalMenu::new(
            "Scoreboard",
            vec![vec![
                (hotkey(Key::Escape), "quit"),
                (hotkey(Key::B), "browse trips"),
            ]],
            ctx,
        );
        let t = ui.primary.sim.get_finished_trips();

        let mut summary = Text::from_line("Score at ".to_string());
        summary.append(ui.primary.sim.time().to_string(), Some(Color::RED));
        summary.add_styled_line(
            prettyprint_usize(t.unfinished_trips),
            Some(Color::CYAN),
            None,
            None,
        );
        summary.append(" unfinished trips".to_string(), None);

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
            summary.add_styled_line(format!("{:?}", mode), Some(Color::CYAN), None, None);
            summary.append(format!(" trips: {}", distrib.describe()), None);
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
    let (_, mode) = wizard.choose_something("Browse which trips?", || {
        vec![
            ("walk".to_string(), TripMode::Walk),
            ("bike".to_string(), TripMode::Bike),
            ("transit".to_string(), TripMode::Transit),
            ("drive".to_string(), TripMode::Drive),
        ]
    })?;
    wizard.choose_something_hotkeys("Examine which trip?", || {
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
            .map(|(id, _, dt)| (None, format!("{} taking {}", id, dt), *id))
            .collect()
    })?;
    // TODO show trip departure, where it started and ended
    Some(Transition::Pop)
}
