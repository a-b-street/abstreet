use abstutil::prettyprint_usize;
use geom::Polygon;
use widgetry::{Color, EventCtx, GeomBatch, Line, Text, Widget};

pub fn make_bar(ctx: &mut EventCtx, filled_color: Color, value: usize, max: usize) -> Widget {
    let total_width = 300.0;
    let height = 32.0;
    let radius = Some(4.0);
    let pct_full = (value as f64) / (max as f64);

    let mut batch = GeomBatch::new();
    // Background
    batch.push(
        Color::hex("#666666"),
        Polygon::rounded_rectangle(total_width, height, radius),
    );
    // Foreground
    if let Some(poly) = Polygon::maybe_rounded_rectangle(pct_full * total_width, height, radius) {
        batch.push(filled_color, poly);
    }
    // Text
    let label = Text::from(Line(format!(
        "{} / {}",
        prettyprint_usize(value),
        prettyprint_usize(max)
    )))
    .render_to_batch(ctx.prerender);
    let dims = label.get_dims();
    batch.append(label.translate(10.0, height / 2.0 - dims.height / 2.0));
    Widget::draw_batch(ctx, batch)
}
