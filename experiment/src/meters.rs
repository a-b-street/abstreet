use abstutil::prettyprint_usize;
use geom::{Line, Polygon, Pt2D};
use map_gui::tools::ColorScale;
use widgetry::{Color, EventCtx, Fill, GeomBatch, LinearGradient, TextExt, Widget};

pub fn make_bar(ctx: &mut EventCtx, scale: ColorScale, value: usize, max: usize) -> Widget {
    let pct_full = (value as f64) / (max as f64);

    let total_width = 300.0;
    let height = 32.0;
    let n = scale.0.len();
    let width_each = total_width / ((n - 1) as f64);

    let mut pieces = Vec::new();
    let mut width_remaining = pct_full * total_width;
    for i in 0..n - 1 {
        let width = width_each.min(width_remaining);
        pieces.push(Polygon::rectangle(width, height).translate((i as f64) * width_each, 0.0));
        if width < width_each {
            break;
        }
        width_remaining -= width;
    }

    let mut batch = GeomBatch::new();
    batch.push(
        Fill::LinearGradient(LinearGradient {
            line: Line::must_new(Pt2D::new(0.0, 0.0), Pt2D::new(total_width, 0.0)),
            stops: scale
                .0
                .iter()
                .enumerate()
                .map(|(idx, color)| ((idx as f64) / ((n - 1) as f64), *color))
                .collect(),
        }),
        Polygon::union_all(pieces),
    );
    batch.push(
        Color::BLACK,
        Polygon::rectangle((1.0 - pct_full) * total_width, height)
            .translate(pct_full * total_width, 0.0),
    );
    Widget::row(vec![
        format!("{} / {}", prettyprint_usize(value), prettyprint_usize(max)).draw_text(ctx),
        Widget::draw_batch(ctx, batch)
            .padding(2)
            .outline(2.0, Color::WHITE),
    ])
}
