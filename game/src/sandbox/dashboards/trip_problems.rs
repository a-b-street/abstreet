use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Display;

use abstutil::{abbreviated_format, prettyprint_usize};
use geom::{Angle, Distance, Duration, Line, Polygon, Pt2D, Time};
use map_gui::tools::ColorScale;
use sim::{Problem, TripID, TripMode};
use widgetry::{Color, DrawWithTooltips, GeomBatch, GeomBatchStack, StackAlignment, Text, Widget};

use crate::{App, EventCtx};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ProblemType {
    IntersectionDelay,
    ComplexIntersectionCrossing,
    OvertakeDesired,
    ArterialIntersectionCrossing,
}

impl From<&Problem> for ProblemType {
    fn from(problem: &Problem) -> Self {
        match problem {
            Problem::IntersectionDelay(_, _) => Self::IntersectionDelay,
            Problem::ComplexIntersectionCrossing(_) => Self::ComplexIntersectionCrossing,
            Problem::OvertakeDesired(_) => Self::OvertakeDesired,
            Problem::ArterialIntersectionCrossing(_) => Self::ArterialIntersectionCrossing,
        }
    }
}

impl ProblemType {
    pub fn count(self, problems: &[(Time, Problem)]) -> usize {
        let mut cnt = 0;
        for (_, problem) in problems {
            if self == ProblemType::from(problem) {
                cnt += 1;
            }
        }
        cnt
    }

    pub fn all() -> Vec<ProblemType> {
        vec![
            ProblemType::IntersectionDelay,
            ProblemType::ComplexIntersectionCrossing,
            ProblemType::OvertakeDesired,
            ProblemType::ArterialIntersectionCrossing,
        ]
    }
}

pub trait TripProblemFilter {
    fn includes_mode(&self, mode: &TripMode) -> bool;
    fn include_no_changes(&self) -> bool;

    // Returns:
    // 1) trip ID
    // 2) trip duration after changes
    // 3) difference in number of matching problems, where positive means MORE problems after
    //    changes
    fn trip_problems(
        &self,
        app: &App,
        problem_type: ProblemType,
    ) -> Vec<(TripID, Duration, isize)> {
        let before = app.prebaked();
        let after = app.primary.sim.get_analytics();
        let empty = Vec::new();

        let mut points = Vec::new();
        for (id, _, time_after, mode) in after.both_finished_trips(app.primary.sim.time(), before) {
            if self.includes_mode(&mode) {
                let count_before = problem_type
                    .count(before.problems_per_trip.get(&id).unwrap_or(&empty))
                    as isize;
                let count_after =
                    problem_type.count(after.problems_per_trip.get(&id).unwrap_or(&empty)) as isize;
                if !self.include_no_changes() && count_after == count_before {
                    continue;
                }
                points.push((id, time_after, count_after - count_before));
            }
        }
        points
    }

    fn finished_trip_count(&self, app: &App) -> usize {
        let before = app.prebaked();
        let after = app.primary.sim.get_analytics();

        let mut count = 0;
        for (_, _, _, mode) in after.both_finished_trips(app.primary.sim.time(), before) {
            if self.includes_mode(&mode) {
                count += 1;
            }
        }
        count
    }
}

lazy_static::lazy_static! {
    static ref CLEAR_COLOR_SCALE: ColorScale = ColorScale(vec![Color::CLEAR, Color::CLEAR]);
}

