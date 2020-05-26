use crate::widgets::line_plot::Yvalue;
use crate::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, JustDraw, Line, PlotOptions, ScreenDims,
    ScreenPt, ScreenRectangle, Series, Text, TextExt, Widget, WidgetImpl, WidgetOutput,
};
use geom::{Angle, Circle, Distance, Duration, PolyLine, Pt2D, Time};

// TODO This is tuned for the trip time comparison right now.
// - Generic types for x and y axis
// - number of labels
// - rounding behavior
// - forcing the x and y axis to be on the same scale, be drawn as a square
// - coloring the better/worse

pub struct ScatterPlot {
    draw: Drawable,

    max: Duration,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl ScatterPlot {
    pub fn new(
        ctx: &mut EventCtx,
        x_name: &str,
        y_name: &str,
        points: Vec<(Duration, Duration)>,
    ) -> Widget {
        if points.is_empty() {
            return Widget::nothing();
        }

        let actual_max = *points.iter().map(|(b, a)| a.max(b)).max().unwrap();
        // Excluding 0
        let num_labels = 5;
        let (max, labels) = actual_max.make_intervals_for_max(num_labels);

        // We want a nice square so the scales match up.
        let width = 500.0;
        let height = width;

        let mut batch = GeomBatch::new();
        batch.autocrop_dims = false;

        // Grid lines
        let thickness = Distance::meters(2.0);
        for i in 1..num_labels {
            let x = (i as f64) / (num_labels as f64) * width;
            let y = (i as f64) / (num_labels as f64) * height;
            // Horizontal
            batch.push(
                Color::grey(0.5),
                geom::Line::new(Pt2D::new(0.0, y), Pt2D::new(width, y)).make_polygons(thickness),
            );
            // Vertical
            batch.push(
                Color::grey(0.5),
                geom::Line::new(Pt2D::new(x, 0.0), Pt2D::new(x, height)).make_polygons(thickness),
            );
        }
        // Draw the diagonal, since we're comparing things on the same scale
        batch.push(
            Color::grey(0.5),
            geom::Line::new(Pt2D::new(0.0, height), Pt2D::new(width, 0.0)).make_polygons(thickness),
        );

        let circle = Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(4.0)).to_polygon();
        for (b, a) in points {
            let pt = Pt2D::new((b / max) * width, (1.0 - (a / max)) * height);
            // TODO Could color circles by mode
            let color = if a == b {
                Color::YELLOW.alpha(0.5)
            } else if a < b {
                Color::GREEN.alpha(0.9)
            } else {
                Color::RED.alpha(0.9)
            };
            batch.push(color, circle.translate(pt.x(), pt.y()));
        }
        let plot = Widget::new(Box::new(ScatterPlot {
            dims: batch.get_dims(),
            draw: ctx.upload(batch),
            max,
            top_left: ScreenPt::new(0.0, 0.0),
        }));

        let y_axis = Widget::col(
            labels
                .iter()
                .rev()
                .map(|x| Line(x.to_string()).small().draw(ctx))
                .collect(),
        )
        .evenly_spaced();
        let y_label = {
            let mut label = GeomBatch::new();
            for (color, poly) in Text::from(Line(format!("{} (minutes)", y_name)))
                .render_ctx(ctx)
                .consume()
            {
                label.fancy_push(color, poly.rotate(Angle::new_degs(90.0)));
            }
            // The text is already scaled; don't use Widget::draw_batch and scale it again.
            JustDraw::wrap(ctx, label.autocrop()).centered_vert()
        };

        let x_axis = Widget::row(
            labels
                .iter()
                .map(|x| Line(x.to_string()).small().draw(ctx))
                .collect(),
        )
        .evenly_spaced();
        let x_label = format!("{} (minutes)", x_name)
            .draw_text(ctx)
            .centered_horiz();

        // It's a bit of work to make both the x and y axis line up with the plot. :)
        let plot_width = plot.get_width_for_forcing();
        Widget::row(vec![Widget::col(vec![
            Widget::row(vec![y_label, y_axis, plot]),
            Widget::col(vec![x_axis, x_label])
                .force_width(plot_width)
                .align_right(),
        ])])
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

        if let Some(cursor) = g.canvas.get_cursor_in_screen_space() {
            let rect = ScreenRectangle::top_left(self.top_left, self.dims);
            if let Some((pct_x, pct_y)) = rect.pt_to_percent(cursor) {
                let thickness = Distance::meters(2.0);
                let mut batch = GeomBatch::new();
                // Horizontal
                if let Some(l) = geom::Line::maybe_new(Pt2D::new(rect.x1, cursor.y), cursor.to_pt())
                {
                    batch.push(Color::WHITE, l.make_polygons(thickness));
                }
                // Vertical
                if let Some(l) = geom::Line::maybe_new(Pt2D::new(cursor.x, rect.y2), cursor.to_pt())
                {
                    batch.push(Color::WHITE, l.make_polygons(thickness));
                }

                g.fork_screenspace();
                let draw = g.upload(batch);
                g.redraw(&draw);
                // TODO Quite specialized to the one use right now
                let before = pct_x * self.max;
                let after = (1.0 - pct_y) * self.max;
                if after <= before {
                    g.draw_mouse_tooltip(Text::from_all(vec![
                        Line(format!("{} faster", before - after)).fg(Color::GREEN),
                        Line(format!(" than {}", before)),
                    ]));
                } else {
                    g.draw_mouse_tooltip(Text::from_all(vec![
                        Line(format!("{} slower", after - before)).fg(Color::RED),
                        Line(format!(" than {}", before)),
                    ]));
                }
                g.unfork();
            }
        }
    }
}

