use crate::{
    Checkbox, Color, Drawable, EventCtx, GeomBatch, GfxCtx, JustDraw, Line, ScreenDims, ScreenPt,
    ScreenRectangle, Text, TextExt, Widget, WidgetImpl, WidgetOutput,
};
use abstutil::prettyprint_usize;
use geom::{Angle, Bounds, Circle, Distance, Duration, FindClosest, PolyLine, Polygon, Pt2D, Time};
use std::collections::HashSet;

// The X is always time
pub struct LinePlot<T: Yvalue<T>> {
    draw: Drawable,

    // The geometry here is in screen-space.
    max_x: Time,
    max_y: T,
    closest: FindClosest<String>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

pub struct PlotOptions<T: Yvalue<T>> {
    pub filterable: bool,
    pub max_x: Option<Time>,
    pub max_y: Option<T>,
    pub disabled: HashSet<String>,
}

impl<T: Yvalue<T>> PlotOptions<T> {
    pub fn filterable() -> PlotOptions<T> {
        PlotOptions {
            filterable: true,
            max_x: None,
            max_y: None,
            disabled: HashSet::new(),
        }
    }

    pub fn fixed() -> PlotOptions<T> {
        PlotOptions {
            filterable: false,
            max_x: None,
            max_y: None,
            disabled: HashSet::new(),
        }
    }
}

impl<T: Yvalue<T>> LinePlot<T> {
    pub fn new(ctx: &EventCtx, mut series: Vec<Series<T>>, opts: PlotOptions<T>) -> Widget {
        let legend = make_legend(ctx, &series, &opts);
        series.retain(|s| !opts.disabled.contains(&s.label));

        // Assume min_x is Time::START_OF_DAY and min_y is T::zero()
        let max_x = opts.max_x.unwrap_or_else(|| {
            series
                .iter()
                .map(|s| {
                    s.pts
                        .iter()
                        .map(|(t, _)| *t)
                        .max()
                        .unwrap_or(Time::START_OF_DAY)
                })
                .max()
                .unwrap_or(Time::START_OF_DAY)
        });
        let max_y = opts.max_y.unwrap_or_else(|| {
            series
                .iter()
                .map(|s| {
                    s.pts
                        .iter()
                        .map(|(_, value)| *value)
                        .max()
                        .unwrap_or(T::zero())
                })
                .max()
                .unwrap_or(T::zero())
        });

        // TODO Tuned to fit the info panel. Instead these should somehow stretch to fill their
        // container.
        let width = 0.23 * ctx.canvas.window_width;
        let height = 0.2 * ctx.canvas.window_height;

        let mut batch = GeomBatch::new();
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
                    Color::hex("#7C7C7C"),
                    PolyLine::new(vec![
                        Pt2D::new(0.0, (1.0 - pct) * height),
                        Pt2D::new(width, (1.0 - pct) * height),
                    ])
                    .make_polygons(Distance::meters(1.0)),
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
                    Color::hex("#7C7C7C"),
                    PolyLine::new(vec![
                        Pt2D::new(pct * width, 0.0),
                        Pt2D::new(pct * width, height),
                    ])
                    .make_polygons(Distance::meters(1.0)),
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
                batch.push(s.color, thick_lineseries(pts, Distance::meters(5.0)));
            }
        }

        let plot = LinePlot {
            draw: ctx.upload(batch),
            closest,
            max_x,
            max_y,

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(width, height),
        };

        let num_x_labels = 3;
        let mut row = Vec::new();
        for i in 0..num_x_labels {
            let percent_x = (i as f64) / ((num_x_labels - 1) as f64);
            let t = max_x.percent_of(percent_x);
            // TODO Need ticks now to actually see where this goes
            let batch = Text::from(Line(t.to_string()))
                .render_ctx(ctx)
                .rotate(Angle::new_degs(-15.0))
                .autocrop();
            // The text is already scaled; don't use Widget::draw_batch and scale it again.
            row.push(JustDraw::wrap(ctx, batch));
        }
        let x_axis = Widget::custom_row(row).padding(10).evenly_spaced();

        let num_y_labels = 4;
        let mut col = Vec::new();
        for i in 0..num_y_labels {
            let percent_y = (i as f64) / ((num_y_labels - 1) as f64);
            col.push(max_y.from_percent(percent_y).prettyprint().draw_text(ctx));
        }
        col.reverse();
        let y_axis = Widget::custom_col(col).padding(10).evenly_spaced();

