use crate::widgets::line_plot::{make_legend, Yvalue};
use crate::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, JustDraw, Line, PlotOptions, ScreenDims,
    ScreenPt, Series, Text, TextExt, Widget, WidgetImpl, WidgetOutput,
};
use geom::{Angle, Circle, Distance, Duration, PolyLine, Pt2D, Time};

// The X is always time
pub struct ScatterPlot {
    draw: Drawable,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl ScatterPlot {
    pub fn new<T: Yvalue<T> + std::ops::AddAssign + std::ops::Div<f64, Output = T>>(
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

        let circle = Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(4.0)).to_polygon();
        let mut sum = T::zero();
        let mut cnt = 0;
        for s in series {
            for (t, y) in s.pts {
                cnt += 1;
                sum += y;
                let percent_x = t.to_percent(max_x);
                let percent_y = y.to_percent(max_y);
                // Y inversion
                batch.push(
                    s.color,
                    circle.translate(percent_x * width, (1.0 - percent_y) * height),
                );
            }
        }

        if sum != T::zero() {
            let avg = (sum / (cnt as f64)).to_percent(max_y);
            batch.extend(
                Color::hex("#F2F2F2"),
                PolyLine::must_new(vec![
                    Pt2D::new(0.0, (1.0 - avg) * height),
                    Pt2D::new(width, (1.0 - avg) * height),
                ])
                .exact_dashed_polygons(
                    Distance::meters(1.0),
                    Distance::meters(10.0),
                    Distance::meters(4.0),
                ),
            );

            let txt = Text::from(Line("avg")).render_ctx(ctx).autocrop();
            let width = txt.get_dims().width;
            batch.append(txt.centered_on(Pt2D::new(-width / 2.0, (1.0 - avg) * height)));
        }

        let plot = ScatterPlot {
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

impl WidgetImpl for ScatterPlot {
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
