use crate::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, ScreenDims, ScreenPt, Text, TextExt,
    Widget, WidgetImpl, WidgetOutput,
};
use abstutil::prettyprint_usize;
use geom::{Distance, Duration, Polygon, Pt2D};

// The X axis is Durations, with positive meaning "faster" (considered good) and negative "slower"
pub struct Histogram {
    draw: Drawable,

    // TODO Bit sad to pretty much duplicate the geometry?
    rect_labels: Vec<(Polygon, Text)>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl Histogram {
    pub fn new(ctx: &EventCtx, unsorted_dts: Vec<Duration>) -> Widget {
        let mut batch = GeomBatch::new();
        let mut rect_labels = Vec::new();

        let width = 0.20 * ctx.canvas.window_width;
        let height = 0.15 * ctx.canvas.window_height;

        let num_buckets = 10;
        let (min_x, max_x, bars) = bucketize(unsorted_dts, num_buckets);

        let min_y = 0;
        let max_y = bars.iter().map(|(_, _, cnt)| *cnt).max().unwrap();
        let mut outlines = Vec::new();
        for (idx, (min, max, cnt)) in bars.into_iter().enumerate() {
            let color = if min < Duration::ZERO {
                Color::RED
            } else if min == Duration::ZERO && max == Duration::ZERO {
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
                outlines.push(rect.to_outline(Distance::meters(1.5)));
                rect_labels.push((
                    rect,
                    Text::from(Line(format!(
                        "[{}, {}) has {} trips",
                        min,
                        max,
                        prettyprint_usize(cnt)
                    ))),
                ));
            }
        }
        batch.extend(Color::BLACK, outlines);

        let histogram = Histogram {
            draw: ctx.upload(batch),
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
            row.push(dt.to_string().draw_text(ctx));
        }
        let x_axis = Widget::row(row);

        let num_y_labels = 3;
        let mut col = Vec::new();
        for i in 0..num_y_labels {
            let percent_y = (i as f64) / ((num_y_labels - 1) as f64);
            col.push(prettyprint_usize(((max_y as f64) * percent_y) as usize).draw_text(ctx));
        }
        col.reverse();
        let y_axis = Widget::col(col);

        // Don't let the x-axis fill the parent container
        Widget::row(vec![Widget::col(vec![
            Widget::row(vec![
                y_axis.evenly_spaced(),
                Widget::new(Box::new(histogram)),
            ]),
            x_axis.evenly_spaced(),
        ])])
    }
}

impl WidgetImpl for Histogram {
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
            let pt = Pt2D::new(cursor.x - self.top_left.x, cursor.y - self.top_left.y);
            for (rect, lbl) in &self.rect_labels {
                if rect.contains_pt(pt) {
                    g.draw_mouse_tooltip(lbl.clone());
                    break;
                }
            }
        }
    }
}

// min, max, bars
// TODO This has bugs. Perfect surface area to unit test.
fn bucketize(
    unsorted_dts: Vec<Duration>,
    num_buckets: usize,
) -> (Duration, Duration, Vec<(Duration, Duration, usize)>) {
    assert!(num_buckets >= 3);
    if unsorted_dts.is_empty() {
        return (
            Duration::ZERO,
            Duration::ZERO,
            vec![(Duration::ZERO, Duration::ZERO, 0)],
        );
    }

    let min_x = *unsorted_dts.iter().min().unwrap();
    let max_x = *unsorted_dts.iter().max().unwrap();

    let bucket_size = (max_x - min_x) / ((num_buckets - 3) as f64);
    // lower, upper, count
    let mut bars: Vec<(Duration, Duration, usize)> = Vec::new();
    let mut min = min_x;
    while min < max_x {
        let max = min + bucket_size;
        if min < Duration::ZERO && max > Duration::ZERO {
            bars.push((min, Duration::ZERO, 0));
            bars.push((Duration::ZERO, Duration::ZERO, 0));
            bars.push((Duration::ZERO, max, 0));
        } else {
            bars.push((min, max, 0));
        }

        min = max;
    }
    if bars.is_empty() {
        assert_eq!(min, max_x);
        bars.push((Duration::ZERO, Duration::ZERO, 0));
    } else {
        //assert_eq!(bars.len(), num_buckets);
    }

    for dt in unsorted_dts {
        // TODO Could sort them and do this more efficiently.
        let mut ok = false;
        for (min, max, count) in bars.iter_mut() {
            if (dt >= *min && dt < *max) || (*min == *max && dt == *min) {
                *count += 1;
                ok = true;
                break;
            }
        }
        // Most bars represent [low, high) except the last and the [0, 0] one
        if !ok {
            bars.last_mut().unwrap().2 += 1;
        }
    }
    (min_x, max_x, bars)
}
