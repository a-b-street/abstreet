use crate::common::ColorLegend;
use abstutil::prettyprint_usize;
use ezgui::{Color, DrawBoth, EventCtx, GeomBatch, GfxCtx, Line, ScreenPt, ScreenRectangle, Text};
use geom::{Bounds, Circle, Distance, Duration, FindClosest, PolyLine, Polygon, Pt2D, Time};

// The X is always time
pub struct Plot<T> {
    draw: DrawBoth,
    legend: ColorLegend,
    rect: ScreenRectangle,

    // The geometry here is in screen-space.
    max_x: Time,
    max_y: Box<dyn Yvalue<T>>,
    closest: FindClosest<String>,
}

impl<T: 'static + Ord + PartialEq + Copy + core::fmt::Debug + Yvalue<T>> Plot<T> {
    // TODO I want to store y_zero in the trait, but then we can't Box max_y.
    pub fn new(title: &str, series: Vec<Series<T>>, y_zero: T, ctx: &EventCtx) -> Plot<T> {
        let mut batch = GeomBatch::new();
        let mut labels: Vec<(Text, ScreenPt)> = Vec::new();

        let x1 = 0.1 * ctx.canvas.window_width;
        let x2 = 0.7 * ctx.canvas.window_width;
        let y1 = 0.2 * ctx.canvas.window_height;
        let y2 = 0.8 * ctx.canvas.window_height;
        batch.push(
            Color::grey(0.8),
            Polygon::rectangle_topleft(
                Pt2D::new(x1, y1),
                Distance::meters(x2 - x1),
                Distance::meters(y2 - y1),
            ),
        );

        // Assume min_x is Time::START_OF_DAY and min_y is 0
        let max_x = series
            .iter()
            .map(|s| {
                s.pts
                    .iter()
                    .map(|(t, _)| *t)
                    .max()
                    .unwrap_or(Time::START_OF_DAY)
            })
            .max()
            .unwrap_or(Time::START_OF_DAY);
        let max_y = series
            .iter()
            .map(|s| {
                s.pts
                    .iter()
                    .map(|(_, value)| *value)
                    .max()
                    .unwrap_or(y_zero)
            })
            .max()
            .unwrap_or(y_zero);

        let num_x_labels = 5;
        for i in 0..num_x_labels {
            let percent_x = (i as f64) / ((num_x_labels - 1) as f64);
            let t = max_x.percent_of(percent_x);
            labels.push((
                Text::from(Line(t.to_string())),
                ScreenPt::new(x1 + percent_x * (x2 - x1), y2),
            ));
        }

        let num_y_labels = 5;
        for i in 0..num_y_labels {
            let percent_y = (i as f64) / ((num_y_labels - 1) as f64);
            labels.push((
                Text::from(Line(max_y.from_percent(percent_y).prettyprint())),
                ScreenPt::new(x1, y2 - percent_y * (y2 - y1)),
            ));
        }

        let legend = ColorLegend::new(
            Text::prompt(title),
            series.iter().map(|s| (s.label.as_str(), s.color)).collect(),
        );

        let mut closest =
            FindClosest::new(&Bounds::from(&vec![Pt2D::new(x1, y1), Pt2D::new(x2, y2)]));
        for s in series {
            if max_x == Time::START_OF_DAY {
                continue;
            }
            let mut pts = Vec::new();
            for (t, y) in s.pts {
                let percent_x = t.to_percent(max_x);
                let percent_y = y.to_percent(max_y);
                pts.push(Pt2D::new(
                    x1 + (x2 - x1) * percent_x,
                    // Y inversion! :D
                    y2 - (y2 - y1) * percent_y,
                ));
            }
            pts.dedup();
            if pts.len() >= 2 {
                closest.add(s.label.clone(), &pts);
                batch.push(
                    s.color,
                    PolyLine::new(pts).make_polygons(Distance::meters(5.0)),
                );
            }
        }

        Plot {
            draw: DrawBoth::new(ctx, batch, labels),
            legend,
            rect: ScreenRectangle { x1, y1, x2, y2 },
            closest,
            max_x,
            max_y: Box::new(max_y),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.canvas.mark_covered_area(self.rect.clone());

        self.draw.redraw(ScreenPt::new(0.0, 0.0), g);

        let cursor = g.canvas.get_cursor_in_screen_space();
        if self.rect.contains(cursor) {
            let radius = Distance::meters(5.0);
            let mut txt = Text::new();
            for (label, pt, _) in self.closest.all_close_pts(cursor.to_pt(), radius) {
                let t = self
                    .max_x
                    .percent_of((pt.x() - self.rect.x1) / self.rect.width());
                let y_percent = 1.0 - (pt.y() - self.rect.y1) / self.rect.height();

                // TODO Draw this info in the ColorLegend
                txt.add(Line(format!(
                    "{}: at {}, {}",
                    label,
                    t,
                    self.max_y.from_percent(y_percent).prettyprint()
                )));
            }
            if txt.num_lines() > 0 {
                g.fork_screenspace();
                g.draw_circle(Color::RED, &Circle::new(cursor.to_pt(), radius));
                g.draw_mouse_tooltip(&txt);
                g.unfork();
            }
        }

        self.legend.draw(g);
    }
}

pub trait Yvalue<T> {
    // percent is [0.0, 1.0]
    fn from_percent(&self, percent: f64) -> T;
    fn to_percent(self, max: T) -> f64;
    fn prettyprint(self) -> String;
}

