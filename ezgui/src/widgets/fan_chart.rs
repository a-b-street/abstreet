use crate::widgets::line_plot::{make_legend, thick_lineseries, Yvalue};
use crate::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, JustDraw, Line, PlotOptions, ScreenDims,
    ScreenPt, Series, Text, TextExt, Widget, WidgetImpl, WidgetOutput,
};
use geom::{
    Angle, Distance, Duration, HgramValue, Histogram, PolyLine, Polygon, Pt2D, Statistic, Time,
};
use std::collections::VecDeque;

// The X is always time
pub struct FanChart {
    draw: Drawable,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl FanChart {
    pub fn new<T: Yvalue<T> + HgramValue<T>>(
        ctx: &EventCtx,
        mut series: Vec<Series<T>>,
        opts: PlotOptions<T>,
    ) -> Widget {
        let legend = make_legend(ctx, &series, &opts);
        series.retain(|s| !opts.disabled.contains(&s.label));

        // TODO Refactor this part with LinePlot too
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
        let width = 0.22 * ctx.canvas.window_width;
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
                    PolyLine::must_new(vec![
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
                    PolyLine::must_new(vec![
                        Pt2D::new(pct * width, 0.0),
                        Pt2D::new(pct * width, height),
                    ])
                    .make_polygons(Distance::meters(1.0)),
                );
            }
        }

        let transform = |input: Vec<(Time, T)>| {
            // TODO Copied from LinePlot...
            let mut pts = Vec::new();
            for (t, y) in input {
                let percent_x = t.to_percent(max_x);
                let percent_y = y.to_percent(max_y);
                pts.push(Pt2D::new(
                    percent_x * width,
                    // Y inversion! :D
                    (1.0 - percent_y) * height,
                ));
            }
            pts.dedup();
            pts
        };

        for s in series {
            if s.pts.len() < 2 {
                continue;
            }
            let (p50, p90, mut p99) = slidey_window(s.pts, Duration::hours(1));

            // Make a band between p50 and p99
            let mut band = transform(p50);
            p99.reverse();
            band.extend(transform(p99));
            band.push(band[0]);
            batch.push(s.color.alpha(0.5), Polygon::new(&band));

            batch.push(
                s.color,
                thick_lineseries(transform(p90), Distance::meters(5.0)),
            );
        }

        let plot = FanChart {
            draw: ctx.upload(batch),

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

impl WidgetImpl for FanChart {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, _ctx: &mut EventCtx, _output: &mut WidgetOutput) {}

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);
    }
}

// Returns (P50, P90, P99)
fn slidey_window<T: HgramValue<T>>(
    input: Vec<(Time, T)>,
    window_size: Duration,
) -> (Vec<(Time, T)>, Vec<(Time, T)>, Vec<(Time, T)>) {
    let mut p50: Vec<(Time, T)> = Vec::new();
    let mut p90: Vec<(Time, T)> = Vec::new();
    let mut p99: Vec<(Time, T)> = Vec::new();
    let mut window: VecDeque<(Time, T)> = VecDeque::new();
    let mut hgram = Histogram::new();

    let mut last_sample = Time::START_OF_DAY;

    for (t, value) in input {
        window.push_back((t, value));
        hgram.add(value);

        if t - last_sample > Duration::minutes(1) {
            p50.push((t, hgram.select(Statistic::P50).unwrap()));
            p90.push((t, hgram.select(Statistic::P90).unwrap()));
            p99.push((t, hgram.select(Statistic::P99).unwrap()));
            last_sample = t;
        }

        while !window.is_empty() && t - window.front().unwrap().0 > window_size {
            let (_, old_value) = window.pop_front().unwrap();
            hgram.remove(old_value);
        }
    }

    (p50, p90, p99)
}
