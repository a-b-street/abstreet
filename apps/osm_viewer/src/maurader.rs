use widgetry::{EventCtx, GeomBatch, Color, Text, Drawable};
use geom::{Line, Distance};
use map_model::Map;

pub fn draw(ctx: &EventCtx, map: &Map) -> Drawable {
    let mut batch = GeomBatch::new();

    let step_size = Distance::meters(5.0);
    let buffer_ends = Distance::ZERO;
    let project_away = Distance::meters(2.0);
    let thickness = Distance::meters(0.5);
    let color = Color::BLACK;
    let text_size = 0.2;

    for b in map.all_buildings() {
        batch.push(Color::grey(0.9), b.polygon.clone());

        /*for line in b.polygon.get_outer_ring().as_polyline().lines() {
            for (pt, angle) in line.to_polyline().step_along(step_size, buffer_ends) {
                let polygon = Line::must_new(
                    pt.project_away(project_away, angle.rotate_degs(90.0)),
                    pt.project_away(project_away, angle.rotate_degs(-90.0)),
                ).make_polygons(thickness);
                batch.push(color, polygon);
            }
        }*/

        batch.append(widgetry::Line(map.get_parent(b.sidewalk()).get_name(None)).fg(color).render_curvey(ctx, &b.polygon.get_outer_ring().as_polyline(), text_size));
    }

    ctx.upload(batch)
}
