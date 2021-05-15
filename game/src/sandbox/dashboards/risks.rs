use std::collections::BTreeSet;
use std::fmt::Display;

use abstutil::{abbreviated_format, prettyprint_usize};
use geom::{Angle, Duration, Polygon, Pt2D, Time};
use map_gui::tools::ColorScale;
use sim::{Problem, TripMode};
use widgetry::{
    Color, DrawWithTooltips, EventCtx, GeomBatch, GeomBatchStack, GfxCtx, Image, Line, Outcome,
    Panel, State, Text, TextExt, Toggle, Widget,
};

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
                            safety_matrix(
                                ctx,
                                app,
                                &bike_filter,
                                ProblemType::LargeIntersectionCrossing,
                            ),
                        ])
                        .section(ctx),
                        Widget::col(vec![
                            Line("Cars wanting to over-take cyclists")
                                .small_heading()
                                .into_widget(ctx)
                                .centered_horiz(),
                            safety_matrix(ctx, app, &bike_filter, ProblemType::OvertakeDesired),
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

lazy_static::lazy_static! {
    static ref CLEAR_COLOR_SCALE: ColorScale = ColorScale(vec![Color::CLEAR, Color::CLEAR]);
}

fn safety_matrix(
    ctx: &mut EventCtx,
    app: &App,
    filter: &Filter,
    problem_type: ProblemType,
) -> Widget {
    let points = filter.get_trips(app, problem_type);

    let duration_buckets = vec![
        Duration::ZERO,
        Duration::minutes(5),
        Duration::minutes(15),
        Duration::minutes(30),
        Duration::hours(1),
        Duration::hours(2),
    ];

    let num_buckets = 7;
    let mut matrix = Matrix::new(duration_buckets, bucketize_isizes(num_buckets, &points));
    for (x, y) in points {
        matrix.add_pt(x, y);
    }
    matrix.draw(
        ctx,
        app,
        MatrixOptions {
            total_width: 600.0,
            total_height: 600.0,
            color_scale_for_bucket: Box::new(|app, _, n| match n.cmp(&0) {
                std::cmp::Ordering::Equal => &CLEAR_COLOR_SCALE,
                std::cmp::Ordering::Less => &app.cs.good_to_bad_green,
                std::cmp::Ordering::Greater => &app.cs.good_to_bad_red,
            }),
            tooltip_for_bucket: Box::new(|(t1, t2), (problems1, problems2), count| {
                let trip_string = if count == 1 {
                    "1 trip".to_string()
                } else {
                    format!("{} trips", prettyprint_usize(count))
                };
                let duration_string = match (t1, t2) {
                    (None, Some(end)) => format!("shorter than {}", end),
                    (Some(start), None) => format!("longer than {}", start),
                    (Some(start), Some(end)) => format!("between {} and {}", start, end),
                    (None, None) => {
                        unreachable!("at least one end of the duration range must be specified")
                    }
                };
                let mut txt = Text::from(format!("{} {}", trip_string, duration_string));
                txt.add_line(match problems1.cmp(&0) {
                    std::cmp::Ordering::Equal => {
                        "had no change in the number of problems encountered.".to_string()
                    }
                    std::cmp::Ordering::Less => {
                        if problems1.abs() == problems2.abs() + 1 {
                            if problems1.abs() == 1 {
                                "encountered 1 fewer problem.".to_string()
                            } else {
                                format!("encountered {} fewer problems.", problems1.abs())
                            }
                        } else {
                            format!(
                                "encountered {}-{} fewer problems.",
                                problems2.abs() + 1,
                                problems1.abs()
                            )
                        }
                    }
                    std::cmp::Ordering::Greater => {
                        if problems1 == problems2 - 1 {
                            if problems1 == 1 {
                                "encountered 1 more problems.".to_string()
                            } else {
                                format!("encountered {} more problems.", problems1,)
                            }
                        } else {
                            format!("encountered {}-{} more problems.", problems1, problems2 - 1)
                        }
                    }
                });
                txt
            }),
        },
    )
}

#[derive(Clone, Copy, PartialEq)]
enum ProblemType {
    IntersectionDelay,
    LargeIntersectionCrossing,
    OvertakeDesired,
}

impl ProblemType {
    fn count(self, problems: &[(Time, Problem)]) -> usize {
        let mut cnt = 0;
        for (_, problem) in problems {
            if match problem {
                Problem::IntersectionDelay(_, _) => self == ProblemType::IntersectionDelay,
                Problem::LargeIntersectionCrossing(_) => {
                    self == ProblemType::LargeIntersectionCrossing
                }
                Problem::OvertakeDesired(_) => self == ProblemType::OvertakeDesired,
            } {
                cnt += 1;
            }
        }
        cnt
    }
}

pub struct Filter {
    modes: BTreeSet<TripMode>,
    include_no_changes: bool,
}

impl Filter {
    // Returns:
    // 1) trip duration after changes
    // 2) difference in number of matching problems, where positive means MORE problems after
    //    changes
    fn get_trips(&self, app: &App, problem_type: ProblemType) -> Vec<(Duration, isize)> {
        let before = app.prebaked();
        let after = app.primary.sim.get_analytics();
        let empty = Vec::new();

        let mut points = Vec::new();
        for (id, _, time_after, mode) in after.both_finished_trips(app.primary.sim.time(), before) {
            if self.modes.contains(&mode) {
                let count_before = problem_type
                    .count(before.problems_per_trip.get(&id).unwrap_or(&empty))
                    as isize;
                let count_after =
                    problem_type.count(after.problems_per_trip.get(&id).unwrap_or(&empty)) as isize;
                if !self.include_no_changes && count_after == count_before {
                    continue;
                }
                points.push((time_after, count_after - count_before));
            }
        }
        points
    }

    fn finished_trip_count(&self, app: &App) -> usize {
        let before = app.prebaked();
        let after = app.primary.sim.get_analytics();

        let mut count = 0;
        for (_, _, _, mode) in after.both_finished_trips(app.primary.sim.time(), before) {
            if self.modes.contains(&mode) {
                count += 1;
            }
        }
        count
    }
}

/// Aka a 2D histogram. Counts the number of matching points in each cell.
struct Matrix<X, Y> {
    counts: Vec<usize>,
    buckets_x: Vec<X>,
    buckets_y: Vec<Y>,
}

impl<X: Copy + PartialOrd + Display, Y: Copy + PartialOrd + Display> Matrix<X, Y> {
    fn new(buckets_x: Vec<X>, buckets_y: Vec<Y>) -> Matrix<X, Y> {
        Matrix {
            counts: std::iter::repeat(0)
                .take(buckets_x.len() * buckets_y.len())
                .collect(),
            buckets_x,
            buckets_y,
        }
    }

    fn add_pt(&mut self, x: X, y: Y) {
        // Find its bucket
        // TODO Unit test this
        let x_idx = self
            .buckets_x
            .iter()
            .position(|min| *min > x)
            .unwrap_or(self.buckets_x.len())
            - 1;
        let y_idx = self
            .buckets_y
            .iter()
            .position(|min| *min > y)
            .unwrap_or(self.buckets_y.len())
            - 1;
        let idx = self.idx(x_idx, y_idx);
        self.counts[idx] += 1;
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        // Row-major
        y * self.buckets_x.len() + x
    }

    fn draw(self, ctx: &mut EventCtx, app: &App, opts: MatrixOptions<X, Y>) -> Widget {
        let mut batch = GeomBatch::new();
        let mut tooltips = Vec::new();
        let cell_width = opts.total_width / (self.buckets_x.len() as f64);
        let cell_height = opts.total_height / (self.buckets_y.len() as f64);
        let cell = Polygon::rectangle(cell_width, cell_height);

        let max_count = *self.counts.iter().max().unwrap() as f64;

        for x in 0..self.buckets_x.len() - 1 {
            for y in 0..self.buckets_y.len() - 1 {
                let is_first_xbucket = x == 0;
                let is_last_xbucket = x == self.buckets_x.len() - 2;
                let is_middle_ybucket = y + 1 == self.buckets_y.len() / 2;
                let count = self.counts[self.idx(x, y)];
                let color = if count == 0 {
                    widgetry::Color::CLEAR
                } else {
                    let density_pct = (count as f64) / max_count;
                    (opts.color_scale_for_bucket)(app, self.buckets_x[x], self.buckets_y[y])
                        .eval(density_pct)
                };
                let x1 = cell_width * (x as f64);
                let y1 = cell_height * (y as f64);
                let rect = cell.clone().translate(x1, y1);
                batch.push(color, rect.clone());
                batch.append(
                    Text::from(if count == 0 && is_middle_ybucket {
                        "-".to_string()
                    } else {
                        abbreviated_format(count)
                    })
                    .change_fg(if count == 0 || is_middle_ybucket {
                        ctx.style().text_primary_color
                    } else {
                        Color::WHITE
                    })
                    .render(ctx)
                    .centered_on(Pt2D::new(x1 + cell_width / 2.0, y1 + cell_height / 2.0)),
                );

                if count != 0 || !is_middle_ybucket {
                    tooltips.push((
                        rect,
                        (opts.tooltip_for_bucket)(
                            (
                                if is_first_xbucket {
                                    None
                                } else {
                                    Some(self.buckets_x[x])
                                },
                                if is_last_xbucket {
                                    None
                                } else {
                                    Some(self.buckets_x[x + 1])
                                },
                            ),
                            (self.buckets_y[y], self.buckets_y[y + 1]),
                            count,
                        ),
                    ));
                }
            }
        }

        // Axis Labels
        let mut y_axis_label = Text::from("More Problems <--------> Fewer Problems")
            .change_fg(ctx.style().text_secondary_color)
            .render(ctx)
            .rotate(Angle::degrees(-90.0));
        y_axis_label.autocrop_dims = true;
        y_axis_label = y_axis_label.autocrop();

        let x_axis_label = Text::from("Short Trips <--------> Long Trips")
            .change_fg(ctx.style().text_secondary_color)
            .render(ctx);

        let vmargin = 32.0;
        for (polygon, _) in tooltips.iter_mut() {
            let mut translated =
                polygon.translate(vmargin + y_axis_label.get_bounds().width(), 0.0);
            std::mem::swap(&mut translated, polygon);
        }
        let mut row = GeomBatchStack::horizontal(vec![y_axis_label, batch]);
        row.set_spacing(vmargin);
        let mut chart = GeomBatchStack::vertical(vec![row.batch(), x_axis_label]);
        chart.set_spacing(16);

        DrawWithTooltips::new_widget(ctx, chart.batch(), tooltips, Box::new(|_| GeomBatch::new()))
    }
}

struct MatrixOptions<X, Y> {
    total_width: f64,
    total_height: f64,
    color_scale_for_bucket: Box<dyn Fn(&App, X, Y) -> &ColorScale>,
    tooltip_for_bucket: Box<dyn Fn((Option<X>, Option<X>), (Y, Y), usize) -> Text>,
}

fn bucketize_isizes(max_buckets: usize, pts: &[(Duration, isize)]) -> Vec<isize> {
    debug_assert!(
        max_buckets % 2 == 1,
        "num_buckets must be odd to have a symmetrical number of buckets around axis"
    );
    debug_assert!(max_buckets >= 3, "num_buckets must be at least 3");

    let positive_buckets = (max_buckets - 1) / 2;
    // uniformly sized integer buckets
    let max = match pts.iter().max_by_key(|(_, cnt)| cnt.abs()) {
        Some(t) if (t.1.abs() as usize) >= positive_buckets => t.1.abs(),
        _ => {
            // Enforce a bucket width of at least 1.
            let negative_buckets = -(positive_buckets as isize);
            return (negative_buckets..=(positive_buckets as isize + 1)).collect();
        }
    };

    let bucket_size = (max as f64 / positive_buckets as f64).ceil() as isize;

    // we start with a 0-based bucket, and build the other buckets out from that.
    let mut buckets = vec![0];

    for i in 0..=positive_buckets {
        // the first positive bucket starts at `1`, to ensure that the 0 bucket stands alone
        buckets.push(1 + (i as isize) * bucket_size);
    }
    for i in 1..=positive_buckets {
        buckets.push(-(i as isize) * bucket_size);
    }
    buckets.sort_unstable();
    debug!("buckets: {:?}", buckets);

    buckets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucketize_isizes() {
        let buckets = bucketize_isizes(
            7,
            &[
                (Duration::minutes(3), -3),
                (Duration::minutes(3), -3),
                (Duration::minutes(3), -1),
                (Duration::minutes(3), 2),
                (Duration::minutes(3), 5),
            ],
        );
        // there should be an even number of buckets on either side of zero so as to center
        // our x-axis.
        //
        // there should always be a 0-1 bucket, ensuring that only '0' falls into the zero-bucket.
        //
        // all other buckets edges should be evenly spaced from the zero bucket
        assert_eq!(buckets, vec![-6, -4, -2, 0, 1, 3, 5, 7])
    }

    #[test]
    fn test_bucketize_empty_isizes() {
        let buckets = bucketize_isizes(7, &[]);
        assert_eq!(buckets, vec![-2, -1, 0, 1, 2])
    }

    #[test]
    fn test_bucketize_small_isizes() {
        let buckets = bucketize_isizes(
            7,
            &[
                (Duration::minutes(3), -1),
                (Duration::minutes(3), -1),
                (Duration::minutes(3), 0),
                (Duration::minutes(3), -1),
                (Duration::minutes(3), 0),
            ],
        );
        assert_eq!(buckets, vec![-3, -2, -1, 0, 1, 2, 3, 4])
    }
}
