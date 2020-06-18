use crate::widgets::line_plot::Yvalue;
use crate::{
    Checkbox, Color, Drawable, EventCtx, GeomBatch, GfxCtx, JustDraw, Line, PlotOptions,
    ScreenDims, ScreenPt, Series, Text, TextExt, Widget, WidgetImpl, WidgetOutput,
};
use geom::{Angle, Circle, Distance, Duration, PolyLine, Pt2D, Time};

// TODO Should rescale grid when a series is enabled. aka lift the enabling out of here and into
// DataOptions.

// The X is always time
pub struct ScatterPlot<T: Yvalue<T> + std::ops::AddAssign + std::ops::Div<f64, Output = T>> {
    series: Vec<SeriesState<T>>,
    draw_grid: Drawable,
    draw_avg: Drawable,
    max_y: T,

    top_left: ScreenPt,
    dims: ScreenDims,
}

struct SeriesState<T: Yvalue<T> + std::ops::AddAssign + std::ops::Div<f64, Output = T>> {
    label: String,
    enabled: bool,
    draw: Drawable,

    sum: T,
    cnt: usize,
}

impl<T: Yvalue<T> + std::ops::AddAssign + std::ops::Div<f64, Output = T>> ScatterPlot<T> {
    // id must be unique in a Composite
    pub fn new(ctx: &EventCtx, id: &str, series: Vec<Series<T>>, opts: PlotOptions<T>) -> Widget {
        let legend = if series.len() == 1 {
            let radius = 15.0;
            // Can't hide if there's just one series
            Widget::row(vec![
                Widget::draw_batch(
                    ctx,
                    GeomBatch::from(vec![(
                        series[0].color,
                        Circle::new(Pt2D::new(radius, radius), Distance::meters(radius))
                            .to_polygon(),
                    )]),
                )
                .margin(5),
                series[0].label.clone().draw_text(ctx),
            ])
        } else {
            let mut row = Vec::new();
            for s in &series {
                row.push(Widget::row(vec![
                    Widget::new(Box::new(
                        Checkbox::colored(ctx, &s.label, s.color, true)
                            .take_checkbox()
                            .callback_to_plot(id, &s.label),
                    ))
                    // TODO Messy! We have to remember to repeat what Checkbox::text does,
                    // because we used take_checkbox
                    .named(&s.label)
                    .margin_right(8),
                    Line(&s.label).draw(ctx),
                ]));
            }
            Widget::row(row).flex_wrap(ctx, 24)
        };

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

        let mut grid_batch = GeomBatch::new();
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
                grid_batch.push(
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
                grid_batch.push(
                    Color::hex("#7C7C7C"),
                    PolyLine::new(vec![
                        Pt2D::new(pct * width, 0.0),
                        Pt2D::new(pct * width, height),
                    ])
                    .make_polygons(Distance::meters(1.0)),
                );
            }
        }

        let circle = Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(4.0)).to_polygon();
        let mut series_state = Vec::new();
        for s in series {
            let mut sum = T::zero();
            let mut cnt = 0;
            let mut batch = GeomBatch::new();
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
            series_state.push(SeriesState {
                label: s.label,
                enabled: true,
                draw: batch.upload(ctx),
                sum,
                cnt,
            });
        }

        let dims = ScreenDims::new(width, height);
        let draw_avg = find_avg(ctx, &series_state, max_y, dims);

        let plot = ScatterPlot {
            series: series_state,
            draw_grid: ctx.upload(grid_batch),
            draw_avg,
            max_y,

            top_left: ScreenPt::new(0.0, 0.0),
            dims,
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
        let x_axis = Widget::row(row).padding(10);

        let num_y_labels = 4;
        let mut col = Vec::new();
        for i in 0..num_y_labels {
            let percent_y = (i as f64) / ((num_y_labels - 1) as f64);
            col.push(max_y.from_percent(percent_y).prettyprint().draw_text(ctx));
        }
        col.reverse();
        let y_axis = Widget::col(col).padding(10);

        // Don't let the x-axis fill the parent container
        Widget::row(vec![Widget::col(vec![
            legend.margin_below(10),
            Widget::row(vec![
                y_axis.evenly_spaced(),
                Widget::new(Box::new(plot)).named(id),
            ]),
            x_axis.evenly_spaced(),
        ])])
    }
}

impl<T: Yvalue<T> + std::ops::AddAssign + std::ops::Div<f64, Output = T>> WidgetImpl
    for ScatterPlot<T>
{
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, _ctx: &mut EventCtx, _output: &mut WidgetOutput) {}

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw_grid);
        for series in &self.series {
            if series.enabled {
                g.redraw_at(self.top_left, &series.draw);
            }
        }
        g.redraw_at(self.top_left, &self.draw_avg);
    }

    fn update_series(&mut self, ctx: &mut EventCtx, label: String, enabled: bool) {
        for series in &mut self.series {
            if series.label == label {
                series.enabled = enabled;
                self.draw_avg = find_avg(ctx, &self.series, self.max_y, self.dims);
                return;
            }
        }
        panic!("ScatterPlot doesn't have a series {}", label);
    }

    fn can_restore(&self) -> bool {
        true
    }
    fn restore(&mut self, ctx: &mut EventCtx, prev: &Box<dyn WidgetImpl>) {
        let prev = prev.downcast_ref::<ScatterPlot<T>>().unwrap();
        for (s1, s2) in self.series.iter_mut().zip(prev.series.iter()) {
            s1.enabled = s2.enabled;
        }
        self.draw_avg = find_avg(ctx, &self.series, self.max_y, self.dims);
    }
}

fn find_avg<T: Yvalue<T> + std::ops::AddAssign + std::ops::Div<f64, Output = T>>(
    ctx: &EventCtx,
    series: &Vec<SeriesState<T>>,
    max_y: T,
    dims: ScreenDims,
) -> Drawable {
    let mut sum = T::zero();
    let mut cnt = 0;
    for s in series {
        if s.enabled {
            sum += s.sum;
            cnt += s.cnt;
        }
    }

    let mut avg_batch = GeomBatch::new();
    if sum != T::zero() {
        let avg = (sum / (cnt as f64)).to_percent(max_y);
        avg_batch.extend(
            Color::hex("#F2F2F2"),
            PolyLine::new(vec![
                Pt2D::new(0.0, (1.0 - avg) * dims.height),
                Pt2D::new(dims.width, (1.0 - avg) * dims.height),
            ])
            .exact_dashed_polygons(
                Distance::meters(1.0),
                Distance::meters(10.0),
                Distance::meters(4.0),
            ),
        );

        let txt = Text::from(Line("avg")).render_ctx(ctx).autocrop();
        let width = txt.get_dims().width;
        avg_batch.append(txt.centered_on(Pt2D::new(-width / 2.0, (1.0 - avg) * dims.height)));
    }
    ctx.upload(avg_batch)
}
