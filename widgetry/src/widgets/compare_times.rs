use geom::{Angle, Circle, Distance, Duration, Pt2D};

use crate::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, ScreenDims, ScreenPt, ScreenRectangle,
    Text, TextExt, Widget, WidgetImpl, WidgetOutput,
};

// TODO This is tuned for the trip time comparison right now.
// - Generic types for x and y axis
// - number of labels
// - rounding behavior
// - forcing the x and y axis to be on the same scale, be drawn as a square
// - coloring the better/worse

pub struct CompareTimes {
    draw: Drawable,

    max: Duration,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl CompareTimes {
    pub fn new_widget<I: AsRef<str>>(
        ctx: &mut EventCtx,
        x_name: I,
        y_name: I,
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
                geom::Line::new(Pt2D::new(0.0, y), Pt2D::new(width, y))
                    .unwrap()
                    .make_polygons(thickness),
            );
            // Vertical
            batch.push(
                Color::grey(0.5),
                geom::Line::new(Pt2D::new(x, 0.0), Pt2D::new(x, height))
                    .unwrap()
                    .make_polygons(thickness),
            );
        }
        // Draw the diagonal, since we're comparing things on the same scale
        batch.push(
            Color::grey(0.5),
            geom::Line::new(Pt2D::new(0.0, height), Pt2D::new(width, 0.0))
                .unwrap()
                .make_polygons(thickness),
        );

        let circle = Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(4.0)).to_polygon();
        for (b, a) in points {
            let pt = Pt2D::new((b / max) * width, (1.0 - (a / max)) * height);
            // TODO Could color circles by mode
            let color = match a.cmp(&b) {
                std::cmp::Ordering::Equal => Color::YELLOW.alpha(0.5),
                std::cmp::Ordering::Less => Color::GREEN.alpha(0.9),
                std::cmp::Ordering::Greater => Color::RED.alpha(0.9),
            };
            batch.push(color, circle.translate(pt.x(), pt.y()));
        }
        let plot = Widget::new(Box::new(CompareTimes {
            dims: batch.get_dims(),
            draw: ctx.upload(batch),
            max,
            top_left: ScreenPt::new(0.0, 0.0),
        }));

        let y_axis = Widget::custom_col(
            labels
                .iter()
                .rev()
                .map(|x| {
                    Line(x.num_minutes_rounded_up().to_string())
                        .small()
                        .into_widget(ctx)
                })
                .collect(),
        )
        .evenly_spaced();
        let mut y_label = Text::from(format!("{} (minutes)", y_name.as_ref()))
            .render(ctx)
            .rotate(Angle::degrees(90.0));
        y_label.autocrop_dims = true;
        let y_label = y_label
            .autocrop()
            .into_widget(ctx)
            .centered_vert()
            .margin_right(5);

        let x_axis = Widget::custom_row(
            labels
                .iter()
                .map(|x| {
                    Line(x.num_minutes_rounded_up().to_string())
                        .small()
                        .into_widget(ctx)
                })
                .collect(),
        )
        .evenly_spaced();
        let x_label = format!("{} (minutes)", x_name.as_ref())
            .text_widget(ctx)
            .centered_horiz();

        // It's a bit of work to make both the x and y axis line up with the plot. :)
        let plot_width = plot.get_width_for_forcing();
        Widget::custom_col(vec![
            Widget::custom_row(vec![y_label, y_axis, plot]),
            Widget::custom_col(vec![x_axis, x_label])
                .force_width(plot_width)
                .align_right(),
        ])
        .container()
    }
}

impl WidgetImpl for CompareTimes {
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
            let rect = ScreenRectangle::top_left(self.top_left, self.dims);
            if let Some((pct_x, pct_y)) = rect.pt_to_percent(cursor) {
                let thickness = Distance::meters(2.0);
                let mut batch = GeomBatch::new();
                // Horizontal
                if let Some(l) = geom::Line::new(Pt2D::new(rect.x1, cursor.y), cursor.to_pt()) {
                    batch.push(Color::WHITE, l.make_polygons(thickness));
                }
                // Vertical
                if let Some(l) = geom::Line::new(Pt2D::new(cursor.x, rect.y2), cursor.to_pt()) {
                    batch.push(Color::WHITE, l.make_polygons(thickness));
                }

                g.fork_screenspace();
                let draw = g.upload(batch);
                g.redraw(&draw);
                // TODO Quite specialized to the one use right now
                let before = pct_x * self.max;
                let after = (1.0 - pct_y) * self.max;
                if after <= before {
                    g.draw_mouse_tooltip(Text::from_multiline(vec![
                        Line(format!("Before: {}", before)),
                        Line(format!("After: {}", after)),
                        Line(format!(
                            "{} faster (-{:.1}%)",
                            before - after,
                            100.0 * (1.0 - after / before)
                        ))
                        .fg(Color::hex("#72CE36")),
                    ]));
                } else {
                    g.draw_mouse_tooltip(Text::from_multiline(vec![
                        Line(format!("Before: {}", before)),
                        Line(format!("After: {}", after)),
                        Line(format!(
                            "{} slower (+{:.1}%)",
                            after - before,
                            100.0 * (after / before - 1.0)
                        ))
                        .fg(Color::hex("#EB3223")),
                    ]));
                }
                g.unfork();
            }
        }
    }
}