// TODO Dedupe

// The X is always time
pub struct ScatterPlotV2 {
    draw_data: Drawable,
    draw_grid: Drawable,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl ScatterPlotV2 {
    pub fn new<T: Yvalue<T>>(ctx: &EventCtx, data: Series<T>, opts: PlotOptions<T>) -> Widget {
        // Assume min_x is Time::START_OF_DAY and min_y is T::zero()
        let max_x = opts.max_x.unwrap_or_else(|| {
            data.pts
                .iter()
                .map(|(t, _)| *t)
                .max()
                .unwrap_or(Time::START_OF_DAY)
        });
        let max_y = opts.max_y.unwrap_or_else(|| {
            data.pts
                .iter()
                .map(|(_, value)| *value)
                .max()
                .unwrap_or(T::zero())
        });

        // TODO Tuned to fit the info panel. Instead these should somehow stretch to fill their
        // container.
        let width = 0.23 * ctx.canvas.window_width;
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
                grid_batch.push(
                    Color::BLACK,
                    PolyLine::new(vec![
                        Pt2D::new(pct * width, 0.0),
                        Pt2D::new(pct * width, height),
                    ])
                    .make_polygons(Distance::meters(5.0)),
                );
            }
        }

        let mut batch = GeomBatch::new();
        let circle = Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(4.0)).to_polygon();
        for (t, y) in data.pts {
            let percent_x = t.to_percent(max_x);
            let percent_y = y.to_percent(max_y);
            // Y inversion
            batch.push(
                data.color,
                circle.translate(percent_x * width, (1.0 - percent_y) * height),
            );
        }

        let plot = ScatterPlotV2 {
            draw_data: ctx.upload(batch),
            draw_grid: ctx.upload(grid_batch),

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(width, height),
        };

        let num_x_labels = 3;
        let mut row = Vec::new();
        for i in 0..num_x_labels {
            let percent_x = (i as f64) / ((num_x_labels - 1) as f64);
            let t = max_x.percent_of(percent_x);
            // TODO Need ticks now to actually see where this goes
            let mut batch = GeomBatch::new();
            for (color, poly) in Text::from(Line(t.to_string())).render_ctx(ctx).consume() {
                batch.fancy_push(color, poly.rotate(Angle::new_degs(-15.0)));
            }
            // The text is already scaled; don't use Widget::draw_batch and scale it again.
            row.push(JustDraw::wrap(ctx, batch.autocrop()));
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
            Line(data.label).draw(ctx),
            Widget::row(vec![y_axis.evenly_spaced(), Widget::new(Box::new(plot))]),
            x_axis.evenly_spaced(),
        ])])
    }
}

impl WidgetImpl for ScatterPlotV2 {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, _ctx: &mut EventCtx, _output: &mut WidgetOutput) {}

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw_grid);
        g.redraw_at(self.top_left, &self.draw_data);
    }
}
