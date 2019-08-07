use crate::game::{State, Transition, WizardState};
use crate::ui::UI;
use ezgui::{
    hotkey, EventCtx, GfxCtx, HorizontalAlignment, Key, ModalMenu, Text, VerticalAlignment, Wizard,
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

        let mut summary = Text::new();
        summary.push(format!("Score at [red:{}]", ui.primary.sim.time()));
        summary.push(format!("[cyan:{}] unfinished trips", t.unfinished_trips));

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
            summary.push(format!("[cyan:{:?}] trips: {}", mode, distrib.describe()));
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
    let (_, mode) = wizard.choose_something_no_keys::<TripMode>(
        "Browse which trips?",
        Box::new(|| {
            vec![
                ("walk".to_string(), TripMode::Walk),
                ("bike".to_string(), TripMode::Bike),
                ("transit".to_string(), TripMode::Transit),
                ("drive".to_string(), TripMode::Drive),
            ]
        }),
    )?;
    wizard.new_choose_something("Examine which trip?", || {
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