        // Don't let the x-axis fill the parent container
        Widget::custom_col(vec![
            legend.margin_below(10),
            Widget::custom_row(vec![y_axis, Widget::new(Box::new(plot))]),
            x_axis,
        ])
        .container()
    }
}

impl<T: Yvalue<T>> WidgetImpl for LinePlot<T> {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, _ctx: &mut EventCtx, _output: &mut WidgetOutput) {}

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);

        if let Some(cursor) = g.canvas.get_cursor_in_screen_space() {
            if ScreenRectangle::top_left(self.top_left, self.dims).contains(cursor) {
                let radius = Distance::meters(15.0);
                let mut txt = Text::new();
                for (label, pt, _) in self.closest.all_close_pts(
                    Pt2D::new(cursor.x - self.top_left.x, cursor.y - self.top_left.y),
                    radius,
                ) {
                    // TODO If some/all of the matches have the same t, write it once?
                    let t = self.max_x.percent_of(pt.x() / self.dims.width);
                    let y_percent = 1.0 - (pt.y() / self.dims.height);

                    // TODO Draw this info in the ColorLegend
                    txt.add(Line(format!(
                        "{}: at {}, {}",
                        label,
                        t.ampm_tostring(),
                        self.max_y.from_percent(y_percent).prettyprint()
                    )));
                }
                if !txt.is_empty() {
                    g.fork_screenspace();
                    g.draw_circle(Color::RED, &Circle::new(cursor.to_pt(), radius));
                    g.draw_mouse_tooltip(txt);
                    g.unfork();
                }
            }
        }
    }
}

pub trait Yvalue<T>: 'static + Copy + std::cmp::Ord {
    // percent is [0.0, 1.0]
    fn from_percent(&self, percent: f64) -> T;
    fn to_percent(self, max: T) -> f64;
    fn prettyprint(self) -> String;
    // For order of magnitude calculations
    fn to_f64(self) -> f64;
    fn from_f64(&self, x: f64) -> T;
    fn zero() -> T;
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
    fn zero() -> usize {
        0
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
    fn zero() -> Duration {
        Duration::ZERO
    }
}

pub struct Series<T> {
    pub label: String,
    pub color: Color,
    // X-axis is time. Assume this is sorted by X.
    pub pts: Vec<(Time, T)>,
}

pub fn make_legend<T: Yvalue<T>>(
    ctx: &EventCtx,
    series: &Vec<Series<T>>,
    opts: &PlotOptions<T>,
) -> Widget {
    let mut row = Vec::new();
    let mut seen = HashSet::new();
    for s in series {
        if seen.contains(&s.label) {
            continue;
        }
        seen.insert(s.label.clone());
        if opts.filterable {
            row.push(Widget::row(vec![
                Checkbox::colored(ctx, &s.label, s.color, !opts.disabled.contains(&s.label)),
                Line(&s.label).draw(ctx),
            ]));
        } else {
            let radius = 15.0;
            row.push(Widget::row(vec![
                Widget::draw_batch(
                    ctx,
                    GeomBatch::from(vec![(
                        s.color,
                        Circle::new(Pt2D::new(radius, radius), Distance::meters(radius))
                            .to_polygon(),
                    )]),
                ),
                s.label.clone().draw_text(ctx),
            ]));
        }
    }
    Widget::custom_row(row).flex_wrap(ctx, 24)
}

// TODO If this proves useful, lift to geom
fn thick_lineseries(pts: Vec<Pt2D>, width: Distance) -> Polygon {
    use lyon::math::{point, Point};
    use lyon::path::Path;
    use lyon::tessellation::geometry_builder::{BuffersBuilder, Positions, VertexBuffers};
    use lyon::tessellation::{StrokeOptions, StrokeTessellator};

    let mut builder = Path::builder();
    for (idx, pt) in pts.into_iter().enumerate() {
        let pt = point(pt.x() as f32, pt.y() as f32);
        if idx == 0 {
            builder.move_to(pt);
        } else {
            builder.line_to(pt);
        }
    }
    let path = builder.build();

    let mut geom: VertexBuffers<Point, u32> = VertexBuffers::new();
    let mut buffer = BuffersBuilder::new(&mut geom, Positions);
    StrokeTessellator::new()
        .tessellate(
            &path,
            &StrokeOptions::tolerance(0.01).with_line_width(width.inner_meters() as f32),
            &mut buffer,
        )
        .unwrap();
    Polygon::precomputed(
        geom.vertices
            .into_iter()
            .map(|v| Pt2D::new(f64::from(v.x), f64::from(v.y)))
            .collect(),
        geom.indices.into_iter().map(|idx| idx as usize).collect(),
    )
}
