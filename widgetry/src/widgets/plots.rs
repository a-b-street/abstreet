use std::collections::HashSet;

use abstutil::prettyprint_usize;
use geom::{Circle, Distance, Duration, Percent, Polygon, Pt2D, Time, UnitFmt};

use crate::{Color, EventCtx, GeomBatch, TextExt, Toggle, Widget};

pub struct PlotOptions<X: Axis<X>, Y: Axis<Y>> {
    pub filterable: bool,
    pub max_x: Option<X>,
    pub max_y: Option<Y>,
    pub disabled: HashSet<String>,
}

impl<X: Axis<X>, Y: Axis<Y>> PlotOptions<X, Y> {
    pub fn filterable() -> PlotOptions<X, Y> {
        PlotOptions {
            filterable: true,
            max_x: None,
            max_y: None,
            disabled: HashSet::new(),
        }
    }

    pub fn fixed() -> PlotOptions<X, Y> {
        PlotOptions {
            filterable: false,
            max_x: None,
            max_y: None,
            disabled: HashSet::new(),
        }
    }
}

pub trait Axis<T>: 'static + Copy + std::cmp::Ord {
    // percent is [0.0, 1.0]
    fn from_percent(&self, percent: f64) -> T;
    fn to_percent(self, max: T) -> f64;
    fn prettyprint(self) -> String;
    // For order of magnitude calculations
    fn to_f64(self) -> f64;
    fn from_f64(&self, x: f64) -> T;
    fn zero() -> T;
}

impl Axis<usize> for usize {
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

impl Axis<Duration> for Duration {
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
        self.to_string(&UnitFmt {
            metric: false,
            round_durations: true,
        })
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

impl Axis<Time> for Time {
    fn from_percent(&self, percent: f64) -> Time {
        self.percent_of(percent)
    }
    fn to_percent(self, max: Time) -> f64 {
        if max == Time::START_OF_DAY {
            0.0
        } else {
            self.to_percent(max)
        }
    }
    fn prettyprint(self) -> String {
        self.ampm_tostring()
    }
    fn to_f64(self) -> f64 {
        self.inner_seconds() as f64
    }
    fn from_f64(&self, x: f64) -> Time {
        Time::START_OF_DAY + Duration::seconds(x as f64)
    }
    fn zero() -> Time {
        Time::START_OF_DAY
    }
}

impl Axis<Distance> for Distance {
    fn from_percent(&self, percent: f64) -> Distance {
        *self * percent
    }
    fn to_percent(self, max: Distance) -> f64 {
        if max == Distance::ZERO {
            0.0
        } else {
            self / max
        }
    }
    fn prettyprint(self) -> String {
        self.to_string(&UnitFmt {
            metric: false,
            round_durations: true,
        })
    }
    fn to_f64(self) -> f64 {
        self.inner_meters() as f64
    }
    fn from_f64(&self, x: f64) -> Distance {
        Distance::meters(x as f64)
    }
    fn zero() -> Distance {
        Distance::ZERO
    }
}

pub struct Series<X, Y> {
    pub label: String,
    pub color: Color,
    // Assume this is sorted by X.
    pub pts: Vec<(X, Y)>,
}

pub fn make_legend<X: Axis<X>, Y: Axis<Y>>(
    ctx: &EventCtx,
    series: &[Series<X, Y>],
    opts: &PlotOptions<X, Y>,
) -> Widget {
    let mut row = Vec::new();
    let mut seen = HashSet::new();
    for s in series {
        if seen.contains(&s.label) {
            continue;
        }
        seen.insert(s.label.clone());
        if opts.filterable {
            row.push(Toggle::colored_checkbox(
                ctx,
                &s.label,
                s.color,
                !opts.disabled.contains(&s.label),
            ));
        } else {
            let radius = 15.0;
            row.push(Widget::row(vec![
                GeomBatch::from(vec![(
                    s.color,
                    Circle::new(Pt2D::new(radius, radius), Distance::meters(radius)).to_polygon(),
                )])
                .into_widget(ctx),
                s.label.clone().text_widget(ctx),
            ]));
        }
    }
    Widget::custom_row(row).flex_wrap(ctx, Percent::int(24))
}

// TODO If this proves useful, lift to geom
pub fn thick_lineseries(pts: Vec<Pt2D>, width: Distance) -> Polygon {
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
