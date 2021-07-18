use std::collections::BTreeSet;
use std::fs::File;
use std::io::Write;

use anyhow::Result;

use map_gui::tools::PopupMsg;
use sim::{TripID, TripMode};
use widgetry::{EventCtx, GfxCtx, Image, Line, Outcome, Panel, State, TextExt, Toggle, Widget};

use super::trip_problems::{problem_matrix, ProblemType, TripProblemFilter};
use crate::app::{App, Transition};
use crate::sandbox::dashboards::generic_trip_table::open_trip_transition;
use crate::sandbox::dashboards::DashTab;

pub struct RiskSummaries {
    panel: Panel,
}

impl RiskSummaries {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        include_no_changes: bool,
    ) -> Box<dyn State<App>> {
        let bike_filter = Filter {
            modes: maplit::btreeset! { TripMode::Bike },
            include_no_changes,
        };

        let ped_filter = Filter {
            modes: maplit::btreeset! { TripMode::Walk },
            include_no_changes,
        };

        Box::new(RiskSummaries {
            panel: Panel::new_builder(Widget::col(vec![
                DashTab::RiskSummaries.picker(ctx, app),
                Widget::col(vec![
                    "Filters".text_widget(ctx),
                    Toggle::checkbox(
                        ctx,
                        "include trips without any changes",
                        None,
                        include_no_changes,
                    ),
                ])
                .section(ctx),
                Widget::row(vec![
                    Image::from_path("system/assets/meters/pedestrian.svg")
                        .dims(36.0)
                        .into_widget(ctx)
                        .centered_vert(),
                    Line(format!(
                        "Pedestrian Risks - {} Finished Trips",
                        ped_filter.finished_trip_count(app)
                    ))
                    .big_heading_plain()
                    .into_widget(ctx)
                    .centered_vert(),
                ])
                .margin_above(30),
                Widget::evenly_spaced_row(
                    32,
                    vec![Widget::col(vec![
                        Line("Arterial intersection crossings")
                            .small_heading()
                            .into_widget(ctx)
                            .centered_horiz(),
                        problem_matrix(
                            ctx,
                            app,
                            ped_filter
                                .trip_problems(app, ProblemType::ArterialIntersectionCrossing),
                        ),
                    ])
                    .section(ctx)],
                )
                .margin_above(30),
                Widget::row(vec![
                    Image::from_path("system/assets/meters/bike.svg")
                        .dims(36.0)
                        .into_widget(ctx)
                        .centered_vert(),
                    Line(format!(
                        "Cyclist Risks - {} Finished Trips",
                        bike_filter.finished_trip_count(app)
                    ))
                    .big_heading_plain()
                    .into_widget(ctx)
                    .centered_vert(),
                ])
                .margin_above(30),
                Widget::evenly_spaced_row(
                    32,
                    vec![
                        Widget::col(vec![
                            Line("Complex intersection crossings")
                                .small_heading()
                                .into_widget(ctx)
                                .centered_horiz(),
                            problem_matrix(
                                ctx,
                                app,
                                bike_filter
                                    .trip_problems(app, ProblemType::ComplexIntersectionCrossing),
                            ),
                        ])
                        .section(ctx),
                        Widget::col(vec![
                            Line("Cars wanting to over-take cyclists")
                                .small_heading()
                                .into_widget(ctx)
                                .centered_horiz(),
                            problem_matrix(
                                ctx,
                                app,
                                bike_filter.trip_problems(app, ProblemType::OvertakeDesired),
                            ),
                        ])
                        .section(ctx),
                    ],
                )
                .margin_above(30),
                ctx.style().btn_plain.text("Export to CSV").build_def(ctx),
            ]))
            .exact_size_percent(90, 90)
            .build(ctx),
        })
    }
}

impl State<App> for RiskSummaries {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                "Export to CSV" => {
                    return Transition::Push(match export_problems(app) {
                        Ok(path) => PopupMsg::new_state(
                            ctx,
                            "Data exported",
                            vec![format!("Data exported to {}", path)],
                        ),
                        Err(err) => {
                            PopupMsg::new_state(ctx, "Export failed", vec![err.to_string()])
                        }
                    });
                }
                _ => unreachable!(),
            },
            Outcome::ClickCustom(data) => {
                let trips = data.as_any().downcast_ref::<Vec<TripID>>().unwrap();
                // TODO Handle browsing multiple trips
                open_trip_transition(app, trips[0].0)
            }
            Outcome::Changed(_) => {
                if let Some(t) = DashTab::RiskSummaries.transition(ctx, app, &self.panel) {
                    return t;
                }

                let include_no_changes = self.panel.is_checked("include trips without any changes");
                Transition::Replace(RiskSummaries::new_state(ctx, app, include_no_changes))
            }
            _ => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _app: &App) {
        self.panel.draw(g);
    }
}

pub struct Filter {
    modes: BTreeSet<TripMode>,
    include_no_changes: bool,
}

impl TripProblemFilter for Filter {
    fn includes_mode(&self, mode: &TripMode) -> bool {
        self.modes.contains(mode)
    }
    fn include_no_changes(&self) -> bool {
        self.include_no_changes
    }
}

fn export_problems(app: &App) -> Result<String> {
    let path = format!(
        "trip_problems_{}_{}.csv",
        app.primary.map.get_name().as_filename(),
        app.primary.sim.time().as_filename()
    );
    let mut f = File::create(&path)?;
    writeln!(
        f,
        "id,mode,seconds_after,problem_type,problems_before,problems_after"
    )?;

    let before = app.prebaked();
    let after = app.primary.sim.get_analytics();
    let empty = Vec::new();

    for (id, _, time_after, mode) in after.both_finished_trips(app.primary.sim.time(), before) {
        for problem_type in ProblemType::all() {
            let count_before =
                problem_type.count(before.problems_per_trip.get(&id).unwrap_or(&empty));
            let count_after =
                problem_type.count(after.problems_per_trip.get(&id).unwrap_or(&empty));
            if count_before != 0 || count_after != 0 {
                writeln!(
                    f,
                    "{},{:?},{},{:?},{},{}",
                    id.0,
                    mode,
                    time_after.inner_seconds(),
                    problem_type,
                    count_before,
                    count_after
                )?;
            }
        }
    }

    Ok(path)
}
