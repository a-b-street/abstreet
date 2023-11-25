mod cells;
pub mod colors;
mod filters;

use std::collections::HashMap;

use geom::{Angle, Distance, Pt2D};
use map_model::{AmenityType, ExtraPOIType, FilterType, Map, RestrictionType, Road, TurnType};
use widgetry::mapspace::DrawCustomUnzoomedShapes;
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, RewriteColor, Text};

pub use cells::RenderCells;
pub use filters::render_modal_filters;

pub fn render_poi_icons(ctx: &EventCtx, map: &Map) -> Drawable {
    let mut batch = GeomBatch::new();
    let school = GeomBatch::load_svg(ctx, "system/assets/map/school.svg")
        .scale(0.2)
        .color(RewriteColor::ChangeAll(Color::WHITE));

    for b in map.all_buildings() {
        if b.amenities.iter().any(|a| {
            let at = AmenityType::categorize(&a.amenity_type);
            at == Some(AmenityType::School) || at == Some(AmenityType::University)
        }) {
            batch.append(school.clone().centered_on(b.polygon.polylabel()));
        }
    }

    let tfl =
        GeomBatch::load_svg(ctx, "system/assets/map/tfl_underground.svg").scale_to_fit_width(20.0);
    let national_rail =
        GeomBatch::load_svg(ctx, "system/assets/map/national_rail.svg").scale_to_fit_width(20.0);

    // TODO Toggle3Zoomed could be nicer; these're not terribly visible from afar
    for extra in map.all_extra_pois() {
        let (name, icon) = match extra.kind {
            ExtraPOIType::LondonUndergroundStation(ref name) => (name, &tfl),
            ExtraPOIType::NationalRailStation(ref name) => (name, &national_rail),
        };
        batch.append(icon.clone().centered_on(extra.pt));
        batch.append(
            Text::from(Line(name).fg(Color::WHITE))
                .bg(Color::hex("#0019A8"))
                .render_autocropped(ctx)
                .scale_to_fit_height(10.0)
                .centered_on(extra.pt.offset(0.0, icon.get_bounds().height())),
        );
    }

    ctx.upload(batch)
}

pub fn render_bus_routes(ctx: &EventCtx, map: &Map) -> Drawable {
    let mut batch = GeomBatch::new();
    for r in map.all_roads() {
        if map.get_bus_routes_on_road(r.id).is_empty() {
            continue;
        }
        // Draw dashed outlines surrounding the road
        let width = r.get_width();
        for pl in [
            r.center_pts.shift_left(width * 0.7),
            r.center_pts.shift_right(width * 0.7),
        ]
        .into_iter()
        .flatten()
        {
            batch.extend(
                *colors::BUS_ROUTE,
                pl.exact_dashed_polygons(
                    Distance::meters(2.0),
                    Distance::meters(5.0),
                    Distance::meters(2.0),
                ),
            );
        }
    }
    ctx.upload(batch)
}

pub fn render_turn_restrictions(ctx: &EventCtx, map: &Map) -> Drawable {
    let mut batch = GeomBatch::new();
    for r1 in map.all_roads() {
        // TODO Also interpret lane-level? Maybe just check all the generated turns and see what's
        // allowed / banned in practice?

        // Count the number of turn restrictions at each end of the road
        let mut icon_counter = HashMap::from([(r1.dst_i, 1), (r1.src_i, 1)]);

        for (restriction, r2) in &r1.turn_restrictions {
            // TODO "Invert" OnlyAllowTurns so we can just draw banned things
            if *restriction == RestrictionType::BanTurns {
                let (t_type, sign_pt, r1_angle, i) =
                    map.get_ban_turn_info(r1, map.get_r(*r2), &icon_counter);
                // add to the counter
                icon_counter.entry(i).and_modify(|n| *n += 1);
                batch.append(draw_turn_restriction_icon(
                    ctx, t_type, sign_pt, r1, r1_angle,
                ));
            }
        }
        for (_via, r2) in &r1.complicated_turn_restrictions {
            // TODO Show the 'via'? Or just draw the entire shape?
            let (t_type, sign_pt, r1_angle, i) =
                map.get_ban_turn_info(r1, map.get_r(*r2), &icon_counter);
            icon_counter.entry(i).and_modify(|n| *n += 1);
            batch.append(draw_turn_restriction_icon(
                ctx, t_type, sign_pt, r1, r1_angle,
            ));
        }
    }
    ctx.upload(batch)
}

fn draw_turn_restriction_icon(
    ctx: &EventCtx,
    t_type: TurnType,
    sign_pt: Pt2D,
    r1: &Road,
    r1_angle: Angle,
) -> GeomBatch {
    let mut batch = GeomBatch::new();

    // Which icon do we want?
    let no_right_t = "system/assets/map/no_right_turn.svg";
    let no_left_t = "system/assets/map/no_left_turn.svg";
    let no_u_t = "system/assets/map/no_u_turn_left_to_right.svg";
    let no_straight = "system/assets/map/no_straight_ahead.svg";
    // TODO - what should we do with these?
    let other_t = "system/assets/map/thought_bubble.svg";

    let icon_path = match t_type {
        TurnType::Right => no_right_t,
        TurnType::Left => no_left_t,
        TurnType::UTurn => no_u_t,
        TurnType::Crosswalk => other_t,
        TurnType::SharedSidewalkCorner => other_t,
        TurnType::Straight => no_straight,
        TurnType::UnmarkedCrossing => other_t,
    };

    // Draw the svg icon
    let icon = GeomBatch::load_svg(ctx, icon_path)
        .scale_to_fit_width(r1.get_width().inner_meters())
        .centered_on(sign_pt)
        .rotate_around_batch_center(r1_angle.rotate_degs(90.0));

    batch.append(icon);
    batch
}

/// Depending on the canvas zoom level, draws one of 2 things.
// TODO Rethink filter styles and do something better than this.
pub struct Toggle3Zoomed {
    draw_zoomed: Drawable,
    unzoomed: DrawCustomUnzoomedShapes,
}

impl Toggle3Zoomed {
    pub fn new(draw_zoomed: Drawable, unzoomed: DrawCustomUnzoomedShapes) -> Self {
        Self {
            draw_zoomed,
            unzoomed,
        }
    }

    pub fn empty(ctx: &EventCtx) -> Self {
        Self::new(Drawable::empty(ctx), DrawCustomUnzoomedShapes::empty())
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if !self.unzoomed.maybe_draw(g) {
            self.draw_zoomed.draw(g);
        }
    }
}

pub fn filter_svg_path(ft: FilterType) -> &'static str {
    match ft {
        FilterType::NoEntry => "system/assets/tools/no_entry.svg",
        FilterType::WalkCycleOnly => "system/assets/tools/modal_filter.svg",
        FilterType::BusGate => "system/assets/tools/bus_gate.svg",
        FilterType::SchoolStreet => "system/assets/tools/school_street.svg",
    }
}

pub fn filter_hide_color(ft: FilterType) -> Color {
    match ft {
        FilterType::WalkCycleOnly => Color::hex("#0b793a"),
        FilterType::NoEntry => Color::RED,
        FilterType::BusGate => *colors::BUS_ROUTE,
        FilterType::SchoolStreet => Color::hex("#e31017"),
    }
}
