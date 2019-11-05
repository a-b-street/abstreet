use crate::common::ColorLegend;
use ezgui::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, MultiText, ScreenPt, ScreenRectangle, Text,
};
use geom::{Distance, Duration, PolyLine, Polygon, Pt2D};

pub struct Plot {
    draw: Drawable,
    legend: ColorLegend,
    labels: MultiText,
    rect: ScreenRectangle,
}

impl Plot {
    pub fn new<T: Ord + PartialEq + Copy + Yvalue<T>>(
        title: &str,
        series: Vec<Series<T>>,
        ctx: &EventCtx,
    ) -> Option<Plot> {
        let mut batch = GeomBatch::new();
        let mut labels = MultiText::new();

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

        // Assume min_x is Duration::ZERO and min_y is 0
        let max_x = series
            .iter()
            .map(|s| s.pts.iter().map(|(t, _)| *t).max().unwrap())
            .max()
            .unwrap();
        let max_y = series
            .iter()
            .map(|s| s.pts.iter().map(|(_, cnt)| *cnt).max().unwrap())
            .max()
            .unwrap();
        if max_x == Duration::ZERO {
            return None;
        }

        let num_x_labels = 5;
        for i in 0..num_x_labels {
            let percent_x = (i as f64) / ((num_x_labels - 1) as f64);
            let t = percent_x * max_x;
            labels.add(
                Text::from(Line(t.to_string())),
                ScreenPt::new(x1 + percent_x * (x2 - x1), y2),
            );
        }

        let num_y_labels = 5;
        for i in 0..num_y_labels {
            let percent_y = (i as f64) / ((num_y_labels - 1) as f64);
            labels.add(
                Text::from(Line(max_y.from_percent(percent_y).prettyprint())),
                ScreenPt::new(x1, y2 - percent_y * (y2 - y1)),
            );
        }

        let legend = ColorLegend::new(
            Text::prompt(title),
            series.iter().map(|s| (s.label.as_str(), s.color)).collect(),
        );

        for s in series {
            let mut pts = Vec::new();
            if max_y == T::zero() {
                pts.push(Pt2D::new(x1, y2));
                pts.push(Pt2D::new(x2, y2));
            } else {
                for (t, y) in s.pts {
                    let percent_x = t / max_x;
                    let percent_y = y.to_percent(max_y);
                    pts.push(Pt2D::new(
                        x1 + (x2 - x1) * percent_x,
                        // Y inversion! :D
                        y2 - (y2 - y1) * percent_y,
                    ));
                }
            }
            batch.push(
                s.color,
                PolyLine::new(pts).make_polygons(Distance::meters(5.0)),
            );
        }

        Some(Plot {
            draw: ctx.prerender.upload(batch),
            labels,
            legend,
            rect: ScreenRectangle { x1, y1, x2, y2 },
        })
    }
    pub fn draw(&self, g: &mut GfxCtx) {
        self.legend.draw(g);

        g.fork_screenspace();
        g.redraw(&self.draw);
        g.unfork();
        self.labels.draw(g);

        g.canvas.mark_covered_area(self.rect.clone());
    }
}

pub trait Yvalue<T> {
    // percent is [0.0, 1.0]
    fn from_percent(self, percent: f64) -> T;
    fn to_percent(self, max: T) -> f64;
    fn prettyprint(self) -> String;
    fn zero() -> T;
}

impl Yvalue<usize> for usize {
    fn from_percent(self, percent: f64) -> usize {
        ((self as f64) * percent) as usize
    }
    fn to_percent(self, max: usize) -> f64 {
        (self as f64) / (max as f64)
    }
    fn prettyprint(self) -> String {
        abstutil::prettyprint_usize(self)
    }
    fn zero() -> usize {
        0
    }
}
impl Yvalue<Duration> for Duration {
    fn from_percent(self, percent: f64) -> Duration {
        percent * self
    }
    fn to_percent(self, max: Duration) -> f64 {
        self / max
    }
    fn prettyprint(self) -> String {
        self.minimal_tostring()
    }
    fn zero() -> Duration {
        Duration::ZERO
    }
}

pub struct Series<T> {
    pub label: String,
    pub color: Color,
    // X-axis is time. Assume this is sorted by X.
    pub pts: Vec<(Duration, T)>,
}
