use geom::{Circle, Distance, Line, Polygon, Pt2D, Tessellation};

use crate::{Color, EventCtx, Fill, GeomBatch, Line, LinearGradient, Text, Widget};

pub struct ColorLegend {}

impl ColorLegend {
    pub fn row(ctx: &EventCtx, color: Color, label: impl AsRef<str>) -> Widget {
        let radius = 15.0;
        Widget::row(vec![
            GeomBatch::from(vec![(
                color,
                Circle::new(Pt2D::new(radius, radius), Distance::meters(radius)).to_polygon(),
            )])
            .into_widget(ctx)
            .centered_vert(),
            Text::from(label).wrap_to_pct(ctx, 35).into_widget(ctx),
        ])
    }

    pub fn gradient_with_width<I: Into<String>>(
        ctx: &mut EventCtx,
        scale: &ColorScale,
        labels: Vec<I>,
        width: f64,
    ) -> Widget {
        assert!(scale.0.len() >= 2);
        let n = scale.0.len();
        let mut batch = GeomBatch::new();
        let width_each = width / ((n - 1) as f64);
        batch.push(
            Fill::LinearGradient(LinearGradient {
                line: Line::must_new(Pt2D::new(0.0, 0.0), Pt2D::new(width, 0.0)),
                stops: scale
                    .0
                    .iter()
                    .enumerate()
                    .map(|(idx, color)| ((idx as f64) / ((n - 1) as f64), *color))
                    .collect(),
            }),
            Tessellation::union_all(
                (0..n - 1)
                    .map(|i| {
                        Tessellation::from(
                            Polygon::rectangle(width_each, 32.0)
                                .translate((i as f64) * width_each, 0.0),
                        )
                    })
                    .collect(),
            ),
        );
        // Extra wrapping to make the labels stretch against just the scale, not everything else
        // TODO Long labels aren't nicely lined up with the boundaries between buckets
        Widget::col(vec![
            batch.into_widget(ctx),
            Widget::custom_row(
                labels
                    .into_iter()
                    .map(|lbl| Line(lbl).small().into_widget(ctx))
                    .collect(),
            )
            .evenly_spaced(),
        ])
        .container()
    }

    pub fn gradient<I: Into<String>>(
        ctx: &mut EventCtx,
        scale: &ColorScale,
        labels: Vec<I>,
    ) -> Widget {
        Self::gradient_with_width(ctx, scale, labels, 300.0)
    }

    pub fn categories(ctx: &mut EventCtx, pairs: Vec<(Color, &str)>, max: &str) -> Widget {
        assert!(pairs.len() >= 2);
        let width = 300.0;
        let n = pairs.len();
        let mut batch = GeomBatch::new();
        let width_each = width / ((n - 1) as f64);
        for (idx, (color, _)) in pairs.iter().enumerate() {
            batch.push(
                *color,
                Polygon::rectangle(width_each, 32.0).translate((idx as f64) * width_each, 0.0),
            );
        }
        // Extra wrapping to make the labels stretch against just the scale, not everything else
        // TODO Long labels aren't nicely lined up with the boundaries between buckets
        let mut labels = pairs
            .into_iter()
            .map(|(_, lbl)| Line(lbl).small().into_widget(ctx))
            .collect::<Vec<_>>();
        labels.push(Line(max).small().into_widget(ctx));
        Widget::col(vec![
            batch.into_widget(ctx),
            Widget::custom_row(labels).evenly_spaced(),
        ])
        .container()
    }
}

pub struct DivergingScale {
    low_color: Color,
    mid_color: Color,
    high_color: Color,
    min: f64,
    avg: f64,
    max: f64,
    ignore: Option<(f64, f64)>,
}

impl DivergingScale {
    pub fn new(low_color: Color, mid_color: Color, high_color: Color) -> DivergingScale {
        DivergingScale {
            low_color,
            mid_color,
            high_color,
            min: 0.0,
            avg: 0.5,
            max: 1.0,
            ignore: None,
        }
    }

    pub fn range(mut self, min: f64, max: f64) -> DivergingScale {
        assert!(min < max);
        self.min = min;
        self.avg = (min + max) / 2.0;
        self.max = max;
        self
    }

    pub fn ignore(mut self, from: f64, to: f64) -> DivergingScale {
        assert!(from < to);
        self.ignore = Some((from, to));
        self
    }

    pub fn eval(&self, value: f64) -> Option<Color> {
        let value = value.clamp(self.min, self.max);
        if let Some((from, to)) = self.ignore {
            if value >= from && value <= to {
                return None;
            }
        }
        if value <= self.avg {
            Some(
                self.low_color
                    .lerp(self.mid_color, (value - self.min) / (self.avg - self.min)),
            )
        } else {
            Some(
                self.mid_color
                    .lerp(self.high_color, (value - self.avg) / (self.max - self.avg)),
            )
        }
    }

    pub fn make_legend<I: Into<String>>(self, ctx: &mut EventCtx, labels: Vec<I>) -> Widget {
        ColorLegend::gradient(
            ctx,
            &ColorScale(vec![self.low_color, self.mid_color, self.high_color]),
            labels,
        )
    }
}

pub struct ColorScale(pub Vec<Color>);

impl ColorScale {
    pub fn eval(&self, pct: f64) -> Color {
        let (low, pct) = self.inner_eval(pct);
        self.0[low].lerp(self.0[low + 1], pct)
    }

    #[allow(unused)]
    pub fn from_colorous(gradient: colorous::Gradient) -> ColorScale {
        let n = 7;
        ColorScale(
            (0..n)
                .map(|i| {
                    let c = gradient.eval_rational(i, n);
                    Color::rgb(c.r as usize, c.g as usize, c.b as usize)
                })
                .collect(),
        )
    }

    fn inner_eval(&self, pct: f64) -> (usize, f64) {
        assert!((0.0..=1.0).contains(&pct));
        // What's the interval between each pair of colors?
        let width = 1.0 / (self.0.len() - 1) as f64;
        let low = (pct / width).floor() as usize;
        if low == self.0.len() - 1 {
            return (low - 1, 1.0);
        }
        (low, (pct % width) / width)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_scale() {
        use super::{Color, ColorScale};

        let two = ColorScale(vec![Color::BLACK, Color::WHITE]);
        assert_same((0, 0.0), two.inner_eval(0.0));
        assert_same((0, 0.5), two.inner_eval(0.5));
        assert_same((0, 1.0), two.inner_eval(1.0));

        let three = ColorScale(vec![Color::BLACK, Color::RED, Color::WHITE]);
        assert_same((0, 0.0), three.inner_eval(0.0));
        assert_same((0, 0.4), three.inner_eval(0.2));
        assert_same((1, 0.0), three.inner_eval(0.5));
        assert_same((1, 0.4), three.inner_eval(0.7));
        assert_same((1, 1.0), three.inner_eval(1.0));
    }

    fn assert_same(expected: (usize, f64), actual: (usize, f64)) {
        assert_eq!(expected.0, actual.0);
        if (expected.1 - actual.1).abs() > 0.0001 {
            panic!("{:?} != {:?}", expected, actual);
        }
    }
}