/// The `title` is just used to generate unique labels. Returns a widget and a mapping from
/// `Outcome::Clicked` labels to the list of trips matching the bucket.
pub fn problem_matrix(
    ctx: &mut EventCtx,
    app: &App,
    title: &str,
    trips: Vec<(TripID, Duration, isize)>,
) -> (Widget, HashMap<String, Vec<TripID>>) {
    let duration_buckets = vec![
        Duration::ZERO,
        Duration::minutes(5),
        Duration::minutes(15),
        Duration::minutes(30),
        Duration::hours(1),
        Duration::hours(2),
    ];

    let num_buckets = 7;
    let mut matrix = Matrix::new(duration_buckets, bucketize_isizes(num_buckets, &trips));
    for (id, x, y) in trips {
        matrix.add_pt(id, x, y);
    }
    matrix.draw(
        ctx,
        app,
        MatrixOptions {
            title: title.to_string(),
            total_width: 600.0,
            total_height: 600.0,
            color_scale_for_bucket: Box::new(|app, _, n| match n.cmp(&0) {
                std::cmp::Ordering::Equal => &CLEAR_COLOR_SCALE,
                std::cmp::Ordering::Less => &app.cs.good_to_bad_green,
                std::cmp::Ordering::Greater => &app.cs.good_to_bad_red,
            }),
            fmt_y_axis: Box::new(|lower_bound: isize, upper_bound: isize| -> Text {
                if lower_bound + 1 == upper_bound {
                    Text::from(lower_bound.abs().to_string())
                } else if lower_bound.is_negative() {
                    Text::from(format!("{}-{}", upper_bound.abs() + 1, lower_bound.abs()))
                } else {
                    Text::from(format!("{}-{}", lower_bound.abs(), upper_bound.abs() - 1))
                }
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

/// Aka a 2D histogram. Tracks matching IDs in each cell.
struct Matrix<ID, X, Y> {
    entries: Vec<Vec<ID>>,
    buckets_x: Vec<X>,
    buckets_y: Vec<Y>,
}

impl<ID, X: Copy + PartialOrd + Display, Y: Copy + PartialOrd + Display> Matrix<ID, X, Y> {
    fn new(buckets_x: Vec<X>, buckets_y: Vec<Y>) -> Matrix<ID, X, Y> {
        Matrix {
            entries: std::iter::repeat_with(Vec::new)
                .take(buckets_x.len() * buckets_y.len())
                .collect(),
            buckets_x,
            buckets_y,
        }
    }

    fn add_pt(&mut self, id: ID, x: X, y: Y) {
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
        self.entries[idx].push(id);
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        // Row-major
        y * self.buckets_x.len() + x
    }

    fn draw(
        mut self,
        ctx: &mut EventCtx,
        app: &App,
        opts: MatrixOptions<X, Y>,
    ) -> (Widget, HashMap<String, Vec<ID>>) {
        let mut grid_batch = GeomBatch::new();
        let mut tooltips = Vec::new();
        let cell_width = opts.total_width / (self.buckets_x.len() as f64);
        let cell_height = opts.total_height / (self.buckets_y.len() as f64);
        let cell = Polygon::rectangle(cell_width, cell_height);

        let max_count = self.entries.iter().map(|list| list.len()).max().unwrap() as f64;

        let mut mapping = HashMap::new();
        for x in 0..self.buckets_x.len() - 1 {
            for y in 0..self.buckets_y.len() - 1 {
                let is_first_xbucket = x == 0;
                let is_last_xbucket = x == self.buckets_x.len() - 2;
                let is_middle_ybucket = y + 1 == self.buckets_y.len() / 2;
                let idx = self.idx(x, y);
                let count = self.entries[idx].len();
                let bucket_label = format!("{}/{}", opts.title, idx);
                if count != 0 {
                    mapping.insert(bucket_label.clone(), std::mem::take(&mut self.entries[idx]));
                }
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
                grid_batch.push(color, rect.clone());
                grid_batch.append(
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
                        if count != 0 { Some(bucket_label) } else { None },
                    ));
                }
            }
        }
        {
            let bottom = cell_height * (self.buckets_y.len() - 1) as f64;
            let right = cell_width * (self.buckets_x.len() - 1) as f64;

            let border_lines = vec![
                Line::new(Pt2D::zero(), Pt2D::new(right, 0.0)).unwrap(),
                Line::new(Pt2D::new(right, 0.0), Pt2D::new(right, bottom)).unwrap(),
                Line::new(Pt2D::new(right, bottom), Pt2D::new(0.0, bottom)).unwrap(),
                Line::new(Pt2D::new(0.0, bottom), Pt2D::zero()).unwrap(),
            ];
            for line in border_lines {
                let border_poly = line.make_polygons(Distance::meters(3.0));
                grid_batch.push(ctx.style().text_secondary_color, border_poly);
            }
        }

        // Draw the axes
        let y_axis_batch = {
            let mut y_axis_scale = GeomBatch::new();
            for y in 0..self.buckets_y.len() - 1 {
                let x1 = 0.0;
                let mut y1 = cell_height * y as f64;

                let middle_bucket = self.buckets_y.len() / 2 - 1;
                let y_offset = match y.cmp(&middle_bucket) {
                    Ordering::Less => cell_height,
                    Ordering::Greater => 0.0,
                    Ordering::Equal => cell_height / 2.0,
                };

                let y_label = (opts.fmt_y_axis)(self.buckets_y[y], self.buckets_y[y + 1])
                    .change_fg(ctx.style().text_secondary_color)
                    .render(ctx)
                    .centered_on(Pt2D::new(x1 + cell_width / 2.0, y1 + 0.5 * cell_height));
                y_axis_scale.append(y_label);

                if y != middle_bucket {
                    y1 += y_offset;
                    let tick_length = 8.0;
                    let tick_thickness = 2.0;
                    let start = Pt2D::new(x1 + cell_width - tick_length, y1 - tick_thickness / 2.0);
                    let line = Line::new(start, start.offset(tick_length, 0.0))
                        .unwrap()
                        .make_polygons(Distance::meters(tick_thickness));
                    y_axis_scale.push(ctx.style().text_secondary_color, line);
                }
            }
            let mut y_axis_label = Text::from("More Problems <--------> Fewer Problems")
                .change_fg(ctx.style().text_secondary_color)
                .render(ctx)
                .rotate(Angle::degrees(-90.0));

            y_axis_label.autocrop_dims = true;
            y_axis_label = y_axis_label.autocrop();

            y_axis_label = y_axis_label.centered_on(Pt2D::new(
                8.0,
                cell_height * (self.buckets_y.len() as f64 / 2.0 - 1.0),
            ));

            GeomBatchStack::horizontal(vec![y_axis_label, y_axis_scale]).batch()
        };

        let x_axis_batch = {
            let mut x_axis_scale = GeomBatch::new();
            for x in 1..self.buckets_x.len() - 1 {
                let x1 = cell_width * x as f64;
                let y1 = 0.0;

                x_axis_scale.append(
                    Text::from(format!("{}", self.buckets_x[x]))
                        .change_fg(ctx.style().text_secondary_color)
                        .render(ctx)
                        .centered_on(Pt2D::new(x1, y1 + cell_height / 2.0)),
                );
                let tick_length = 8.0;
                let tick_thickness = 2.0;
                let start = Pt2D::new(x1, y1 - 2.0);
                let line = Line::new(start, start.offset(0.0, tick_length))
                    .unwrap()
                    .make_polygons(Distance::meters(tick_thickness));
                x_axis_scale.push(ctx.style().text_secondary_color, line);
            }
            let x_axis_label = Text::from("Short Trips <--------> Long Trips")
                .change_fg(ctx.style().text_secondary_color)
                .render(ctx)
                .centered_on(Pt2D::new(
                    cell_width * ((self.buckets_x.len() as f64) / 2.0 - 0.5),
                    cell_height,
                ));

            x_axis_scale.append(x_axis_label);

            x_axis_scale
        };

        for (polygon, _, _) in &mut tooltips {
            let mut translated = polygon.translate(y_axis_batch.get_bounds().width(), 0.0);
            std::mem::swap(&mut translated, polygon);
        }
        let mut col = GeomBatchStack::vertical(vec![grid_batch, x_axis_batch]);
        col.set_alignment(StackAlignment::Left);

        let mut chart = GeomBatchStack::horizontal(vec![y_axis_batch, col.batch()]);
        chart.set_alignment(StackAlignment::Top);

        (
            DrawWithTooltips::new_widget(
                ctx,
                chart.batch(),
                tooltips,
                Box::new(|_| GeomBatch::new()),
            ),
            mapping,
        )
    }
}

struct MatrixOptions<X, Y> {
    // To disambiguate labels
    title: String,
    total_width: f64,
    total_height: f64,
    // (lower_bound, upper_bound) -> Cell Label
    fmt_y_axis: Box<dyn Fn(Y, Y) -> Text>,
    color_scale_for_bucket: Box<dyn Fn(&App, X, Y) -> &ColorScale>,
    tooltip_for_bucket: Box<dyn Fn((Option<X>, Option<X>), (Y, Y), usize) -> Text>,
}

fn bucketize_isizes(max_buckets: usize, pts: &[(TripID, Duration, isize)]) -> Vec<isize> {
    debug_assert!(
        max_buckets % 2 == 1,
        "num_buckets must be odd to have a symmetrical number of buckets around axis"
    );
    debug_assert!(max_buckets >= 3, "num_buckets must be at least 3");

    let positive_buckets = (max_buckets - 1) / 2;
    // uniformly sized integer buckets
    let max = match pts.iter().max_by_key(|(_, _, cnt)| cnt.abs()) {
        Some(t) if (t.2.abs() as usize) >= positive_buckets => t.2.abs(),
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
            &vec![
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
        let buckets = bucketize_isizes(7, &vec![]);
        assert_eq!(buckets, vec![-2, -1, 0, 1, 2])
    }

    #[test]
    fn test_bucketize_small_isizes() {
        let buckets = bucketize_isizes(
            7,
            &vec![
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
