use std::collections::HashMap;

use abstutil::Counter;
use geom::{Circle, Distance, Line, Polygon, Pt2D};
use map_gui::common::ColorScale;
use map_model::{BuildingID, BusStopID, IntersectionID, LaneID, Map, ParkingLotID, RoadID};
use widgetry::{Color, Drawable, EventCtx, Fill, GeomBatch, Line, LinearGradient, Text, Widget};

use crate::app::App;

pub struct ColorDiscrete<'a> {
    map: &'a Map,
    unzoomed: GeomBatch,
    zoomed: GeomBatch,
    // Store both, so we can build the legend in the original order later
    categories: Vec<(&'static str, Color)>,
    colors: HashMap<&'static str, Color>,
}

impl<'a> ColorDiscrete<'a> {
    pub fn new(app: &'a App, categories: Vec<(&'static str, Color)>) -> ColorDiscrete<'a> {
        let mut unzoomed = GeomBatch::new();
        unzoomed.push(
            app.cs.fade_map_dark,
            app.primary.map.get_boundary_polygon().clone(),
        );
        ColorDiscrete {
            map: &app.primary.map,
            unzoomed,
            zoomed: GeomBatch::new(),
            colors: categories.iter().cloned().collect(),
            categories,
        }
    }

    pub fn add_l(&mut self, l: LaneID, category: &'static str) {
        let color = self.colors[category];
        self.unzoomed
            .push(color, self.map.get_parent(l).get_thick_polygon(self.map));
        let lane = self.map.get_l(l);
        self.zoomed.push(
            color.alpha(0.4),
            lane.lane_center_pts.make_polygons(lane.width),
        );
    }

    pub fn add_r(&mut self, r: RoadID, category: &'static str) {
        let color = self.colors[category];
        self.unzoomed
            .push(color, self.map.get_r(r).get_thick_polygon(self.map));
        self.zoomed.push(
            color.alpha(0.4),
            self.map.get_r(r).get_thick_polygon(self.map),
        );
    }

    pub fn add_i(&mut self, i: IntersectionID, category: &'static str) {
        let color = self.colors[category];
        self.unzoomed.push(color, self.map.get_i(i).polygon.clone());
        self.zoomed
            .push(color.alpha(0.4), self.map.get_i(i).polygon.clone());
    }

    pub fn add_b(&mut self, b: BuildingID, category: &'static str) {
        let color = self.colors[category];
        self.unzoomed.push(color, self.map.get_b(b).polygon.clone());
        self.zoomed
            .push(color.alpha(0.4), self.map.get_b(b).polygon.clone());
    }

    pub fn add_bs(&mut self, bs: BusStopID, category: &'static str) {
        let color = self.colors[category];
        let pt = self.map.get_bs(bs).sidewalk_pos.pt(self.map);
        self.zoomed.push(
            color.alpha(0.4),
            Circle::new(pt, Distance::meters(5.0)).to_polygon(),
        );
        self.unzoomed
            .push(color, Circle::new(pt, Distance::meters(15.0)).to_polygon());
    }

    pub fn build(self, ctx: &mut EventCtx) -> (Drawable, Drawable, Widget) {
        let legend = self
            .categories
            .into_iter()
            .map(|(name, color)| ColorLegend::row(ctx, color, name))
            .collect();
        (
            ctx.upload(self.unzoomed),
            ctx.upload(self.zoomed),
            Widget::col(legend),
        )
    }
}

pub struct ColorLegend {}

impl ColorLegend {
    pub fn row<S: Into<String>>(ctx: &mut EventCtx, color: Color, label: S) -> Widget {
        let radius = 15.0;
        Widget::row(vec![
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(
                    color,
                    Circle::new(Pt2D::new(radius, radius), Distance::meters(radius)).to_polygon(),
                )]),
            )
            .centered_vert(),
            Text::from(Line(label)).wrap_to_pct(ctx, 35).draw(ctx),
        ])
    }

    pub fn gradient<I: Into<String>>(
        ctx: &mut EventCtx,
        scale: &ColorScale,
        labels: Vec<I>,
    ) -> Widget {
        assert!(scale.0.len() >= 2);
        let width = 300.0;
        let n = scale.0.len();
        let mut batch = GeomBatch::new();
        let width_each = width / ((n - 1) as f64);
        batch.push(
            Fill::LinearGradient(LinearGradient {
                line: Line::must_new(Pt2D::new(0.0, 0.0), Pt2D::new(width, 0.0)),
                stops: scale
                    .0
                    .iter()
                    .enumerate()
                    .map(|(idx, color)| ((idx as f64) / ((n - 1) as f64), *color))
                    .collect(),
            }),
            Polygon::union_all(
                (0..n - 1)
                    .map(|i| {
                        Polygon::rectangle(width_each, 32.0).translate((i as f64) * width_each, 0.0)
                    })
                    .collect(),
            ),
        );
        // Extra wrapping to make the labels stretch against just the scale, not everything else
        // TODO Long labels aren't nicely lined up with the boundaries between buckets
        Widget::col(vec![
            Widget::draw_batch(ctx, batch),
            Widget::custom_row(
                labels
                    .into_iter()
                    .map(|lbl| Line(lbl).small().draw(ctx))
                    .collect(),
            )
            .evenly_spaced(),
        ])
        .container()
    }
}

