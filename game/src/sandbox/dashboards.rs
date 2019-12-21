use crate::common::TripExplorer;
use crate::game::{State, Transition, WizardState};
use crate::managed::{Composite, ManagedGUIState};
use crate::sandbox::bus_explorer;
use crate::sandbox::gameplay::{cmp_count_fewer, cmp_count_more, cmp_duration_shorter};
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{hotkey, Choice, Color, EventCtx, Key, Line, ManagedWidget, Text, Wizard};
use geom::{Duration, Statistic, Time};
use sim::{Analytics, TripID, TripMode};
use std::collections::BTreeSet;

#[derive(PartialEq, Clone, Copy)]
pub enum Tab {
    FinishedTripsSummary,
    IndividualFinishedTrips,
    ParkingOverhead,
    ExploreBusRoute,
}

// Oh the dashboards melted, but we still had the radio
pub fn make(ctx: &EventCtx, ui: &UI, prebaked: &Analytics, tab: Tab) -> Box<dyn State> {
    let tab_data = vec![
        (Tab::FinishedTripsSummary, "Finished trips summary"),
        (
            Tab::IndividualFinishedTrips,
            "Deep-dive into individual finished trips",
        ),
        (Tab::ParkingOverhead, "Parking overhead analysis"),
        (Tab::ExploreBusRoute, "Explore a bus route"),
    ];

    let mut tabs = tab_data
        .iter()
        .map(|(t, label)| {
            if *t == tab {
                ManagedWidget::draw_text(ctx, Text::from(Line(*label)))
            } else {
                Composite::text_button(ctx, label, None)
            }
        })
        .collect::<Vec<_>>();
    tabs.push(Composite::text_button(ctx, "BACK", hotkey(Key::Escape)));

    let content = match tab {
        Tab::FinishedTripsSummary => finished_trips_summary(ctx, ui, prebaked),
        Tab::IndividualFinishedTrips => {
            return WizardState::new(Box::new(browse_trips));
        }
        Tab::ParkingOverhead => parking_overhead(ctx, ui),
        Tab::ExploreBusRoute => {
            return bus_explorer::pick_any_bus_route(ui);
        }
    };

    let mut c = Composite::new(ezgui::Composite::fill_screen(
        ctx,
        ManagedWidget::col(vec![ManagedWidget::row(tabs).evenly_spaced(), content]),
    ))
    .cb("BACK", Box::new(|_, _| Some(Transition::Pop)));
    for (t, label) in tab_data {
        if t != tab {
            c = c.cb(
                label,
                Box::new(move |ctx, ui| {
                    Some(Transition::Replace(make(
                        ctx,
                        ui,
                        // TODO prebaked?
                        &Analytics::new(),
                        t,
                    )))
                }),
            );
        }
    }

    ManagedGUIState::new(c)
}

fn finished_trips_summary(ctx: &EventCtx, ui: &UI, prebaked: &Analytics) -> ManagedWidget {
    let (now_all, now_aborted, now_per_mode) = ui
        .primary
        .sim
        .get_analytics()
        .all_finished_trips(ui.primary.sim.time());
    let (baseline_all, baseline_aborted, baseline_per_mode) =
        prebaked.all_finished_trips(ui.primary.sim.time());

    // TODO Include unfinished count
    let mut txt = Text::new();
    txt.add_appended(vec![
        Line("Finished trips as of "),
        Line(ui.primary.sim.time().ampm_tostring()).fg(Color::CYAN),
    ]);
    txt.add_appended(vec![
        Line(format!(
            "  {} aborted trips (",
            prettyprint_usize(now_aborted)
        )),
        cmp_count_fewer(now_aborted, baseline_aborted),
        Line(")"),
    ]);
    // TODO Refactor
    txt.add_appended(vec![
        Line(format!(
            "{} total finished trips (",
            prettyprint_usize(now_all.count())
        )),
        cmp_count_more(now_all.count(), baseline_all.count()),
        Line(")"),
    ]);
    if now_all.count() > 0 && baseline_all.count() > 0 {
        for stat in Statistic::all() {
            txt.add(Line(format!("  {}: {} ", stat, now_all.select(stat))));
            txt.append_all(cmp_duration_shorter(
                now_all.select(stat),
                baseline_all.select(stat),
            ));
        }
    }

    for mode in TripMode::all() {
        let a = &now_per_mode[&mode];
        let b = &baseline_per_mode[&mode];
        txt.add_appended(vec![
            Line(format!("{} {} trips (", prettyprint_usize(a.count()), mode)),
            cmp_count_more(a.count(), b.count()),
            Line(")"),
        ]);
        if a.count() > 0 && b.count() > 0 {
            for stat in Statistic::all() {
                txt.add(Line(format!("  {}: {} ", stat, a.select(stat))));
                txt.append_all(cmp_duration_shorter(a.select(stat), b.select(stat)));
            }
        }
    }

    // TODO include the plot here

    ManagedWidget::draw_text(ctx, txt)
}

fn browse_trips(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let mut wizard = wiz.wrap(ctx);
    let (_, mode) = wizard.choose("Browse which trips?", || {
        let modes = ui
            .primary
            .sim
            .get_analytics()
            .finished_trips
            .iter()
            .filter_map(|(_, _, m, _)| *m)
            .collect::<BTreeSet<TripMode>>();
        TripMode::all()
            .into_iter()
            .map(|m| Choice::new(m.to_string(), m).active(modes.contains(&m)))
            .collect()
    })?;
    let (_, trip) = wizard.choose("Examine which trip?", || {
        let mut filtered: Vec<&(Time, TripID, Option<TripMode>, Duration)> = ui
            .primary
            .sim
            .get_analytics()
            .finished_trips
            .iter()
            .filter(|(_, _, m, _)| *m == Some(mode))
            .collect();
        filtered.sort_by_key(|(_, _, _, dt)| *dt);
        filtered.reverse();
        filtered
            .into_iter()
            // TODO Show percentile for time
            .map(|(_, id, _, dt)| Choice::new(format!("{} taking {}", id, dt), *id))
            .collect()
    })?;

    wizard.reset();
    Some(Transition::Push(Box::new(TripExplorer::new(trip, ctx, ui))))
}

fn parking_overhead(ctx: &EventCtx, ui: &UI) -> ManagedWidget {
    let mut txt = Text::new();
    for line in ui.primary.sim.get_analytics().analyze_parking_phases() {
        txt.add_wrapped_line(ctx, line);
    }
    ManagedWidget::draw_text(ctx, txt)
}
