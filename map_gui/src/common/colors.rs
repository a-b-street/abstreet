use widgetry::Color;

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
        assert!(pct >= 0.0 && pct <= 1.0);
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
        use super::ColorScale;
        use widgetry::Color;

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
