use crate::ui::UI;
use ezgui::{
    hotkey, EventCtx, GfxCtx, HorizontalAlignment, Key, ModalMenu, Text, VerticalAlignment, Wizard,
    WrappedWizard,
};
use geom::{Duration, DurationHistogram};
use itertools::Itertools;
use sim::{FinishedTrips, TripID, TripMode};

pub enum Scoreboard {
    Summary(ModalMenu, Text),
    BrowseTrips(FinishedTrips, Wizard),
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

        Scoreboard::Summary(menu, summary)
    }

    // Returns true if done and we should go back to main sandbox mode.
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) -> bool {
        match self {
            Scoreboard::Summary(ref mut menu, _) => {
                menu.handle_event(ctx, None);
                if menu.action("quit") {
                    return true;
                }
                if menu.action("browse trips") {
                    *self =
                        Scoreboard::BrowseTrips(ui.primary.sim.get_finished_trips(), Wizard::new());
                }
            }
            Scoreboard::BrowseTrips(ref trips, ref mut wizard) => {
                if pick_trip(trips, &mut wizard.wrap(ctx)).is_some() {
                    // TODO show trip departure, where it started and ended
                    *self = Scoreboard::new(ctx, ui);
                } else if wizard.aborted() {
                    *self = Scoreboard::new(ctx, ui);
                }
            }
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            Scoreboard::Summary(ref menu, ref txt) => {
                g.draw_blocking_text(
                    txt,
                    (HorizontalAlignment::Center, VerticalAlignment::Center),
                );
                menu.draw(g);
            }
            Scoreboard::BrowseTrips(_, ref wizard) => {
                wizard.draw(g);
            }
        }
    }
}

fn pick_trip(trips: &FinishedTrips, wizard: &mut WrappedWizard) -> Option<TripID> {
    let mode = wizard
        .choose_something_no_keys::<TripMode>(
            "Browse which trips?",
            Box::new(|| {
                vec![
                    ("walk".to_string(), TripMode::Walk),
                    ("bike".to_string(), TripMode::Bike),
                    ("transit".to_string(), TripMode::Transit),
                    ("drive".to_string(), TripMode::Drive),
                ]
            }),
        )?
        .1;
    // TODO Ewwww. Can't do this inside choices_generator because trips isn't &'a static.
    let mut filtered: Vec<&(TripID, TripMode, Duration)> = trips
        .finished_trips
        .iter()
        .filter(|(_, m, _)| *m == mode)
        .collect();
    filtered.sort_by_key(|(_, _, dt)| *dt);
    filtered.reverse();
    let choices: Vec<(String, TripID)> = filtered
        .into_iter()
        // TODO Show percentile for time
        .map(|(id, _, dt)| (format!("{} taking {}", id, dt), *id))
        .collect();
    wizard
        .choose_something_no_keys::<TripID>(
            "Examine which trip?",
            Box::new(move || choices.clone()),
        )
        .map(|(_, id)| id)
}
