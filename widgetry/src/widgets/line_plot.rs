use geom::{Angle, Bounds, Circle, Distance, FindClosest, PolyLine, Pt2D};

use crate::widgets::plots::{make_legend, thick_lineseries, Axis, PlotOptions, Series};
use crate::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, ScreenDims, ScreenPt, ScreenRectangle, Text,
    TextExt, Widget, WidgetImpl, WidgetOutput,
};

pub struct LinePlot<X: Axis<X>, Y: Axis<Y>> {
    draw: Drawable,

    // The geometry here is in screen-space.
    max_x: X,
    max_y: Y,
    closest: FindClosest<String>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl<X: Axis<X>, Y: Axis<Y>> LinePlot<X, Y> {
    pub fn new(ctx: &EventCtx, mut series: Vec<Series<X, Y>>, opts: PlotOptions<X, Y>) -> Widget {
        let legend = make_legend(ctx, &series, &opts);
        series.retain(|s| !opts.disabled.contains(&s.label));

        // Assume min_x is X::zero() and min_y is Y::zero()
        let max_x = opts.max_x.unwrap_or_else(|| {
            series
                .iter()
                .map(|s| s.pts.iter().map(|(x, _)| *x).max().unwrap_or(X::zero()))
                .max()
                .unwrap_or(X::zero())
        });
        let max_y = opts.max_y.unwrap_or_else(|| {
            series
                .iter()
                .map(|s| {
                    s.pts
                        .iter()
                        .map(|(_, value)| *value)
                        .max()
                        .unwrap_or(Y::zero())
                })
                .max()
                .unwrap_or(Y::zero())
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
                    PolyLine::must_new(vec![
                        Pt2D::new(0.0, (1.0 - pct) * height),
                        Pt2D::new(width, (1.0 - pct) * height),
                    ])
                    .make_polygons(Distance::meters(1.0)),
                );
            }
        }
        // X axis grid
        if max_x != X::zero() {
            let order_of_mag = 10.0_f64.powf(max_x.to_f64().log10().ceil());
            for i in 0..10 {
                let x = max_x.from_f64(order_of_mag / 10.0 * (i as f64));
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

        let mut closest = FindClosest::new(&Bounds::from(&vec![
            Pt2D::new(0.0, 0.0),
            Pt2D::new(width, height),
        ]));
        for s in series {
            if max_x == X::zero() {
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
            // Downsample to avoid creating polygons with a huge number of points. 1m is untuned,
            // and here "meters" is really pixels.
            pts = Pt2D::approx_dedupe(pts, Distance::meters(1.0));
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
            let x = max_x.from_percent(percent_x);
            // TODO Need ticks now to actually see where this goes
            let batch = Text::from(x.prettyprint())
                .render(ctx)
                .rotate(Angle::degrees(-15.0))
                .autocrop();
            row.push(batch.into_widget(ctx));
        }
        let x_axis = Widget::custom_row(row).padding(10).evenly_spaced();

        let num_y_labels = 4;
        let mut col = Vec::new();
        for i in 0..num_y_labels {
            let percent_y = (i as f64) / ((num_y_labels - 1) as f64);
            col.push(max_y.from_percent(percent_y).prettyprint().text_widget(ctx));
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

impl<X: Axis<X>, Y: Axis<Y>> WidgetImpl for LinePlot<X, Y> {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, _: &mut EventCtx, _: &mut WidgetOutput) {}

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
                    // TODO If some/all of the matches have the same x, write it once?
                    let x = self.max_x.from_percent(pt.x() / self.dims.width);
                    let y_percent = 1.0 - (pt.y() / self.dims.height);

                    // TODO Draw this info in the ColorLegend
                    txt.add_line(format!(
                        "{}: at {}, {}",
                        label,
                        x.prettyprint(),
                        self.max_y.from_percent(y_percent).prettyprint()
                    ));
                }
                if !txt.is_empty() {
                    g.fork_screenspace();
                    g.draw_polygon(Color::RED, Circle::new(cursor.to_pt(), radius).to_polygon());
                    g.draw_mouse_tooltip(txt);
                    g.unfork();
                }
            }
        }
    }
}
