use crate::layout::Widget;
use crate::{
    Color, DrawBoth, EventCtx, GeomBatch, GfxCtx, Line, ManagedWidget, ScreenDims, ScreenPt,
    ScreenRectangle, Text,
};
use abstutil::prettyprint_usize;
use geom::{Bounds, Circle, Distance, Duration, FindClosest, PolyLine, Pt2D, Time};

// The X is always time
pub struct Plot<T> {
    draw: DrawBoth,

    // The geometry here is in screen-space.
    max_x: Time,
    max_y: Box<dyn Yvalue<T>>,
    closest: FindClosest<String>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl<T: 'static + Ord + PartialEq + Copy + core::fmt::Debug + Yvalue<T>> Plot<T> {
    // TODO I want to store y_zero in the trait, but then we can't Box max_y.
    // Returns (plot, legend, X axis labels, Y axis labels)
    fn new(
        series: Vec<Series<T>>,
        y_zero: T,
        ctx: &EventCtx,
    ) -> (Plot<T>, ManagedWidget, ManagedWidget, ManagedWidget) {
        let mut batch = GeomBatch::new();

        let width = 0.3 * ctx.canvas.window_width;
        let height = 0.2 * ctx.canvas.window_height;

        let radius = 15.0;
        let legend = ManagedWidget::col(
            series
                .iter()
                .map(|s| {
                    ManagedWidget::row(vec![
                        ManagedWidget::draw_batch(
                            ctx,
                            GeomBatch::from(vec![(
                                s.color,
                                Circle::new(Pt2D::new(radius, radius), Distance::meters(radius))
                                    .to_polygon(),
                            )]),
                        ),
                        ManagedWidget::draw_text(ctx, Text::from(Line(&s.label))),
                    ])
                })
                .collect(),
        );

        // Assume min_x is Time::START_OF_DAY and min_y is y_zero
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

        // Grid lines for the Y scale. Draw up to 10 lines max to cover the order of magnitude of
        // the range.
        // TODO This caps correctly, but if the max is 105, then suddenly we just have 2 grid
        // lines.
        {
            let order_of_mag = 10.0_f64.powf(max_y.to_f64().log10().ceil());
            for i in 0..10 {
                let y = max_y.from_f64(order_of_mag / 10.0 * (i as f64));
                let pct = y.to_percent(max_y);
                if pct > 1.0 {
                    break;
                }
                batch.push(
                    Color::BLACK,
                    PolyLine::new(vec![
                        Pt2D::new(0.0, (1.0 - pct) * height),
                        Pt2D::new(width, (1.0 - pct) * height),
                    ])
                    .make_polygons(Distance::meters(5.0)),
                );
            }
        }
        // X axis grid
        if max_x != Time::START_OF_DAY {
            let order_of_mag = 10.0_f64.powf(max_x.inner_seconds().log10().ceil());
            for i in 0..10 {
                let x = Time::START_OF_DAY + Duration::seconds(order_of_mag / 10.0 * (i as f64));
                let pct = x.to_percent(max_x);
                if pct > 1.0 {
                    break;
                }
                batch.push(
                    Color::BLACK,
                    PolyLine::new(vec![
                        Pt2D::new(pct * width, 0.0),
                        Pt2D::new(pct * width, height),
                    ])
                    .make_polygons(Distance::meters(5.0)),
                );
            }
        }

        let mut closest = FindClosest::new(&Bounds::from(&vec![
            Pt2D::new(0.0, 0.0),
            Pt2D::new(width, height),
        ]));
        for s in series {
            if max_x == Time::START_OF_DAY {
                continue;
            }
            let mut pts = Vec::new();
            for (t, y) in s.pts {
                let percent_x = t.to_percent(max_x);
                let percent_y = y.to_percent(max_y);
                pts.push(Pt2D::new(
                    percent_x * width,
                    // Y inversion! :D
                    (1.0 - percent_y) * height,
                ));
            }
            pts.dedup();
            if pts.len() >= 2 {
                closest.add(s.label.clone(), &pts);
                batch.push(
                    s.color,
                    PolyLine::new(pts)
                        .make_polygons_with_miter_threshold(Distance::meters(5.0), 10.0),
                );
            }
        }

        let plot = Plot {
            draw: DrawBoth::new(ctx, batch, Vec::new()),
            closest,
            max_x,
            max_y: Box::new(max_y),

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(width, height),
        };

        let num_x_labels = 4;
        let mut row = Vec::new();
        for i in 0..num_x_labels {
            let percent_x = (i as f64) / ((num_x_labels - 1) as f64);
            let t = max_x.percent_of(percent_x);
            row.push(ManagedWidget::draw_text(
                ctx,
                Text::from(Line(t.to_string())),
            ));
        }
        let x_axis = ManagedWidget::row(row);

        let num_y_labels = 4;
        let mut col = Vec::new();
        for i in 0..num_y_labels {
            let percent_y = (i as f64) / ((num_y_labels - 1) as f64);
            col.push(ManagedWidget::draw_text(
                ctx,
                Text::from(Line(max_y.from_percent(percent_y).prettyprint())),
            ));
        }
        col.reverse();
        let y_axis = ManagedWidget::col(col);

        (plot, legend, x_axis, y_axis)
    }