impl Yvalue<usize> for usize {
    fn from_percent(&self, percent: f64) -> usize {
        ((*self as f64) * percent) as usize
    }
    fn to_percent(self, max: usize) -> f64 {
        if max == 0 {
            0.0
        } else {
            (self as f64) / (max as f64)
        }
    }
    fn prettyprint(self) -> String {
        prettyprint_usize(self)
    }
}
impl Yvalue<Duration> for Duration {
    fn from_percent(&self, percent: f64) -> Duration {
        *self * percent
    }
    fn to_percent(self, max: Duration) -> f64 {
        if max == Duration::ZERO {
            0.0
        } else {
            self / max
        }
    }
    fn prettyprint(self) -> String {
        self.to_string()
    }
}

pub struct Series<T> {
    pub label: String,
    pub color: Color,
    // X-axis is time. Assume this is sorted by X.
    pub pts: Vec<(Time, T)>,
}

// The X axis is Durations, with positive meaning "faster" (considered good) and negative "slower"
pub struct Histogram {
    draw: DrawBoth,
    rect: ScreenRectangle,

    // TODO Bit sad to pretty much duplicate the geometry from DrawBoth...
    rect_labels: Vec<(Polygon, Text)>,
}

impl Histogram {
    pub fn new(unsorted_dts: Vec<Duration>, ctx: &EventCtx) -> Histogram {
        let mut batch = GeomBatch::new();
        let mut labels: Vec<(Text, ScreenPt)> = Vec::new();
        let mut rect_labels = Vec::new();

        let x1 = 0.7 * ctx.canvas.window_width;
        let x2 = 0.95 * ctx.canvas.window_width;
        let y1 = 0.6 * ctx.canvas.window_height;
        let y2 = 0.9 * ctx.canvas.window_height;
        batch.push(
            Color::grey(0.8),
            Polygon::rectangle_topleft(
                Pt2D::new(x1, y1),
                Distance::meters(x2 - x1),
                Distance::meters(y2 - y1),
            ),
        );

        if unsorted_dts.is_empty() {
            labels.push((
                Text::from(Line("not enough data yet")),
                ScreenPt::new(x1 + (x2 - x1) * 0.1, y1 + (y2 - y1) * 0.1),
            ));
        } else {
            // TODO Generic "bucket into 10 groups, give (min, max, count)"
            let min_x = *unsorted_dts.iter().min().unwrap();
            let max_x = *unsorted_dts.iter().max().unwrap();

            let num_buckets = 10;
            let bucket_size = (max_x - min_x) / (num_buckets as f64);
            // lower, upper, count
            let mut bars: Vec<(Duration, Duration, usize)> = (0..num_buckets)
                .map(|idx| {
                    let i = idx as f64;
                    (min_x + bucket_size * i, min_x + bucket_size * (i + 1.0), 0)
                })
                .collect();
            for dt in unsorted_dts {
                // TODO Could sort them and do this more efficiently.
                if dt == max_x {
                    // Most bars represent [low, high) except the last
                    bars[num_buckets - 1].2 += 1;
                } else {
                    let bin = ((dt - min_x) / bucket_size).floor() as usize;
                    bars[bin].2 += 1;
                }
            }

            let min_y = 0;
            let max_y = bars.iter().map(|(_, _, cnt)| *cnt).max().unwrap();
            for (idx, (min, max, cnt)) in bars.into_iter().enumerate() {
                let color = if max < Duration::ZERO {
                    Color::RED
                } else if min < Duration::ZERO {
                    Color::YELLOW
                } else {
                    Color::GREEN
                };
                let percent_x_left = (idx as f64) / (num_buckets as f64);
                let percent_x_right = ((idx + 1) as f64) / (num_buckets as f64);
                if let Some(rect) = Polygon::rectangle_two_corners(
                    // Top-left
                    Pt2D::new(
                        x1 + (x2 - x1) * percent_x_left,
                        y2 - (y2 - y1) * ((cnt as f64) / ((max_y - min_y) as f64)),
                    ),
                    // Bottom-right
                    Pt2D::new(x1 + (x2 - x1) * percent_x_right, y2),
                ) {
                    batch.push(color, rect.clone());
                    rect_labels.push((
                        rect,
                        Text::from(Line(format!(
                            "[{}, {}) has {} trips",
                            min,
                            max,
                            prettyprint_usize(cnt)
                        ))),
                    ));
                }
            }

            // TODO These can still get really squished. Draw rotated?
            let num_x_labels = 3;
            for i in 0..num_x_labels {
                let percent_x = (i as f64) / ((num_x_labels - 1) as f64);
                let dt = min_x + (max_x - min_x) * percent_x;
                labels.push((
                    Text::from(Line(dt.to_string())),
                    ScreenPt::new(x1 + percent_x * (x2 - x1), y2),
                ));
            }

            let num_y_labels = 5;
            for i in 0..num_y_labels {
                let percent_y = (i as f64) / ((num_y_labels - 1) as f64);
                // TODO Better alignment...
                let left_px = 30.0;
                labels.push((
                    Text::from(Line(prettyprint_usize(
                        ((max_y as f64) * percent_y) as usize,
                    ))),
                    ScreenPt::new(x1 - left_px, y2 - percent_y * (y2 - y1)),
                ));
            }
        }

        Histogram {
            draw: DrawBoth::new(ctx, batch, labels),
            rect: ScreenRectangle { x1, y1, x2, y2 },
            rect_labels,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.canvas.mark_covered_area(self.rect.clone());

        self.draw.redraw(ScreenPt::new(0.0, 0.0), g);

        let cursor = g.canvas.get_cursor_in_screen_space();
        if self.rect.contains(cursor) {
            for (rect, lbl) in &self.rect_labels {
                if rect.contains_pt(cursor.to_pt()) {
                    g.draw_mouse_tooltip(lbl);
                }
            }
        }
    }
}