pub struct DivergingScale {
    low_color: Color,
    mid_color: Color,
    high_color: Color,
    min: f64,
    avg: f64,
    max: f64,
    ignore: Option<(f64, f64)>,
}

impl DivergingScale {
    pub fn new(low_color: Color, mid_color: Color, high_color: Color) -> DivergingScale {
        DivergingScale {
            low_color,
            mid_color,
            high_color,
            min: 0.0,
            avg: 0.5,
            max: 1.0,
            ignore: None,
        }
    }

    pub fn range(mut self, min: f64, max: f64) -> DivergingScale {
        assert!(min < max);
        self.min = min;
        self.avg = (min + max) / 2.0;
        self.max = max;
        self
    }

    pub fn ignore(mut self, from: f64, to: f64) -> DivergingScale {
        assert!(from < to);
        self.ignore = Some((from, to));
        self
    }

    pub fn eval(&self, value: f64) -> Option<Color> {
        let value = value.max(self.min).min(self.max);
        if let Some((from, to)) = self.ignore {
            if value >= from && value <= to {
                return None;
            }
        }
        if value <= self.avg {
            Some(
                self.low_color
                    .lerp(self.mid_color, (value - self.min) / (self.avg - self.min)),
            )
        } else {
            Some(
                self.mid_color
                    .lerp(self.high_color, (value - self.avg) / (self.max - self.avg)),
            )
        }
    }

    pub fn make_legend<I: Into<String>>(self, ctx: &mut EventCtx, labels: Vec<I>) -> Widget {
        ColorLegend::gradient(
            ctx,
            &ColorScale(vec![self.low_color, self.mid_color, self.high_color]),
            labels,
        )
    }
}

// TODO Bad name
pub struct ColorNetwork<'a> {
    map: &'a Map,
    pub unzoomed: GeomBatch,
    pub zoomed: GeomBatch,
}

impl<'a> ColorNetwork<'a> {
    pub fn new(app: &'a App) -> ColorNetwork {
        let mut unzoomed = GeomBatch::new();
        unzoomed.push(
            app.cs.fade_map_dark,
            app.primary.map.get_boundary_polygon().clone(),
        );
        ColorNetwork {
            map: &app.primary.map,
            unzoomed,
            zoomed: GeomBatch::new(),
        }
    }

    pub fn add_l(&mut self, l: LaneID, color: Color) {
        self.unzoomed
            .push(color, self.map.get_parent(l).get_thick_polygon(self.map));
        let lane = self.map.get_l(l);
        self.zoomed.push(
            color.alpha(0.4),
            lane.lane_center_pts.make_polygons(lane.width),
        );
    }

    pub fn add_r(&mut self, r: RoadID, color: Color) {
        self.unzoomed
            .push(color, self.map.get_r(r).get_thick_polygon(self.map));
        self.zoomed.push(
            color.alpha(0.4),
            self.map.get_r(r).get_thick_polygon(self.map),
        );
    }

    pub fn add_i(&mut self, i: IntersectionID, color: Color) {
        self.unzoomed.push(color, self.map.get_i(i).polygon.clone());
        self.zoomed
            .push(color.alpha(0.4), self.map.get_i(i).polygon.clone());
    }

    pub fn add_b(&mut self, b: BuildingID, color: Color) {
        self.unzoomed.push(color, self.map.get_b(b).polygon.clone());
        self.zoomed
            .push(color.alpha(0.4), self.map.get_b(b).polygon.clone());
    }

    pub fn add_pl(&mut self, pl: ParkingLotID, color: Color) {
        self.unzoomed
            .push(color, self.map.get_pl(pl).polygon.clone());
        self.zoomed
            .push(color.alpha(0.4), self.map.get_pl(pl).polygon.clone());
    }

    // Order the roads by count, then interpolate a color based on position in that ordering.
    pub fn ranked_roads(&mut self, counter: Counter<RoadID>, scale: &ColorScale) {
        let roads = counter.sorted_asc();
        let len = roads.len() as f64;
        for (idx, list) in roads.into_iter().enumerate() {
            let color = scale.eval((idx as f64) / len);
            for r in list {
                self.add_r(r, color);
            }
        }
    }
    pub fn ranked_intersections(&mut self, counter: Counter<IntersectionID>, scale: &ColorScale) {
        let intersections = counter.sorted_asc();
        let len = intersections.len() as f64;
        for (idx, list) in intersections.into_iter().enumerate() {
            let color = scale.eval((idx as f64) / len);
            for i in list {
                self.add_i(i, color);
            }
        }
    }

    // Interpolate a color for each road based on the max count.
    pub fn pct_roads(&mut self, counter: Counter<RoadID>, scale: &ColorScale) {
        let max = counter.max() as f64;
        for (r, cnt) in counter.consume() {
            self.add_r(r, scale.eval((cnt as f64) / max));
        }
    }
    // Interpolate a color for each intersection based on the max count.
    pub fn pct_intersections(&mut self, counter: Counter<IntersectionID>, scale: &ColorScale) {
        let max = counter.max() as f64;
        for (i, cnt) in counter.consume() {
            self.add_i(i, scale.eval((cnt as f64) / max));
        }
    }

    pub fn build(self, ctx: &mut EventCtx) -> (Drawable, Drawable) {
        (ctx.upload(self.unzoomed), ctx.upload(self.zoomed))
    }
}