    pub(crate) fn draw(&self, g: &mut GfxCtx) {
        self.draw.redraw(self.top_left, g);

        if let Some(cursor) = g.canvas.get_cursor_in_screen_space() {
            if ScreenRectangle::top_left(self.top_left, self.dims).contains(cursor) {
                let radius = Distance::meters(5.0);
                let mut txt = Text::new().bg(Color::grey(0.6));
                for (label, pt, _) in self.closest.all_close_pts(
                    Pt2D::new(cursor.x - self.top_left.x, cursor.y - self.top_left.y),
                    radius,
                ) {
                    let t = self.max_x.percent_of(pt.x() / self.dims.width);
                    let y_percent = 1.0 - (pt.y() / self.dims.height);

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
                    g.draw_mouse_tooltip(txt);
                    g.unfork();
                }
            }
        }
    }
}

impl Plot<usize> {
    pub fn new_usize(series: Vec<Series<usize>>, ctx: &EventCtx) -> ManagedWidget {
        let (plot, legend, x_axis, y_axis) = Plot::new(series, 0, ctx);
        // Don't let the x-axis fill the parent container
        ManagedWidget::row(vec![ManagedWidget::col(vec![
            legend,
            ManagedWidget::row(vec![
                y_axis.evenly_spaced(),
                ManagedWidget::usize_plot(plot),
            ]),
            x_axis.evenly_spaced(),
        ])])
    }
}

impl Plot<Duration> {
    pub fn new_duration(series: Vec<Series<Duration>>, ctx: &EventCtx) -> ManagedWidget {
        let (plot, legend, x_axis, y_axis) = Plot::new(series, Duration::ZERO, ctx);
        // Don't let the x-axis fill the parent container
        ManagedWidget::row(vec![ManagedWidget::col(vec![
            legend,
            ManagedWidget::row(vec![
                y_axis.evenly_spaced(),
                ManagedWidget::duration_plot(plot),
            ]),
            x_axis.evenly_spaced(),
        ])])
    }
}

impl<T> Widget for Plot<T> {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}

pub trait Yvalue<T> {
    // percent is [0.0, 1.0]
    fn from_percent(&self, percent: f64) -> T;
    fn to_percent(self, max: T) -> f64;
    fn prettyprint(self) -> String;
    // For order of magnitude calculations
    fn to_f64(self) -> f64;
    fn from_f64(&self, x: f64) -> T;
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
    fn to_f64(self) -> f64 {
        self as f64
    }
    fn from_f64(&self, x: f64) -> usize {
        x as usize
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
    fn to_f64(self) -> f64 {
        self.inner_seconds() as f64
    }
    fn from_f64(&self, x: f64) -> Duration {
        Duration::seconds(x as f64)
    }
}

pub struct Series<T> {
    pub label: String,
    pub color: Color,
    // X-axis is time. Assume this is sorted by X.
    pub pts: Vec<(Time, T)>,
}
