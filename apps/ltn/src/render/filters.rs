use std::collections::BTreeMap;

use geom::Angle;
use map_model::{FilterType, Map};
use widgetry::mapspace::{DrawCustomUnzoomedShapes, PerZoom};
use widgetry::{EventCtx, GeomBatch, RewriteColor};

use crate::render;

pub fn render_modal_filters(ctx: &EventCtx, map: &Map) -> render::Toggle3Zoomed {
    let mut batch = GeomBatch::new();
    let mut low_zoom = DrawCustomUnzoomedShapes::builder();

    let mut icons = BTreeMap::new();
    for ft in [
        FilterType::NoEntry,
        FilterType::WalkCycleOnly,
        FilterType::BusGate,
        FilterType::SchoolStreet,
    ] {
        icons.insert(ft, GeomBatch::load_svg(ctx, render::filter_svg_path(ft)));
    }

    for (road, filter) in map.all_roads_with_modal_filter() {
        let icon = &icons[&filter.filter_type];
        let rewrite_color = if filter.user_modified {
            RewriteColor::NoOp
        } else {
            RewriteColor::ChangeAlpha(0.7)
        };

        if let Ok((pt, road_angle)) = road.center_pts.dist_along(filter.dist) {
            let angle = if filter.filter_type == FilterType::NoEntry {
                road_angle.rotate_degs(90.0)
            } else {
                Angle::ZERO
            };

            batch.append(
                icon.clone()
                    .scale_to_fit_width(road.get_width().inner_meters())
                    .centered_on(pt)
                    .rotate(angle)
                    .color(rewrite_color),
            );

            // TODO Memory intensive
            let icon = icon.clone();
            // TODO They can shrink a bit past their map size
            low_zoom.add_custom(Box::new(move |batch, thickness| {
                batch.append(
                    icon.clone()
                        .scale_to_fit_width(30.0 * thickness)
                        .centered_on(pt)
                        .rotate(angle)
                        .color(rewrite_color),
                );
            }));
        }
    }

    for i in map.all_intersections() {
        if let Some(ref filter) = i.modal_filter {
            let icon = &icons[&filter.filter_type];
            let rewrite_color = if filter.user_modified {
                RewriteColor::NoOp
            } else {
                RewriteColor::ChangeAlpha(0.7)
            };

            let line = filter.geometry(map);
            let angle = if filter.filter_type == FilterType::NoEntry {
                line.angle()
            } else {
                Angle::ZERO
            };
            let pt = line.middle().unwrap();

            batch.append(
                icon.clone()
                    .scale_to_fit_width(line.length().inner_meters())
                    .centered_on(pt)
                    .rotate(angle)
                    .color(rewrite_color),
            );

            let icon = icon.clone();
            low_zoom.add_custom(Box::new(move |batch, thickness| {
                // TODO Why is this magic value different than the one above?
                batch.append(
                    icon.clone()
                        .scale(0.4 * thickness)
                        .centered_on(pt)
                        .rotate(angle)
                        .color(rewrite_color),
                );
            }));
        }
    }

    let min_zoom_for_detail = 5.0;
    let step_size = 0.1;
    // TODO Ideally we get rid of Toggle3Zoomed and make DrawCustomUnzoomedShapes handle this
    // medium-zoom case.
    render::Toggle3Zoomed::new(
        batch.build(ctx),
        low_zoom.build(PerZoom::new(min_zoom_for_detail, step_size)),
    )
}
