use crate::render::OUTLINE_THICKNESS;
use abstutil::prettyprint_usize;
use ezgui::{Color, DrawBoth, EventCtx, GeomBatch, GfxCtx, Line, ScreenPt, ScreenRectangle, Text};
use geom::{Duration, Polygon, Pt2D};

// The X axis is Durations, with positive meaning "faster" (considered good) and negative "slower"
pub struct Histogram {
    draw: DrawBoth,
    rect: ScreenRectangle,

    // TODO Bit sad to pretty much duplicate the geometry from DrawBoth...
    rect_labels: Vec<(Polygon, Text)>,
}

impl Histogram {
    pub fn new(unsorted_dts: Vec<Duration>, ctx: &EventCtx) -> Histogram {
        let mut batch = GeomBatch::new();
        let mut labels: Vec<(Text, ScreenPt)> = Vec::new();
        let mut rect_labels = Vec::new();

        let x1 = 0.7 * ctx.canvas.window_width;
        let x2 = 0.95 * ctx.canvas.window_width;
        let y1 = 0.6 * ctx.canvas.window_height;
        let y2 = 0.9 * ctx.canvas.window_height;
        batch.push(
            Color::grey(0.8),
            Polygon::rectangle(x2 - x1, y2 - y1).translate(x1, y1),
        );

        if unsorted_dts.is_empty() {
            labels.push((
                Text::from(Line("not enough data yet")),
                ScreenPt::new(x1 + (x2 - x1) * 0.1, y1 + (y2 - y1) * 0.1),
            ));
        } else {
            // TODO Generic "bucket into 10 groups, give (min, max, count)"
            let min_x = *unsorted_dts.iter().min().unwrap();
            let max_x = *unsorted_dts.iter().max().unwrap();

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
                if let Some(rect) = Polygon::rectangle_two_corners(
                    // Top-left
                    Pt2D::new(
                        x1 + (x2 - x1) * percent_x_left,
                        y2 - (y2 - y1) * ((cnt as f64) / ((max_y - min_y) as f64)),
                    ),
                    // Bottom-right
                    Pt2D::new(x1 + (x2 - x1) * percent_x_right, y2),
                ) {
                    batch.push(color, rect.clone());
                    batch.push(Color::BLACK.alpha(0.5), rect.to_outline(OUTLINE_THICKNESS));
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

            // TODO These can still get really squished. Draw rotated?
            let num_x_labels = 3;
            for i in 0..num_x_labels {
                let percent_x = (i as f64) / ((num_x_labels - 1) as f64);
                let dt = min_x + (max_x - min_x) * percent_x;
                labels.push((
                    Text::from(Line(dt.to_string())).with_bg(),
                    ScreenPt::new(x1 + percent_x * (x2 - x1), y2),
                ));
            }

            let num_y_labels = 5;
            for i in 0..num_y_labels {
                let percent_y = (i as f64) / ((num_y_labels - 1) as f64);
                // TODO Better alignment...
                let left_px = 30.0;
                labels.push((
                    Text::from(Line(prettyprint_usize(
                        ((max_y as f64) * percent_y) as usize,
                    )))
                    .with_bg(),
                    ScreenPt::new(x1 - left_px, y2 - percent_y * (y2 - y1)),
                ));
            }
        }

        Histogram {
            draw: DrawBoth::new(ctx, batch, labels),
            rect: ScreenRectangle { x1, y1, x2, y2 },
            rect_labels,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.canvas.mark_covered_area(self.rect.clone());

        self.draw.redraw(ScreenPt::new(0.0, 0.0), g);

        let cursor = g.canvas.get_cursor_in_screen_space();
        if self.rect.contains(cursor) {
            for (rect, lbl) in &self.rect_labels {
                if rect.contains_pt(cursor.to_pt()) {
                    g.draw_mouse_tooltip(lbl);
                }
            }
        }
    }
}
