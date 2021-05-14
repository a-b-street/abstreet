use std::collections::BTreeSet;

use sim::TripMode;
use widgetry::{EventCtx, GfxCtx, Image, Line, Outcome, Panel, State, TextExt, Toggle, Widget};

use super::trip_problems::{problem_matrix, ProblemType, TripProblemFilter};
use crate::app::{App, Transition};
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
                            Line("Large intersection crossings")
                                .small_heading()
                                .into_widget(ctx)
                                .centered_horiz(),
                            problem_matrix(
                                ctx,
                                app,
                                &bike_filter
                                    .trip_problems(app, ProblemType::LargeIntersectionCrossing),
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
                                &bike_filter.trip_problems(app, ProblemType::OvertakeDesired),
                            ),
                        ])
                        .section(ctx),
                    ],
                )
                .margin_above(30),
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
                _ => unreachable!(),
            },
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
