use crate::layout::Widget;
use crate::{
    Color, DrawBoth, EventCtx, GeomBatch, GfxCtx, Line, ManagedWidget, ScreenDims, ScreenPt, Text,
};
use abstutil::prettyprint_usize;
use geom::{Distance, Duration, Polygon, Pt2D};

// The X axis is Durations, with positive meaning "faster" (considered good) and negative "slower"
pub struct Histogram {
    draw: DrawBoth,

    // TODO Bit sad to pretty much duplicate the geometry from DrawBoth...
    rect_labels: Vec<(Polygon, Text)>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl Histogram {
    pub fn new(unsorted_dts: Vec<Duration>, ctx: &EventCtx) -> ManagedWidget {
        let mut batch = GeomBatch::new();
        let mut rect_labels = Vec::new();

        let width = 0.25 * ctx.canvas.window_width;
        let height = 0.3 * ctx.canvas.window_height;

        // TODO Generic "bucket into 10 groups, give (min, max, count)"
        let min_x = unsorted_dts.iter().min().cloned().unwrap_or(Duration::ZERO);
        let max_x = unsorted_dts.iter().max().cloned().unwrap_or(Duration::ZERO);

        let num_buckets = 10;
        let bucket_size = (max_x - min_x) / (num_buckets as f64);
        // lower, upper, count
        let mut bars: Vec<(Duration, Duration, usize)> = (0..num_buckets)
            .map(|idx| {
                let i = idx as f64;
                (min_x + bucket_size * i, min_x + bucket_size * (i + 1.0), 0)
            })
            .collect();
        for dt in unsorted_dts {
            // TODO Could sort them and do this more efficiently.
            if dt == max_x {
                // Most bars represent [low, high) except the last
                bars[num_buckets - 1].2 += 1;
            } else {
                let bin = ((dt - min_x) / bucket_size).floor() as usize;
                bars[bin].2 += 1;
            }
        }

        let min_y = 0;
        let max_y = bars.iter().map(|(_, _, cnt)| *cnt).max().unwrap();
        for (idx, (min, max, cnt)) in bars.into_iter().enumerate() {
            let color = if max < Duration::ZERO {
                Color::RED
            } else if min < Duration::ZERO {
                Color::YELLOW
            } else {
                Color::GREEN
            };
            let percent_x_left = (idx as f64) / (num_buckets as f64);
            let percent_x_right = ((idx + 1) as f64) / (num_buckets as f64);
            let percent_y_top = if max_y == min_y {
                0.0
            } else {
                (cnt as f64) / ((max_y - min_y) as f64)
            };
            if let Some(rect) = Polygon::rectangle_two_corners(
                // Top-left
                Pt2D::new(width * percent_x_left, height * (1.0 - percent_y_top)),
                // Bottom-right
                Pt2D::new(width * percent_x_right, height),
            ) {
                batch.push(color, rect.clone());
                batch.push(
                    Color::BLACK.alpha(0.5),
                    rect.to_outline(Distance::meters(0.5)),
                );
                rect_labels.push((
                    rect,
                    Text::from(Line(format!(
                        "[{}, {}) has {} trips",
                        min,
                        max,
                        prettyprint_usize(cnt)
                    )))
                    .with_bg(),
                ));
            }
        }

        let histogram = Histogram {
            draw: DrawBoth::new(ctx, batch, Vec::new()),
            rect_labels,

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(width, height),
        };

        // TODO These can still get really squished. Draw rotated?
        let num_x_labels = 3;
        let mut row = Vec::new();
        for i in 0..num_x_labels {
            let percent_x = (i as f64) / ((num_x_labels - 1) as f64);
            let dt = min_x + (max_x - min_x) * percent_x;
            row.push(ManagedWidget::draw_text(
                ctx,
                Text::from(Line(dt.to_string())),
            ));
        }
        let x_axis = ManagedWidget::row(row);

        let num_y_labels = 5;
        let mut col = Vec::new();
        for i in 0..num_y_labels {
            let percent_y = (i as f64) / ((num_y_labels - 1) as f64);
            col.push(ManagedWidget::draw_text(
                ctx,
                Text::from(Line(prettyprint_usize(
                    ((max_y as f64) * percent_y) as usize,
                ))),
            ));
        }
        col.reverse();
        let y_axis = ManagedWidget::col(col);

        ManagedWidget::col(vec![
            ManagedWidget::row(vec![
                y_axis.evenly_spaced(),
                ManagedWidget::histogram(histogram),
            ]),
            x_axis.evenly_spaced(),
        ])
    }

    pub(crate) fn draw(&self, g: &mut GfxCtx) {
        self.draw.redraw(self.top_left, g);

        if let Some(cursor) = g.canvas.get_cursor_in_screen_space() {
            let pt = Pt2D::new(cursor.x - self.top_left.x, cursor.y - self.top_left.y);
            for (rect, lbl) in &self.rect_labels {
                if rect.contains_pt(pt) {
                    g.draw_mouse_tooltip(lbl);
                    break;
                }
            }
        }
    }
}

impl Widget for Histogram {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}
