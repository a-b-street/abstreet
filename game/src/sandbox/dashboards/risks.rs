use std::collections::BTreeSet;
use std::fmt::Display;

use abstutil::prettyprint_usize;
use geom::{Duration, Polygon, Pt2D, Time};
use map_gui::tools::ColorScale;
use sim::{Problem, TripMode};
use widgetry::{
    DrawWithTooltips, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, State, Text, TextExt,
    Toggle, Widget,
};

use crate::app::{App, Transition};
use crate::common::color_for_mode;
use crate::sandbox::dashboards::DashTab;

pub struct RiskSummaries {
    panel: Panel,
}

impl RiskSummaries {
    pub fn new(ctx: &mut EventCtx, app: &App, filter: Filter) -> Box<dyn State<App>> {
        let mut filters = Vec::new();
        for mode in TripMode::all() {
            filters.push(Toggle::colored_checkbox(
                ctx,
                mode.ongoing_verb(),
                color_for_mode(app, mode),
                filter.modes.contains(&mode),
            ));
        }

        Box::new(RiskSummaries {
            panel: Panel::new(Widget::col(vec![
                DashTab::RiskSummaries.picker(ctx, app),
                Widget::col(vec![
                    "Filters".text_widget(ctx),
                    Widget::row(filters),
                    Toggle::checkbox(
                        ctx,
                        "include trips without any changes",
                        None,
                        filter.include_no_changes,
                    ),
                ])
                .section(ctx),
                Widget::row(vec![
                    Widget::col(vec![
                        "Delays at an intersection".text_widget(ctx),
                        safety_matrix(ctx, app, &filter, ProblemType::IntersectionDelay),
                    ])
                    .section(ctx),
                    Widget::col(vec![
                        "Large intersection crossings".text_widget(ctx),
                        safety_matrix(ctx, app, &filter, ProblemType::LargeIntersectionCrossing),
                    ])
                    .section(ctx),
                    Widget::col(vec![
                        "Cars wanting to over-take cyclists".text_widget(ctx),
                        safety_matrix(ctx, app, &filter, ProblemType::OvertakeDesired),
                    ])
                    .section(ctx),
                ])
                .evenly_spaced(),
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
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                if let Some(t) = DashTab::RiskSummaries.transition(ctx, app, &self.panel) {
                    return t;
                }

                let mut filter = Filter {
                    modes: BTreeSet::new(),
                    include_no_changes: self.panel.is_checked("include trips without any changes"),
                };
                for m in TripMode::all() {
                    if self.panel.is_checked(m.ongoing_verb()) {
                        filter.modes.insert(m);
                    }
                }
                Transition::Replace(RiskSummaries::new(ctx, app, filter))
            }
            _ => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _app: &App) {
        self.panel.draw(g);
    }
}

fn safety_matrix(
    ctx: &mut EventCtx,
    app: &App,
    filter: &Filter,
    problem_type: ProblemType,
) -> Widget {
    let points = filter.get_trips(app, problem_type);
    if points.is_empty() {
        return Widget::nothing();
    }

    let num_buckets = 10;
    let mut matrix = Matrix::new(
        bucketize_duration(num_buckets, &points),
        bucketize_isizes(num_buckets, &points),
    );
    for (x, y) in points {
        matrix.add_pt(x, y);
    }
    matrix.draw(
        ctx,
        app,
        MatrixOptions {
            total_width: 500.0,
            total_height: 500.0,
            color_scale_for_bucket: Box::new(|app, _, n| {
                if n <= 0 {
                    &app.cs.good_to_bad_green
                } else {
                    &app.cs.good_to_bad_red
                }
            }),
            tooltip_for_bucket: Box::new(|(t1, t2), (problems1, problems2), count| {
                let mut txt = Text::from(Line(format!("Trips between {} and {}", t1, t2)));
                txt.add_line(if problems1 == 0 || problems2 == 0 {
                    Line("with no changes in number of problems encountered")
                } else if problems1 < 0 {
                    Line(format!(
                        "with between {} and {} less problems encountered",
                        -problems2, -problems1
                    ))
                } else {
                    Line(format!(
                        "with between {} and {} more problems encountered",
                        problems1, problems2
                    ))
                });
                txt.add_line(Line(format!("Count: {} trips", prettyprint_usize(count))));
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
    fn count(self, problems: &Vec<(Time, Problem)>) -> usize {
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
    pub fn new() -> Filter {
        Filter {
            modes: TripMode::all().into_iter().collect(),
            include_no_changes: false,
        }
    }

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
                let count = self.counts[self.idx(x, y)];
                // TODO Different colors for better/worse? Or are we just showing density?
                let density_pct = (count as f64) / max_count;
                let color =
                    (opts.color_scale_for_bucket)(app, self.buckets_x[x], self.buckets_y[y])
                        .eval(density_pct);
                let x1 = cell_width * (x as f64);
                let y1 = cell_height * (y as f64);
                let rect = cell.clone().translate(x1, y1);
                batch.push(color, rect.clone());
                batch.append(
                    Text::from(Line(prettyprint_usize(count)))
                        .render(ctx)
                        .centered_on(Pt2D::new(x1 + cell_width / 2.0, y1 + cell_height / 2.0)),
                );
                tooltips.push((
                    rect,
                    (opts.tooltip_for_bucket)(
                        (self.buckets_x[x], self.buckets_x[x + 1]),
                        (self.buckets_y[y], self.buckets_y[y + 1]),
                        count,
                    ),
                ));
            }
        }

        DrawWithTooltips::new(ctx, batch, tooltips, Box::new(|_| GeomBatch::new()))
    }
}

struct MatrixOptions<X, Y> {
    total_width: f64,
    total_height: f64,
    color_scale_for_bucket: Box<dyn Fn(&App, X, Y) -> &ColorScale>,
    tooltip_for_bucket: Box<dyn Fn((X, X), (Y, Y), usize) -> Text>,
}

fn bucketize_duration(num_buckets: usize, pts: &Vec<(Duration, isize)>) -> Vec<Duration> {
    let max = pts.iter().max_by_key(|(dt, _)| *dt).unwrap().0;
    let (_, mins) = max.make_intervals_for_max(num_buckets);
    mins.into_iter().map(|x| Duration::minutes(x)).collect()
}

fn bucketize_isizes(num_buckets: usize, pts: &Vec<(Duration, isize)>) -> Vec<isize> {
    let min = pts.iter().min_by_key(|(_, cnt)| *cnt).unwrap().1;
    let max = pts.iter().max_by_key(|(_, cnt)| *cnt).unwrap().1;
    // TODO Rounding is wrong. We need to make sure to cover the min/max range...
    let step_size = ((max - min).abs() as f64) / (num_buckets as f64);
    let mut buckets = Vec::new();
    for i in 0..num_buckets {
        buckets.push(min + ((i as f64) * step_size) as isize);
    }
    buckets
}
